//! Read-only native observation of an explicitly reviewed VS Code close.
//! The bridge must have a unique workbench window in its own Code.exe ancestry.
//! Success requires both window disappearance and the bound GUI process's exit.
use rusqlite::{params, Connection};
use serde_json::{json, Value};
type R<T> = Result<T, String>;
pub fn migrate(c: &Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS native_close_observations(job_id TEXT PRIMARY KEY,client TEXT NOT NULL,snapshot_id TEXT NOT NULL,deadline INTEGER NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL); CREATE INDEX IF NOT EXISTS native_close_pending ON native_close_observations(state,deadline);").map_err(|e|e.to_string())
}
/// The desktop owns this worker independently of WebView visibility. Dropping
/// its join handle detaches the thread; application process exit ends it.
pub fn start_observer() -> R<std::thread::JoinHandle<()>> {
    let mut c = crate::store::open()?;
    std::thread::Builder::new()
        .name("native-close-observations".into())
        .spawn(move || loop {
            // UI/coordinator reads also surface database failures; never turn an
            // observation error into successful closure or replay a close command.
            let _ = reconcile(&mut c);
            std::thread::sleep(std::time::Duration::from_millis(500));
        })
        .map_err(|e| e.to_string())
}

fn unique_window(owner: &Value, windows: &Value) -> R<Value> {
    let candidates: Vec<_> = windows
        .as_array()
        .ok_or("Window observation unavailable")?
        .iter()
        .filter(|w| w["pid"] == owner["pid"] && w["app"] == "VS Code")
        .collect();
    if candidates.len() != 1 {
        return Err(format!(
            "Native window mapping is ambiguous: {} workbench windows belong to this VS Code process",
            candidates.len()
        ));
    }
    let handle = candidates[0]["handle"]
        .as_str()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|h| *h != 0)
        .ok_or("Invalid native window handle")?;
    Ok(
        json!({"handle":handle.to_string(),"pid":owner["pid"],"processStarted":owner["processStarted"],"executable":owner["executable"],"boundAt":crate::now(),"method":"unique-window-in-bridge-process-ancestry"}),
    )
}
fn bind() -> R<Value> {
    let owner = crate::platform::vscode_owner()?;
    let binding = unique_window(
        &owner,
        &crate::platform::vscode_windows(owner["pid"].as_u64().ok_or("Missing process ID")? as u32)?,
    )?;
    let actual = crate::platform::native_window_identity(
        binding["handle"].as_str().unwrap().parse().unwrap(),
    )?
    .ok_or("VS Code window disappeared before observation could start")?;
    if actual != owner {
        return Err("VS Code process identity changed during binding".into());
    }
    Ok(binding)
}
fn authority(c: &Connection, client: &str, instance: &str, job: &str, snapshot: &str) -> R<()> {
    let valid: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=? AND client=? AND action='close' AND snapshot_id=? AND state='running')", params![job,client,snapshot], |r| r.get(0)).map_err(|e| e.to_string())?;
    let saved = crate::store::snapshot(c, snapshot)?;
    let raw: String = c
        .query_row(
            "SELECT body FROM reports WHERE client=? AND adapter='vscode'",
            [client],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let report = crate::protection::decode(&raw)?;
    if !valid
        || saved["client"] != client
        || saved["adapter"] != "vscode"
        || saved["state"] != "approved"
        || saved["body"]["instance"] != instance
        || report["instance"] != instance
        || crate::now().saturating_sub(saved["created"].as_u64().unwrap_or(0)) > 300000
    {
        return Err(
            "Native close observation requires this window's fresh, approved, running close job"
                .into(),
        );
    }
    Ok(())
}
pub fn begin(
    c: &mut Connection,
    client: &str,
    instance: &str,
    job: &str,
    snapshot: &str,
) -> R<Value> {
    authority(c, client, instance, job, snapshot)?;
    let binding = bind()?;
    let before = crate::platform::memory()?;
    let receipt = json!({"observing":true,"nativeWindow":binding,"detail":"Native close observation committed. The desktop will verify the window and GUI process independently of this bridge."});
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    authority(&tx, client, instance, job, snapshot)?;
    tx.execute(
        "UPDATE snapshots SET state='closing',before_json=?,result=? WHERE id=?",
        params![
            before.to_string(),
            crate::protection::encode(&receipt)?,
            snapshot
        ],
    )
    .map_err(|e| e.to_string())?;
    {
        let body = json!({"nativeWindow":binding,"instance":instance});
        tx.execute(
            "INSERT INTO native_close_observations VALUES(?,?,?,?, 'observing',?)",
            params![
                job,
                client,
                snapshot,
                crate::now() + 30000,
                crate::protection::encode(&body)?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(receipt)
}
fn disappeared(binding: &Value, current: Option<&Value>) -> bool {
    current.is_none_or(|current| {
        current["pid"] != binding["pid"] || current["processStarted"] != binding["processStarted"]
    })
}
/// Reconcile read-only evidence from any controller process. Never launch or
/// close applications here: the VS Code-owned bridge may already be terminated.
pub fn reconcile(c: &mut Connection) -> R<()> {
    let pending = {
        let mut q = c.prepare("SELECT job_id,client,snapshot_id,deadline,body FROM native_close_observations WHERE state='observing' ORDER BY deadline LIMIT 32").map_err(|e|e.to_string())?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, u64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    for (job, client, snapshot, deadline, raw) in pending {
        let body = match crate::protection::decode(&raw) {
            Ok(body) => body,
            Err(_) => {
                finish(
                    c,
                    &job,
                    &snapshot,
                    &client,
                    &json!({"closed":false,"detail":"Native close observation is unreadable. Closure cannot be verified; inspect the application."}),
                    None,
                )?;
                continue;
            }
        };
        let binding = &body["nativeWindow"];
        let active:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM jobs j JOIN snapshots s ON s.id=j.snapshot_id WHERE j.id=? AND j.client=? AND j.state='running' AND s.state='closing')",params![job,client],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !active {
            c.execute(
                "UPDATE native_close_observations SET state='interrupted' WHERE job_id=? AND state='observing'",
                [&job],
            )
            .map_err(|e| e.to_string())?;
            continue;
        }
        let expired = crate::now() > deadline;
        let observed = (|| -> R<bool> {
            let handle = binding["handle"]
                .as_str()
                .ok_or("Missing native window")?
                .parse::<usize>()
                .map_err(|e| e.to_string())?;
            let current = crate::platform::native_window_identity(handle)?;
            let pid = binding["pid"].as_u64().ok_or("Missing owner process")? as u32;
            let started = binding["processStarted"]
                .as_str()
                .ok_or("Missing process identity")?;
            Ok(disappeared(binding, current.as_ref())
                && crate::platform::process_ended(pid, started)?)
        })();
        let closed = !expired && observed.unwrap_or(false);
        if !closed && !expired {
            continue;
        }
        let result = json!({"closed":closed,"nativeWindow":binding,"ownerProcessExited":if closed {json!(true)}else{Value::Null},"checkedAt":crate::now(),"facts":[{"capability":"VS Code native window disappearance and owner process exit","expected":1,"observed":if closed {1}else{0},"status":if closed {"verified"}else{"unverified"}}],"detail":if closed {"The bound VS Code window disappeared and its GUI process exited. Resource observations do not establish exclusive project savings."}else{"The 30-second observation period expired without verified window disappearance and GUI process exit. Inspect VS Code before continuing."}});
        finish(
            c,
            &job,
            &snapshot,
            &client,
            &result,
            crate::platform::memory().ok().as_ref(),
        )?;
    }
    Ok(())
}
fn finish(
    c: &mut Connection,
    job: &str,
    snapshot: &str,
    client: &str,
    result: &Value,
    after: Option<&Value>,
) -> R<()> {
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs j JOIN snapshots s ON s.id=j.snapshot_id WHERE j.id=? AND j.client=? AND j.snapshot_id=? AND j.action='close' AND j.state='running' AND s.state='closing')",params![job,client,snapshot],|r|r.get(0)).map_err(|e|e.to_string())?;
    if active {
        let encoded = crate::protection::encode(result)?;
        tx.execute(
            "UPDATE snapshots SET state=?,after_json=?,result=? WHERE id=?",
            params![
                if result["closed"] == true {
                    "closed"
                } else {
                    "partial"
                },
                after.map(Value::to_string),
                encoded,
                snapshot
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE jobs SET state='done',result=? WHERE id=?",
            params![encoded, job],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE native_close_observations SET state=? WHERE job_id=? AND state='observing'",
        params![
            if active {
                if result["closed"] == true {
                    "verified"
                } else {
                    "unverified"
                }
            } else {
                "interrupted"
            },
            job
        ],
    )
    .map_err(|e| e.to_string())?;
    if active && result["closed"] == true {
        let report: Option<String> = tx
            .query_row("SELECT body FROM reports WHERE client=?", [client], |r| {
                r.get(0)
            })
            .ok();
        let saved = crate::store::snapshot(&tx, snapshot)?;
        if report
            .and_then(|r| crate::protection::decode(&r).ok())
            .is_some_and(|r| r["instance"] == saved["body"]["instance"])
        {
            tx.execute("UPDATE reports SET at=0 WHERE client=?", [client])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_live_process_is_not_exit_evidence_and_creation_identity_must_match() {
        let pid = std::process::id();
        let started = crate::platform::process_started(pid).unwrap();
        assert!(!crate::platform::process_ended(pid, &started).unwrap());
        assert!(crate::platform::process_ended(pid, "0").unwrap());
    }
    #[test]
    fn native_binding_requires_one_window_in_the_observed_process() {
        let owner = json!({"pid":5,"processStarted":"123","executable":"Code.exe"});
        let window = json!({"pid":5,"app":"VS Code","handle":"42"});
        assert!(unique_window(&owner, &json!([])).is_err());
        assert!(unique_window(&owner, &json!([window, window])).is_err());
        assert!(unique_window(&owner, &json!([{"pid":6,"app":"VS Code","handle":"42"}])).is_err());
        let binding = unique_window(&owner, &json!([window])).unwrap();
        assert!(!disappeared(&binding, Some(&owner)));
        assert!(disappeared(&binding, None));
        assert!(disappeared(
            &binding,
            Some(&json!({"pid":5,"processStarted":"456"}))
        ));
    }
    fn fixture() -> (Connection, String) {
        let c = crate::store::open_at(std::env::temp_dir().join(format!(
            "hibernate-native-close-{}.db",
            uuid::Uuid::new_v4()
        )))
        .unwrap();
        let body = crate::protection::encode(&json!({"instance":"window-a"})).unwrap();
        c.execute(
            "INSERT INTO reports VALUES('editor','vscode',?,?)",
            params![crate::now(), body],
        )
        .unwrap();
        c.execute(
            "INSERT INTO snapshots VALUES('saved','editor','vscode','approved',?,?,'{}',NULL,NULL)",
            params![crate::now(), body],
        )
        .unwrap();
        c.execute(
            "INSERT INTO jobs VALUES('close','editor','close',NULL,'saved','running',?,NULL)",
            [crate::now()],
        )
        .unwrap();
        (c, "saved".into())
    }
    #[test]
    fn corrupt_observation_fails_closed_without_breaking_other_dashboard_data() {
        let (mut c, id) = fixture();
        c.execute("UPDATE snapshots SET state='closing'", [])
            .unwrap();
        c.execute("INSERT INTO native_close_observations VALUES('close','editor','saved',?,'observing','not valid JSON')",[crate::now()+30000]).unwrap();
        reconcile(&mut c).unwrap();
        let saved = crate::store::snapshot(&c, &id).unwrap();
        assert_eq!(saved["state"], "partial");
        assert_eq!(saved["result"]["closed"], false);
    }
    #[test]
    fn disconnect_and_database_reopen_preserve_observation_but_timeout_never_claims_success() {
        let (mut c, id) = fixture();
        let path = c.path().unwrap().to_owned();
        let pid = std::process::id();
        let binding = json!({"handle":"42","pid":pid,"processStarted":crate::platform::process_started(pid).unwrap()});
        c.execute("UPDATE snapshots SET state='closing'", [])
            .unwrap();
        c.execute("INSERT INTO native_close_observations VALUES('close','editor','saved',?,'observing',?)",params![crate::now()+30000,crate::protection::encode(&json!({"nativeWindow":binding,"instance":"window-a"})).unwrap()]).unwrap();
        crate::store::integration_disconnected(&mut c, "editor", "window-a").unwrap();
        assert_eq!(crate::store::snapshot(&c, &id).unwrap()["state"], "closing");
        drop(c);
        let mut c = crate::store::open_at(path.into()).unwrap();
        reconcile(&mut c).unwrap();
        assert_eq!(crate::store::snapshot(&c, &id).unwrap()["state"], "closing");
        c.execute("UPDATE native_close_observations SET deadline=0", [])
            .unwrap();
        reconcile(&mut c).unwrap();
        let saved = crate::store::snapshot(&c, &id).unwrap();
        assert_eq!(saved["state"], "partial");
        assert_eq!(saved["result"]["closed"], false);
        assert_eq!(
            c.query_row("SELECT state FROM jobs WHERE id='close'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "done"
        );
    }
    #[test]
    fn observer_requires_review_authority_for_the_exact_window_and_job() {
        let (c, id) = fixture();
        assert!(authority(&c, "editor", "window-a", "close", &id).is_ok());
        assert!(authority(&c, "editor", "window-b", "close", &id).is_err());
        assert!(authority(&c, "foreign", "window-a", "close", &id).is_err());
        assert!(authority(&c, "editor", "window-a", "other-job", &id).is_err());
        c.execute("UPDATE snapshots SET state='captured'", [])
            .unwrap();
        assert!(authority(&c, "editor", "window-a", "close", &id).is_err());
    }
    #[test]
    fn observer_commits_both_results_but_never_overwrites_failed_or_interrupted_work() {
        for state in ["running", "failed", "interrupted"] {
            let (mut c, id) = fixture();
            c.execute("UPDATE snapshots SET state='closing'", [])
                .unwrap();
            c.execute("UPDATE jobs SET state=?", [state]).unwrap();
            c.execute("INSERT INTO native_close_observations VALUES('close','editor','saved',1,'observing','{}')",[]).unwrap();
            finish(
                &mut c,
                "close",
                &id,
                "editor",
                &json!({"closed":true}),
                Some(&json!({"at":1})),
            )
            .unwrap();
            let saved = crate::store::snapshot(&c, &id).unwrap();
            assert_eq!(
                saved["state"],
                if state == "running" {
                    "closed"
                } else {
                    "closing"
                }
            );
            let job: String = c
                .query_row("SELECT state FROM jobs WHERE id='close'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(job, if state == "running" { "done" } else { state });
            let observation: String = c
                .query_row(
                    "SELECT state FROM native_close_observations WHERE job_id='close'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            finish(
                &mut c,
                "close",
                &id,
                "editor",
                &json!({"closed":false}),
                None,
            )
            .unwrap();
            assert_eq!(
                c.query_row(
                    "SELECT state FROM native_close_observations WHERE job_id='close'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                observation,
                "A stale concurrent reconciler must not overwrite the completed observation"
            );
        }
    }
}
