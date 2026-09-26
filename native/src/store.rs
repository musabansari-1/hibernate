//! SQLite is the durability boundary. Each connection is short lived; FULL sync precedes close.
use crate::protection::{encode, sql_decode as parse};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
type R<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn path() -> R<PathBuf> {
    if let Some(base) = std::env::var_os("HIBERNATE_DATA_DIR") {
        return Ok(PathBuf::from(base).join("hibernate.db"));
    }
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is missing")?;
    Ok(PathBuf::from(base)
        .join("ProjectHibernate")
        .join("hibernate.db"))
}
pub fn open() -> R<Connection> {
    open_at(path()?)
}
pub fn open_at(path: PathBuf) -> R<Connection> {
    std::fs::create_dir_all(path.parent().ok_or("Invalid database path")?).map_err(err)?;
    let c = Connection::open(path).map_err(err)?;
    c.busy_timeout(Duration::from_secs(5)).map_err(err)?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;
      CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, body TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS reports(client TEXT PRIMARY KEY, adapter TEXT NOT NULL, at INTEGER NOT NULL, body TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS snapshots(id TEXT PRIMARY KEY, client TEXT NOT NULL, adapter TEXT NOT NULL, state TEXT NOT NULL, created INTEGER NOT NULL, body TEXT NOT NULL, before_json TEXT NOT NULL, after_json TEXT, result TEXT);
      CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, client TEXT NOT NULL, action TEXT NOT NULL, target TEXT, snapshot_id TEXT, state TEXT NOT NULL, created INTEGER NOT NULL, result TEXT);").map_err(err)?;
    let version: Option<String> = c
        .query_row("SELECT body FROM meta WHERE key='schema'", [], |r| r.get(0))
        .optional()
        .map_err(err)?;
    match version.as_deref() {
        None | Some("1") | Some("2") | Some("3") | Some("4") | Some("5") | Some("6") => (),
        _ => {
            return Err("Database schema is newer than this app. No snapshots were changed.".into())
        }
    }
    c.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
    if let Err(e) = crate::projects::migrate(&c) {
        let _ = c.execute_batch("ROLLBACK");
        return Err(e);
    }
    if let Err(e) = crate::protection::migrate(&c) {
        let _ = c.execute_batch("ROLLBACK");
        return Err(e);
    }
    if let Err(e) = crate::terminal::migrate(&c) {
        let _ = c.execute_batch("ROLLBACK");
        return Err(e);
    }
    if let Err(e) = crate::history::migrate(&c) {
        let _ = c.execute_batch("ROLLBACK");
        return Err(e);
    }
    crate::checkpoints::migrate(&c)?;
    crate::hibernation::migrate(&c)?;
    crate::native_close::migrate(&c)?;
    crate::app_context::migrate(&c)?;
    c.execute("INSERT OR REPLACE INTO meta VALUES('schema','6')", [])
        .map_err(err)?;
    c.execute_batch("COMMIT").map_err(err)?;
    Ok(c)
}
pub fn workspace(c: &Connection) -> R<Value> {
    crate::projects::active(c)
}
pub fn snapshot(c: &Connection, id: &str) -> R<Value> {
    c.query_row("SELECT id,client,adapter,state,created,body,before_json,after_json,result FROM snapshots WHERE id=?", [id], |r| Ok(json!({"id":r.get::<_,String>(0)?,"client":r.get::<_,String>(1)?,"adapter":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?,"created":r.get::<_,u64>(4)?,"body":parse(r.get(5)?)?,"before":parse(r.get(6)?)?,"after":r.get::<_,Option<String>>(7)?.map(parse).transpose()?,"result":r.get::<_,Option<String>>(8)?.map(parse).transpose()?}))).map_err(err)
}
pub fn dashboard() -> R<Value> {
    let mut c = open()?;
    let apps = crate::app_context::discover()
        .unwrap_or_else(|e| json!({"at":crate::now(),"items":[],"error":e}));
    crate::native_close::reconcile(&mut c)?;
    let mut rs = c
        .prepare("SELECT client,adapter,at,body FROM reports ORDER BY at DESC")
        .map_err(err)?;
    let reports = rs.query_map([], |r| Ok(json!({"client":r.get::<_,String>(0)?,"adapter":r.get::<_,String>(1)?,"at":r.get::<_,u64>(2)?,"body":parse(r.get(3)?)?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    let current = workspace(&c)?;
    let snapshots = crate::history::list(
        &c,
        current["id"].as_str().ok_or("Missing project")?,
        30,
        false,
    )?;
    let mut js = c
        .prepare("SELECT id,state,action,result FROM jobs ORDER BY created DESC LIMIT 10")
        .map_err(err)?;
    let jobs = js.query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"state":r.get::<_,String>(1)?,"action":r.get::<_,String>(2)?,"result":r.get::<_,Option<String>>(3)?.map(parse).transpose()?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    let workspace = workspace(&c)?;
    let project_id = workspace["id"].as_str().ok_or("Project identity missing")?;
    let health: Option<String> = c
        .query_row(
            "SELECT body FROM meta WHERE key=?",
            [format!("health:{project_id}")],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    let explorer = crate::context::explorer().unwrap_or_else(|e| json!({"items":[],"error":e}));
    let checkpoints = crate::checkpoints::list(&c, project_id)?;
    let chrome_launches: Vec<Value> = reports
        .iter()
        .filter(|r| r["adapter"] == "chrome")
        .filter_map(|r| {
            r["client"]
                .as_str()
                .and_then(|client| crate::context::chrome_launch_binding(&c, client))
        })
        .collect();
    Ok(
        json!({"apps":apps,"adapterCatalog":crate::adapters::catalog(),"checkpoints":checkpoints,"memory":crate::platform::memory()?,"windows":crate::platform::windows()?,"explorer":explorer,"reports":reports,"chromeLaunches":chrome_launches,"snapshots":snapshots,"jobs":jobs,"workspace":workspace,"projects":crate::projects::list(&c)?,"memberships":crate::projects::members(&c)?,"services":crate::health::list(&c,project_id)?,"serviceHealth":health.map(parse).transpose().map_err(err)?}),
    )
}
pub fn save_workspace(name: String, project_path: String) -> R<()> {
    if name.trim().is_empty() || name.len() > 200 || project_path.len() > 32768 {
        return Err("Invalid workspace".into());
    }
    let c = open()?;
    let value = workspace(&c)?;
    crate::projects::save(&c, value["id"].as_str(), &name, &project_path)?;
    Ok(())
}
pub fn enqueue(
    client: String,
    action: String,
    target: Option<String>,
    snapshot_id: Option<String>,
) -> R<String> {
    enqueue_checked(client, action, target, snapshot_id, false, None, None)
}
pub fn capture_for_project(client: String, target: String, project: &str) -> R<String> {
    enqueue_checked(
        client,
        "capture".into(),
        Some(target),
        None,
        false,
        Some(project),
        None,
    )
}
pub fn close_reviewed(snapshot_id: String, acknowledged: bool, quick_sleep: bool) -> R<String> {
    if !acknowledged {
        return Err("Review what can be recovered and confirm saved work before closing".into());
    }
    let c = open()?;
    let s = snapshot(&c, &snapshot_id)?;
    if crate::now().saturating_sub(s["created"].as_u64().unwrap_or(0)) > 300000
        || !["captured", "blocked"].contains(&s["state"].as_str().unwrap_or(""))
    {
        return Err("Review expired or was already used. Capture the current state again.".into());
    }
    let client = s["client"]
        .as_str()
        .ok_or("Missing integration")?
        .to_string();
    enqueue_checked(
        client,
        if quick_sleep { "quick_sleep" } else { "close" }.into(),
        None,
        Some(snapshot_id),
        true,
        None,
        None,
    )
}
pub(crate) fn close_for_run(snapshot: String, run: &str) -> R<String> {
    let s = self::snapshot(&open()?, &snapshot)?;
    enqueue_checked(
        s["client"].as_str().ok_or("Missing client")?.into(),
        "close".into(),
        None,
        Some(snapshot),
        true,
        None,
        Some(run),
    )
}
fn enqueue_checked(
    client: String,
    action: String,
    target: Option<String>,
    snapshot_id: Option<String>,
    reviewed: bool,
    project_id: Option<&str>,
    run_id: Option<&str>,
) -> R<String> {
    if ![
        "capture",
        "hibernate",
        "resume",
        "fixture",
        "close",
        "quick_sleep",
        "preflight_close",
    ]
    .contains(&action.as_str())
    {
        return Err("Unsupported action".into());
    }
    if client == crate::adapters::NATIVE_CLIENT {
        return Err("Native app context has no integration command or close channel".into());
    }
    if ["close", "quick_sleep"].contains(&action.as_str()) && !reviewed {
        return Err("Use the snapshot review before closing or parking user work".into());
    }
    let mut c = open()?;
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let at: u64 = tx
        .query_row("SELECT at FROM reports WHERE client=?", [&client], |r| {
            r.get(0)
        })
        .map_err(|_| "Integration is not connected")?;
    if crate::now().saturating_sub(at) > 15000 {
        return Err(
            "Open the integration in the original browser profile or VS Code window first.".into(),
        );
    }
    let busy: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM jobs WHERE client=? AND state IN ('queued','running')",
            [&client],
            |r| r.get(0),
        )
        .map_err(err)?;
    if busy > 0 {
        return Err("An operation is pending. Reconnect the integration; interrupted operations require inspection.".into());
    }
    let mut bound_project = match project_id {
        Some(id) => crate::projects::get(&tx, id)?,
        None => workspace(&tx)?,
    };
    if ["resume", "close", "quick_sleep", "preflight_close"].contains(&action.as_str()) {
        let s = snapshot(&tx, snapshot_id.as_deref().ok_or("Missing snapshot")?)?;
        if s["client"] != client {
            return Err("Snapshot belongs to a different integration".into());
        }
        bound_project = s["body"]["project"].clone();
        if ["close", "quick_sleep", "preflight_close"].contains(&action.as_str()) {
            if crate::now().saturating_sub(s["created"].as_u64().unwrap_or(0)) > 300000
                || !["captured", "blocked"].contains(&s["state"].as_str().unwrap_or(""))
            {
                return Err(
                    "Review expired or was already used. Capture the current state again.".into(),
                );
            }
            if action == "quick_sleep" && s["adapter"] != "chrome" {
                return Err("Quick Sleep currently supports Chrome background tabs only".into());
            }
            if action != "preflight_close" {
                tx.execute("UPDATE snapshots SET state='approved',result=? WHERE id=?",params![encode(&json!({"reviewedAt":crate::now(),"action":action,"detail":"User reviewed the saved snapshot and confirmed application work was saved. Approval is single-use and the adapter must revalidate live state."}))?,snapshot_id]).map_err(err)?;
            }
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO jobs VALUES(?,?,?,?,?,'queued',?,NULL)",
        params![id, client, action, target, snapshot_id, crate::now()],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT INTO job_context VALUES(?,?)",
        params![id, encode(&bound_project)?],
    )
    .map_err(err)?;
    if let Some(run) = run_id {
        let running: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM hibernation_runs WHERE id=? AND state='running')",
                [run],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !running {
            return Err("Project close review is no longer running".into());
        }
        tx.execute("INSERT INTO hibernation_jobs VALUES(?,?)", params![id, run])
            .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    Ok(id)
}
pub fn handle(c: &mut Connection, req: &Value) -> R<Value> {
    let op = req["op"].as_str().ok_or("Missing operation")?;
    let client = req["client"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 200)
        .ok_or("Invalid client")?;
    if client == crate::adapters::NATIVE_CLIENT {
        return Err(
            "Native discovery identity is reserved; integration messages cannot claim it".into(),
        );
    }
    match op {
        "prepareTerminal" => crate::terminal::prepare_vscode(
            c,
            client,
            req["snapshotId"].as_str().ok_or("Snapshot missing")?,
            req["kind"].as_str().ok_or("Context kind missing")?,
            req["index"].as_u64().ok_or("Context index missing")? as usize,
        ),
        "terminalObservations" => Ok(json!(crate::terminal::observations(c, client)?)),
        "poll" => {
            let adapter = req["adapter"]
                .as_str()
                .filter(|s| ["chrome", "vscode"].contains(s))
                .ok_or("Unknown adapter")?;
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            if adapter == "vscode" {
                let previous: Option<(u64, String)> = tx
                    .query_row(
                        "SELECT at,body FROM reports WHERE client=?",
                        [client],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .map_err(err)?;
                if let Some((at, raw)) = previous {
                    let prior = parse(raw).map_err(err)?;
                    let operation_active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE client=? AND state IN ('queued','running') AND created>?)",params![client,crate::now().saturating_sub(300000)],|r|r.get(0)).map_err(err)?;
                    if at > 0
                        && (crate::now().saturating_sub(at) < 15000 || operation_active)
                        && prior["instance"].is_string()
                        && prior["instance"] != req["body"]["instance"]
                    {
                        return Err("Another VS Code window for this workspace owns the integration. Use one connected window per workspace; no operation was claimed.".into());
                    }
                }
            }
            tx.execute("INSERT INTO reports VALUES(?,?,?,?) ON CONFLICT(client) DO UPDATE SET at=excluded.at,body=excluded.body",params![client,adapter,crate::now(),encode(&req["body"])?]).map_err(err)?;
            crate::projects::observe_activity(&tx, client, &req["body"])?;
            // Never replay an operation after a host/worker crash. Side effects may already have happened.
            tx.execute("UPDATE jobs SET state='interrupted',result='\"Operation interrupted; inspect snapshot and live windows before retrying.\"' WHERE client=? AND state='running' AND created<?",params![client,crate::now().saturating_sub(300000)]).map_err(err)?;
            tx.execute("UPDATE jobs SET state='expired',result='\"Queued action expired without execution. Review current windows and try again.\"' WHERE client=? AND state='queued' AND created<?",params![client,crate::now().saturating_sub(30000)]).map_err(err)?;
            let job=tx.query_row("SELECT id,action,target,snapshot_id FROM jobs WHERE client=? AND state='queued' ORDER BY created LIMIT 1",[client],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?))).optional().map_err(err)?;
            let result = if let Some((id, action, target, sid)) = job {
                tx.execute("UPDATE jobs SET state='running' WHERE id=?", [&id])
                    .map_err(err)?;
                json!({"id":id,"action":action,"target":target,"snapshot":sid.map(|s|snapshot(&tx,&s)).transpose()?})
            } else {
                Value::Null
            };
            tx.commit().map_err(err)?;
            Ok(result)
        }
        "save" => {
            let adapter = req["body"]["adapter"]
                .as_str()
                .filter(|s| ["chrome", "vscode"].contains(s))
                .ok_or("Unknown snapshot adapter")?;
            if req["body"]["version"] != 1 {
                return Err("Unsupported adapter schema".into());
            }
            let id = uuid::Uuid::new_v4().to_string();
            let before = crate::platform::memory()?;
            let mut body = req["body"].clone();
            body["project"] = if let Some(job) = req["jobId"].as_str() {
                let project:String=c.query_row("SELECT jc.project FROM job_context jc JOIN jobs j ON j.id=jc.job_id WHERE j.id=? AND j.client=? AND j.state='running'",params![job,client],|r|r.get(0)).map_err(|_|"Capture job is no longer active for this integration")?;
                parse(project).map_err(err)?
            } else {
                workspace(c)?
            };
            c.execute(
                "INSERT INTO snapshots VALUES(?,?,?,'captured',?,?,?,NULL,NULL)",
                params![
                    id,
                    client,
                    adapter,
                    crate::now(),
                    encode(&body)?,
                    before.to_string()
                ],
            )
            .map_err(err)?;
            crate::history::index(c, &id, body["project"]["id"].as_str())?;
            if let Some(job) = req["jobId"].as_str() {
                c.execute(
                    "UPDATE jobs SET snapshot_id=? WHERE id=? AND client=? AND state='running'",
                    params![id, job, client],
                )
                .map_err(err)?;
            }
            snapshot(c, &id)
        }
        "finishSnapshot" => {
            let state = req["state"]
                .as_str()
                .filter(|s| {
                    [
                        "blocked",
                        "closing",
                        "restoring",
                        "closed",
                        "partial",
                        "verified",
                        "incomplete",
                        "parked",
                    ]
                    .contains(s)
                })
                .ok_or("Invalid outcome")?;
            let after = crate::platform::memory()?;
            let count=c.execute("UPDATE snapshots SET state=?, before_json=CASE WHEN ?='closing' THEN ? ELSE before_json END,after_json=CASE WHEN ? IN ('closed','parked') THEN ? ELSE after_json END,result=? WHERE id=? AND client=?",params![state,state,after.to_string(),state,after.to_string(),encode(&req["result"])? ,req["id"].as_str().ok_or("Missing snapshot id")?,client]).map_err(err)?;
            if count != 1 {
                return Err("Snapshot not found for this client".into());
            }
            Ok(json!(true))
        }
        "complete" => {
            c.execute(
                "UPDATE reports SET at=? WHERE client=?",
                params![crate::now(), client],
            )
            .map_err(err)?;
            c.execute(
                "UPDATE jobs SET state=?,result=? WHERE id=? AND client=? AND state='running'",
                params![
                    if req["ok"] == true { "done" } else { "failed" },
                    encode(&req["result"])?,
                    req["id"].as_str().ok_or("Missing job id")?,
                    client
                ],
            )
            .map_err(err)?;
            Ok(json!(true))
        }
        _ => Err("Unsupported operation".into()),
    }
}
pub fn integration_report_disconnected(c: &mut Connection, client: &str) -> R<()> {
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    tx.execute("UPDATE reports SET at=0 WHERE client=?", [client])
        .map_err(err)?;
    let message = encode(&json!(
        "Integration disconnected before the operation completed. No queued work was replayed."
    ))?;
    tx.execute("UPDATE jobs SET state='interrupted',result=? WHERE client=? AND state IN ('queued','running')",params![message,client]).map_err(err)?;
    tx.commit().map_err(err)
}
pub fn integration_disconnected(c: &mut Connection, client: &str, instance: &str) -> R<()> {
    crate::native_close::reconcile(c)?;
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let current: Option<String> = tx
        .query_row(
            "SELECT body FROM reports WHERE client=? AND adapter='vscode'",
            [client],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    if current
        .map(parse)
        .transpose()
        .map_err(err)?
        .is_some_and(|body| body["instance"] == instance)
    {
        tx.execute("UPDATE reports SET at=0 WHERE client=?", [client])
            .map_err(err)?;
        let message=encode(&json!("VS Code integration disconnected. The operation was not replayed. Inspect the saved context and reopen the original workspace before restoring."))?;
        tx.execute("UPDATE jobs SET state='interrupted',result=? WHERE client=? AND state IN ('queued','running') AND id NOT IN (SELECT job_id FROM native_close_observations WHERE state='observing')",params![message,client]).map_err(err)?;
        let unfinished = {
            let mut query = tx
                .prepare("SELECT id,result FROM snapshots WHERE client=? AND state='closing' AND id NOT IN (SELECT snapshot_id FROM native_close_observations WHERE state='observing')")
                .map_err(err)?;
            let rows = query
                .query_map([client], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
                })
                .map_err(err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(err)?
        };
        for (id, prior) in unfinished {
            let result = json!({"closed":false,"detail":"VS Code integration ended after close intent. Native closure remains unverified; captured context is retained.","closeIntent":prior.and_then(|v|crate::protection::decode(&v).ok())});
            tx.execute(
                "UPDATE snapshots SET state='partial',result=? WHERE id=?",
                params![encode(&result)?, id],
            )
            .map_err(err)?;
        }
    }
    tx.commit().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> (Connection, PathBuf) {
        let p = std::env::temp_dir()
            .join(format!("hibernate-test-{}", uuid::Uuid::new_v4()))
            .join("test.db");
        (open_at(p.clone()).unwrap(), p)
    }
    fn save(c: &mut Connection) -> Value {
        handle(c,&json!({"op":"save","client":"profile-a","body":{"adapter":"chrome","version":1,"tabs":[{"url":"https://example.com"}]}})).unwrap()
    }
    #[test]
    fn delayed_capture_keeps_the_project_bound_at_enqueue() {
        let (mut c, _) = db();
        let original = workspace(&c).unwrap();
        c.execute(
            "INSERT INTO jobs VALUES('bound-job','profile-a','capture','1',NULL,'running',?,NULL)",
            [crate::now()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO job_context VALUES('bound-job',?)",
            [encode(&original).unwrap()],
        )
        .unwrap();
        let other = crate::projects::save(&c, None, "Other project", "D:\\other").unwrap();
        crate::projects::select(&c, other["id"].as_str().unwrap()).unwrap();
        let snapshot=handle(&mut c,&json!({"op":"save","client":"profile-a","jobId":"bound-job","body":{"adapter":"chrome","version":1,"tabs":[]}})).unwrap();
        assert_eq!(snapshot["body"]["project"]["id"], original["id"]);
        assert!(handle(&mut c,&json!({"op":"save","client":"another-profile","jobId":"bound-job","body":{"adapter":"chrome","version":1}})).is_err());
    }
    #[test]
    fn another_vscode_window_cannot_claim_work_and_disconnect_does_not_replay() {
        let (mut c, _) = db();
        let poll = |instance: &str| json!({"op":"poll","client":"editor","adapter":"vscode","body":{"instance":instance,"items":[]}});
        handle(&mut c, &poll("first")).unwrap();
        c.execute("INSERT INTO jobs VALUES('operation','editor','capture','workspace',NULL,'queued',?,NULL)",[crate::now()]).unwrap();
        assert!(handle(&mut c, &poll("second")).is_err());
        assert_eq!(handle(&mut c, &poll("first")).unwrap()["id"], "operation");
        c.execute(
            "UPDATE reports SET at=? WHERE client='editor'",
            [crate::now().saturating_sub(20000)],
        )
        .unwrap();
        assert!(handle(&mut c, &poll("second")).is_err());
        integration_disconnected(&mut c, "editor", "second").unwrap();
        assert_eq!(
            c.query_row("SELECT state FROM jobs WHERE id='operation'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "running"
        );
        integration_disconnected(&mut c, "editor", "first").unwrap();
        assert_eq!(
            c.query_row("SELECT state FROM jobs WHERE id='operation'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "interrupted"
        );
        assert!(handle(&mut c, &poll("second")).unwrap().is_null());
    }
    #[test]
    fn snapshot_payload_is_protected_on_disk_and_corruption_is_reported() {
        let (mut c, _) = db();
        let s = save(&mut c);
        let raw: String = c
            .query_row(
                "SELECT body FROM snapshots WHERE id=?",
                [s["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(raw.starts_with("dpapi:v1:"));
        assert!(!raw.contains("example.com"));
        c.execute(
            "UPDATE snapshots SET body='dpapi:v1:00' WHERE id=?",
            [s["id"].as_str().unwrap()],
        )
        .unwrap();
        assert!(snapshot(&c, s["id"].as_str().unwrap()).is_err());
    }
    #[test]
    fn snapshots_and_project_identity_survive_reopening() {
        let (mut c, p) = db();
        let s = save(&mut c);
        let project = s["body"]["project"]["id"].clone();
        assert!(project.is_string());
        drop(c);
        let c = open_at(p).unwrap();
        let restored = snapshot(&c, s["id"].as_str().unwrap()).unwrap();
        assert_eq!(restored["body"], s["body"]);
        assert_eq!(workspace(&c).unwrap()["id"], project);
    }
    #[test]
    fn reject_future_adapter_schema_before_insert() {
        let (mut c, _) = db();
        assert!(handle(
            &mut c,
            &json!({"op":"save","client":"a","body":{"adapter":"chrome","version":2}})
        )
        .is_err());
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn native_discovery_namespace_cannot_be_claimed_by_an_extension() {
        let (mut c, _) = db();
        assert!(handle(&mut c,&json!({"op":"poll","client":crate::adapters::NATIVE_CLIENT,"adapter":"chrome","body":{"items":[]}})).is_err());
        assert!(handle(&mut c,&json!({"op":"save","client":"extension","body":{"version":1,"adapter":crate::adapters::GENERIC}})).is_err());
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM generic_origins", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn another_profile_cannot_update_a_snapshot() {
        let (mut c, _) = db();
        let s = save(&mut c);
        assert!(handle(
            &mut c,
            &json!({"op":"finishSnapshot","client":"b","id":s["id"],"state":"closed"})
        )
        .is_err());
        assert_eq!(
            snapshot(&c, s["id"].as_str().unwrap()).unwrap()["state"],
            "captured"
        );
    }
    #[test]
    fn stale_jobs_are_never_replayed() {
        let (mut c, _) = db();
        for (id, state) in [("old-queued", "queued"), ("old-running", "running")] {
            c.execute(
                "INSERT INTO jobs VALUES(?,'a','hibernate','123',NULL,?,1,NULL)",
                params![id, state],
            )
            .unwrap();
        }
        assert_eq!(
            handle(
                &mut c,
                &json!({"op":"poll","client":"a","adapter":"chrome","body":{}})
            )
            .unwrap(),
            Value::Null
        );
        assert_eq!(
            c.query_row("SELECT state FROM jobs WHERE id='old-queued'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "expired"
        );
        assert_eq!(
            c.query_row("SELECT state FROM jobs WHERE id='old-running'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "interrupted"
        );
    }
    #[test]
    fn job_claim_is_at_most_once() {
        let (mut c, _) = db();
        c.execute(
            "INSERT INTO jobs VALUES('job','a','capture','123',NULL,'queued',?,NULL)",
            [crate::now()],
        )
        .unwrap();
        let req = json!({"op":"poll","client":"a","adapter":"chrome","body":{}});
        assert_eq!(handle(&mut c, &req).unwrap()["id"], "job");
        assert_eq!(handle(&mut c, &req).unwrap(), Value::Null);
    }
    #[test]
    fn restore_result_preserves_close_measurement() {
        let (mut c, _) = db();
        let s = save(&mut c);
        handle(
            &mut c,
            &json!({"op":"finishSnapshot","client":"profile-a","id":s["id"],"state":"closed"}),
        )
        .unwrap();
        let after = snapshot(&c, s["id"].as_str().unwrap()).unwrap()["after"].clone();
        handle(&mut c,&json!({"op":"finishSnapshot","client":"profile-a","id":s["id"],"state":"verified","result":{"verified":true}})).unwrap();
        assert_eq!(
            snapshot(&c, s["id"].as_str().unwrap()).unwrap()["after"],
            after
        );
    }
    #[test]
    fn schema_three_upgrades_without_losing_project_or_snapshot() {
        let (mut c, p) = db();
        let project = workspace(&c).unwrap();
        let saved = save(&mut c);
        c.execute_batch("DROP TABLE hibernation_jobs; DROP TABLE hibernation_runs; UPDATE meta SET body='3' WHERE key='schema';").unwrap();
        drop(c);
        let c = open_at(p).unwrap();
        assert_eq!(workspace(&c).unwrap(), project);
        assert_eq!(snapshot(&c, saved["id"].as_str().unwrap()).unwrap(), saved);
        assert_eq!(
            c.query_row("SELECT body FROM meta WHERE key='schema'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "6"
        );
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM hibernation_jobs", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn schema_four_adds_durable_close_observation_without_losing_snapshots() {
        let (mut c, p) = db();
        let saved = save(&mut c);
        c.execute_batch(
            "DROP TABLE native_close_observations; UPDATE meta SET body='4' WHERE key='schema';",
        )
        .unwrap();
        drop(c);
        let c = open_at(p).unwrap();
        assert_eq!(snapshot(&c, saved["id"].as_str().unwrap()).unwrap(), saved);
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM native_close_observations", [], |r| {
                r.get::<_, u32>(0)
            })
            .unwrap(),
            0
        );
    }
    #[test]
    fn newer_database_is_not_silently_downgraded() {
        let (c, p) = db();
        c.execute("UPDATE meta SET body='99' WHERE key='schema'", [])
            .unwrap();
        drop(c);
        assert!(open_at(p).is_err());
    }
}
