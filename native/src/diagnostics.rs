//! Privacy-bounded compatibility diagnostics for support and private-beta feedback.
//! The exported report intentionally contains aggregates only: no project paths, URLs,
//! titles, terminal CWDs/history, service commands, snapshot payloads or raw process IDs.
use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::io::Write;

type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn one_counts(c: &Connection, sql: &str, key: &str) -> R<Vec<Value>> {
    let mut q = c.prepare(sql).map_err(err)?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))
        .map_err(err)?;
    rows.map(|row| {
        let (value, count) = row.map_err(err)?;
        let mut object = Map::new();
        object.insert(key.to_string(), json!(value));
        object.insert("count".into(), json!(count));
        Ok(Value::Object(object))
    })
    .collect()
}

fn two_counts(c: &Connection, sql: &str, first: &str, second: &str) -> R<Vec<Value>> {
    let mut q = c.prepare(sql).map_err(err)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u64>(2)?,
            ))
        })
        .map_err(err)?;
    rows.map(|row| {
        let (a, b, count) = row.map_err(err)?;
        let mut object = Map::new();
        object.insert(first.to_string(), json!(a));
        object.insert(second.to_string(), json!(b));
        object.insert("count".into(), json!(count));
        Ok(Value::Object(object))
    })
    .collect()
}

fn integrations(c: &Connection) -> R<Vec<Value>> {
    let fresh_after = crate::now().saturating_sub(15_000);
    let mut q = c
        .prepare(
            "SELECT adapter,COUNT(*),SUM(CASE WHEN at>=? THEN 1 ELSE 0 END) FROM reports GROUP BY adapter ORDER BY adapter",
        )
        .map_err(err)?;
    let rows = q
        .query_map([fresh_after], |r| {
            Ok(json!({
                "adapter": r.get::<_, String>(0)?,
                "registered": r.get::<_, u64>(1)?,
                "fresh": r.get::<_, Option<u64>>(2)?.unwrap_or(0)
            }))
        })
        .map_err(err)?;
    rows.map(|row| row.map_err(err)).collect()
}

fn service_summary(c: &Connection) -> R<Value> {
    let mut q = c.prepare("SELECT body FROM services").map_err(err)?;
    let mut total = 0_u64;
    let mut launch_configured = 0_u64;
    let mut launch_verified = 0_u64;
    let mut shutdown_verified = 0_u64;
    let mut unreadable = 0_u64;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?;
    for raw in rows {
        total += 1;
        match raw.map_err(err).and_then(|v| crate::protection::decode(&v)) {
            Ok(body) => {
                launch_configured += u64::from(!body["launch"].is_null());
                launch_verified += u64::from(body["launchRun"]["verified"] == true);
                shutdown_verified += u64::from(body["shutdownRun"]["verified"] == true);
            }
            Err(_) => unreadable += 1,
        }
    }
    Ok(json!({
        "definitions": total,
        "launchConfigured": launch_configured,
        "launchVerified": launch_verified,
        "shutdownVerified": shutdown_verified,
        "unreadable": unreadable
    }))
}

fn scalar(c: &Connection, sql: &str) -> R<u64> {
    c.query_row(sql, [], |r| r.get::<_, u64>(0)).map_err(err)
}

