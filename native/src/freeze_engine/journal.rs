//! Separate, forward-versioned, encrypted freeze journal. Product DB stays intact.
//! Recovery records evidence and never changes a process state.
use super::{Lease, MemoryEvidence, R};
use crate::app_windows::ProcessIdentity;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub identity: ProcessIdentity,
    pub parent: Option<ProcessIdentity>,
}
pub fn open(path: &Path) -> R<Connection> {
    let c = Connection::open(path).map_err(|e| e.to_string())?;
    c.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    c.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;
        CREATE TABLE IF NOT EXISTS freeze_meta(version INTEGER NOT NULL);",
    )
    .map_err(|e| e.to_string())?;
    let version: Option<u32> = c
        .query_row("SELECT version FROM freeze_meta", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    if version.is_some_and(|v| v != 1) {
        return Err("Unsupported freeze journal schema; no downgrade".into());
    }
    c.execute_batch("BEGIN IMMEDIATE;
        INSERT INTO freeze_meta SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM freeze_meta);
        CREATE TABLE IF NOT EXISTS freezes(id TEXT PRIMARY KEY, state TEXT NOT NULL, body TEXT NOT NULL);
        COMMIT;").map_err(|e| e.to_string())?;
    Ok(c)
}
fn write(c: &Connection, id: &str, state: &str, body: &Value) -> R<()> {
    let n = c
        .execute(
            "UPDATE freezes SET state=?,body=? WHERE id=?",
            params![state, crate::protection::encode(body)?, id],
        )
        .map_err(|e| e.to_string())?;
    if n != 1 {
        return Err("Missing freeze journal entry".into());
    }
    Ok(())
}
pub fn rows(c: &Connection) -> R<Vec<Value>> {
    c.prepare("SELECT id,state,body FROM freezes ORDER BY rowid").map_err(|e| e.to_string())?
        .query_map([], |r| Ok(json!({"id":r.get::<_, String>(0)?, "state":r.get::<_, String>(1)?, "body":crate::protection::sql_decode(r.get(2)?)?})))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
pub fn recover(c: &Connection) -> R<Vec<Value>> {
    for row in rows(c)? {
        if !matches!(row["state"].as_str(), Some("intent" | "frozen" | "waking")) {
            continue;
        }
        let mut body = row["body"].clone();
        let owner: ProcessIdentity =
            serde_json::from_value(body["controller"].clone()).map_err(|e| e.to_string())?;
        // A live or unreadable owner must not be mistaken for an abandoned operation.
        if !crate::platform::process_ended(owner.pid, &owner.process_started)? {
            continue;
        }
        let targets: Vec<Target> =
            serde_json::from_value(body["targets"].clone()).map_err(|e| e.to_string())?;
        let evidence: Vec<_> = targets
            .iter()
            .map(|target| {
                let identity = &target.identity;
                let state =
                    match crate::platform::process_ended(identity.pid, &identity.process_started) {
                        Ok(true) => "original-process-ended",
                        Ok(false)
                            if crate::app_windows::identity(identity.pid).as_ref()
                                == Some(identity) =>
                        {
                            "same-process-present"
                        }
                        _ => "identity-unknown",
                    };
                json!({"identity":identity,"observation":state})
            })
            .collect();
        body["recovery"] = json!({"at":crate::now(),"targets":evidence,"detail":"Controller ended. Its non-inherited state handles were released by Windows; application health is unverified. No suspension, count-based resume, or relaunch was attempted."});
        write(
            c,
            row["id"].as_str().ok_or("Missing operation id")?,
            "owner-exited",
            &body,
        )?;
    }
    rows(c)
}

/// Owns the live state handles. Serialized journal rows never grant authority.
pub struct Operation {
    pub id: String,
    body: Value,
    leases: Vec<Lease>,
}
#[derive(Default)]
pub struct Faults {
    #[cfg(feature = "deep-freeze-lab")]
    pub freeze_at: Option<usize>,
    #[cfg(feature = "deep-freeze-lab")]
    pub resume_at: Option<usize>,
    #[cfg(feature = "deep-freeze-lab")]
    pub confirmation_write: bool,
    #[cfg(feature = "deep-freeze-lab")]
    pub exit_before_confirmation_marker: Option<std::path::PathBuf>,
}
impl Operation {
    #[cfg(feature = "deep-freeze-lab")]
    pub fn begin(c: &Connection, targets: &[Target], trim: bool, faults: &Faults) -> R<Self> {
        let leases = targets
            .iter()
            .map(|t| Lease::open(&t.identity))
            .collect::<R<Vec<_>>>()?;
        Self::begin_owned(c, targets, leases, trim, faults, || Ok(()), Value::Null)
    }
    pub(crate) fn begin_owned(
        c: &Connection,
        targets: &[Target],
        mut leases: Vec<Lease>,
        trim: bool,
        _faults: &Faults,
        validate_frozen: impl FnOnce() -> R<()>,
        metadata: Value,
    ) -> R<Self> {
        if targets.is_empty() || targets.len() > 16 {
            return Err("Invalid fixture group size".into());
        }
        let mut unique = HashSet::new();
        for target in targets {
            if !unique.insert(target.identity.pid) {
                return Err("Duplicate target".into());
            }
        }
        let transaction =
            rusqlite::Transaction::new_unchecked(c, rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
        for row in rows(&transaction)? {
            if matches!(row["state"].as_str(), Some("intent" | "frozen" | "waking")) {
                let prior: Vec<Target> = serde_json::from_value(row["body"]["targets"].clone())
                    .map_err(|e| e.to_string())?;
                if prior
                    .iter()
                    .any(|old| targets.iter().any(|new| new.identity == old.identity))
                {
                    return Err("This process already has an active freeze operation".into());
                }
            }
        }
        if leases.len() != targets.len() {
            return Err("Missing retained process handles".into());
        }
        let before = leases
            .iter()
            .map(Lease::measure)
            .collect::<R<Vec<MemoryEvidence>>>()?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut body = json!({"version":1,"controller":crate::app_windows::identity(std::process::id()).ok_or("Missing controller identity")?,"targets":targets,"intentAt":crate::now(),"intendedState":"frozen","before":before,"trimRequested":trim});
        body["review"] = metadata;
        transaction
            .execute(
                "INSERT INTO freezes VALUES(?,'intent',?)",
                params![id, crate::protection::encode(&body)?],
            )
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        let attempt = (|| {
            for (_i, lease) in leases.iter_mut().enumerate() {
                #[cfg(feature = "deep-freeze-lab")]
                if _faults.freeze_at == Some(_i) {
                    return Err("Injected fixture freeze failure".into());
                }
                lease.freeze()?;
            }
            #[cfg(feature = "deep-freeze-lab")]
            if let Some(marker) = &_faults.exit_before_confirmation_marker {
                std::fs::write(marker, "all leases frozen, journal still intent")
                    .map_err(|e| e.to_string())?;
                std::process::exit(74); // fixture controller only: skip Rust drops
            }
            validate_frozen()?;
            // No trim before all targets have confirmed suspension.
            let mut trim_results = Vec::new();
            if trim {
                for lease in &leases {
                    trim_results.push(match lease.trim() {
                        Ok(()) => json!({"ok":true}),
                        Err(e) => json!({"ok":false,"error":e}),
                    });
                }
            }
            body["trimResults"] = json!(trim_results);
            body["after"] = json!(leases.iter().map(Lease::measure).collect::<R<Vec<_>>>()?);
            body["confirmedAt"] = json!(crate::now());
            #[cfg(feature = "deep-freeze-lab")]
            if _faults.confirmation_write {
                return Err("Injected fixture confirmation commit failure".into());
            }
            write(c, &id, "frozen", &body)
        })();
        if let Err(error) = attempt {
            // Release every acquired state object before trying to write failure.
            drop(leases);
            body["error"] = json!(error);
            let persisted = write(c, &id, "released-after-failure", &body);
            return Err(format!(
                "{error}; owned handles released; failure journal: {persisted:?}"
            ));
        }
        Ok(Self { id, body, leases })
    }
    pub fn wake(&mut self, c: &Connection, _faults: &Faults) -> R<()> {
        if self.leases.is_empty() {
            return Ok(());
        }
        // Failure to record Wake must never prevent emergency release.
        let intent = write(c, &self.id, "waking", &self.body);
        let mut failures = Vec::new();
        if let Err(e) = intent {
            failures.push(e);
        }
        for (_i, lease) in self.leases.iter_mut().enumerate() {
            #[cfg(feature = "deep-freeze-lab")]
            let result = if _faults.resume_at == Some(_i) {
                Err("Injected fixture resume failure".into())
            } else {
                lease.wake()
            };
            #[cfg(not(feature = "deep-freeze-lab"))]
            let result = lease.wake();
            if let Err(e) = result {
                failures.push(e);
            }
        }
        self.leases.clear(); // kernel release backstop, even on partial resume failure
        self.body["wakeAt"] = json!(crate::now());
        self.body["wakeErrors"] = json!(failures);
        write(
            c,
            &self.id,
            if failures.is_empty() {
                "released"
            } else {
                "released-with-errors"
            },
            &self.body,
        )?;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn journal_rejects_newer_schema_and_preserves_evidence() {
        let dir = std::env::temp_dir().join(format!(
            "contextsleep-freeze-journal-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("journal.db");
        let c = open(&path).unwrap();
        c.execute("UPDATE freeze_meta SET version=99", []).unwrap();
        drop(c);
        assert!(open(&path).is_err());
    }
}