pub(crate) fn build(c: &Connection) -> R<Value> {
    let active = crate::projects::active(c).ok();
    let project_configured = active
        .as_ref()
        .and_then(|p| p["projectPath"].as_str())
        .is_some_and(|p| !p.trim().is_empty());
    let memory = crate::platform::memory()?;
    let explorer = crate::context::explorer().unwrap_or_else(|_| json!({"items":[]}));
    let visible_windows = crate::platform::windows()
        .ok()
        .and_then(|v| v.as_array().map(|a| a.len() as u64))
        .unwrap_or(0);
    let explorer_folders = explorer["items"]
        .as_array()
        .map(|a| a.len() as u64)
        .unwrap_or(0);
    let schema: String = c
        .query_row("SELECT body FROM meta WHERE key='schema'", [], |r| r.get(0))
        .unwrap_or_else(|_| "unknown".into());

    Ok(json!({
        "format": "project-hibernate-support-report",
        "version": 1,
        "exportedAt": crate::now(),
        "application": {
            "version": env!("CARGO_PKG_VERSION"),
            "schema": schema,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "debugBuild": cfg!(debug_assertions)
        },
        "privacy": {
            "projectPaths": false,
            "browserUrlsOrTitles": false,
            "terminalCwdsOrHistory": false,
            "serviceUrlsOrCommands": false,
            "snapshotPayloads": false,
            "rawProcessIds": false,
            "detail": "This support report is aggregate-only. It is intended to be shareable without exposing saved project/application content."
        },
        "system": {
            "totalPhysical": memory["totalPhysical"],
            "availablePhysical": memory["availablePhysical"],
            "chromeWorkingSet": memory["chromeWorkingSet"],
            "unreadableProcesses": memory["unreadableProcesses"],
            "visibleWindowCount": visible_windows,
            "shellExposedExplorerFolderCount": explorer_folders
        },
        "inventory": {
            "projectCount": scalar(c,"SELECT COUNT(*) FROM projects")?,
            "activeProjectConfigured": project_configured,
            "membershipCount": scalar(c,"SELECT COUNT(*) FROM memberships")?,
            "recordedTerminalSessions": scalar(c,"SELECT COUNT(*) FROM terminal_sessions")?,
            "terminalIntegrationMembers": scalar(c,"SELECT COUNT(*) FROM terminal_members")?,
            "chromeColdStartBindings": scalar(c,"SELECT COUNT(*) FROM meta WHERE key LIKE 'chrome-launch:%'")?,
            "archivedSnapshots": scalar(c,"SELECT COUNT(*) FROM snapshot_index WHERE archived<>0")?
        },
        "integrations": integrations(c)?,
        "snapshots": two_counts(c,"SELECT adapter,state,COUNT(*) FROM snapshots GROUP BY adapter,state ORDER BY adapter,state","adapter","state")?,
        "jobs": two_counts(c,"SELECT action,state,COUNT(*) FROM jobs GROUP BY action,state ORDER BY action,state","action","state")?,
        "checkpoints": one_counts(c,"SELECT state,COUNT(*) FROM checkpoints GROUP BY state ORDER BY state","state")?,
        "reconstructionRuns": one_counts(c,"SELECT state,COUNT(*) FROM reconstruction_runs GROUP BY state ORDER BY state","state")?,
        "hibernationRuns": one_counts(c,"SELECT state,COUNT(*) FROM hibernation_runs GROUP BY state ORDER BY state","state")?,
        "services": service_summary(c)?,
        "safety": {
            "windowsTerminalAutomaticClose": false,
            "explorerAutomaticClose": false,
            "terminalCommandHistoryReplay": false,
            "serviceForceKillFallback": false,
            "automaticReboot": false,
            "detail": "Context-only capabilities remain context-only; uncertain side effects are not automatically replayed."
        }
    }))
}

pub fn export() -> R<String> {
    let c = crate::store::open()?;
    let report = build(&c)?;
    let dir = crate::store::path()?
        .parent()
        .ok_or("Missing data folder")?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let target = dir.join(format!("support-{}-{}.json", crate::now(), uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(&report).map_err(err)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(err)?;
    file.write_all(&bytes).map_err(err)?;
    file.sync_all().map_err(err)?;
    Ok(target.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn report_is_aggregate_only_even_when_database_contains_sensitive_strings() {
        let path = std::env::temp_dir().join(format!("hibernate-support-{}.db", uuid::Uuid::new_v4()));
        let c = crate::store::open_at(path).unwrap();
        let project = crate::projects::active(&c).unwrap();
        let id = project["id"].as_str().unwrap();
        crate::projects::save(
            &c,
            Some(id),
            "TOPSECRET project",
            "D:\\TOPSECRET\\client-work",
        )
        .unwrap();
        c.execute(
            "INSERT OR REPLACE INTO reports VALUES('sensitive-client','chrome',?,?)",
            params![
                crate::now(),
                crate::protection::encode(&json!({"adapter":"chrome","version":1,"items":[{"title":"TOPSECRET title","url":"https://example.com/TOPSECRET"}]})).unwrap()
            ],
        )
        .unwrap();
        c.execute(
            "INSERT INTO services VALUES('sensitive-service',?,?)",
            params![
                id,
                crate::protection::encode(&json!({
                    "name":"TOPSECRET service",
                    "url":"http://127.0.0.1:32123/TOPSECRET",
                    "launch":{"executable":"D:\\TOPSECRET\\node.exe","args":["TOPSECRET"]},
                    "launchRun":{"verified":true}
                })).unwrap()
            ],
        )
        .unwrap();
        c.execute(
            "INSERT INTO snapshots VALUES('sensitive-snapshot','sensitive-client','chrome','captured',?,?,?,NULL,?)",
            params![
                crate::now(),
                crate::protection::encode(&json!({"TOPSECRET":"https://example.com/TOPSECRET"})).unwrap(),
                "{}",
                crate::protection::encode(&json!({"TOPSECRET":"result"})).unwrap()
            ],
        )
        .unwrap();
        c.execute(
            "INSERT INTO jobs VALUES('sensitive-job','sensitive-client','capture','TOPSECRET-target','sensitive-snapshot','done',?,?)",
            params![crate::now(), crate::protection::encode(&json!({"TOPSECRET":"job-result"})).unwrap()],
        )
        .unwrap();

        let report = build(&c).unwrap().to_string();
        for secret in ["TOPSECRET", "example.com", "32123", "client-work", "node.exe"] {
            assert!(!report.contains(secret), "support report leaked {secret}: {report}");
        }
        assert!(report.contains("project-hibernate-support-report"));
        assert!(report.contains("chrome"));
        assert!(report.contains("capture"));
    }
}
