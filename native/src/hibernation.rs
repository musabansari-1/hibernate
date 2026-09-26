//! Single-use project reviews, all-target preflight, then sequential reviewed closes.
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;
type R<T> = Result<T, String>;
pub fn migrate(c: &Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS hibernation_runs(id TEXT PRIMARY KEY,checkpoint_id TEXT NOT NULL,created INTEGER NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS hibernation_jobs(job_id TEXT PRIMARY KEY,run_id TEXT NOT NULL);").map_err(|e|e.to_string())
}
pub fn recover(c: &Connection) -> R<()> {
    c.execute_batch("BEGIN IMMEDIATE;
        UPDATE hibernation_runs SET state='expired' WHERE state='review';
        UPDATE hibernation_runs SET state='interrupted' WHERE state='running';
        UPDATE snapshots SET state='partial' WHERE state='approved' AND id IN (SELECT j.snapshot_id FROM jobs j JOIN hibernation_jobs h ON h.job_id=j.id JOIN hibernation_runs r ON r.id=h.run_id WHERE r.state='interrupted' AND j.state='queued');
        UPDATE jobs SET state='interrupted',result='\"Project close interrupted before dispatch. Capture and review again.\"' WHERE state='queued' AND id IN (SELECT h.job_id FROM hibernation_jobs h JOIN hibernation_runs r ON r.id=h.run_id WHERE r.state='interrupted');
        COMMIT;").map_err(|e|e.to_string())
}
pub fn list(c: &Connection, checkpoint: &str) -> R<Vec<Value>> {
    let mut q=c.prepare("SELECT id,created,state,body FROM hibernation_runs WHERE checkpoint_id=? AND state!='review' ORDER BY created DESC,rowid DESC LIMIT 5").map_err(|e|e.to_string())?;
    let rows=q.query_map([checkpoint],|r|Ok(json!({"id":r.get::<_,String>(0)?,"created":r.get::<_,u64>(1)?,"state":r.get::<_,String>(2)?,"body":crate::protection::sql_decode(r.get(3)?)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    Ok(rows)
}
fn eligible(c: &Connection, s: &Value, project: &Value) -> R<()> {
    if s["body"]["project"]["id"] != project["id"] {
        return Err("Snapshot belongs to another project".into());
    }
    if !crate::adapters::descriptor(s["adapter"].as_str().unwrap_or(""))
        .is_some_and(|d| d.reviewed_close)
    {
        return Err("Automatic close is not supported for this context".into());
    }
    if s["body"]["version"] != 1 {
        return Err("Unsupported capture version".into());
    }
    if !["captured", "blocked"].contains(&s["state"].as_str().unwrap_or(""))
        || crate::now().saturating_sub(s["created"].as_u64().unwrap_or(0)) > 300000
    {
        return Err(
            "Capture again: this snapshot expired or its close review was already used".into(),
        );
    }
    let at: u64 = c
        .query_row(
            "SELECT at FROM reports WHERE client=? AND adapter=?",
            params![s["client"].as_str(), s["adapter"].as_str()],
            |r| r.get(0),
        )
        .map_err(|_| "Original integration is offline")?;
    if crate::now().saturating_sub(at) >= 15000 {
        return Err("Original integration is offline".into());
    }
    let b = &s["body"];
    if s["adapter"] == "vscode" {
        if b["dirty"] == true {
            return Err("Unsaved editor work was captured; save it and capture again".into());
        }
        if b["terminals"].as_array().is_some_and(|a| !a.is_empty()) {
            return Err("Existing VS Code terminals must be finished before closing".into());
        }
        if b["debugging"] == true || b["tasks"].as_u64().unwrap_or(0) > 0 || b["remote"] == true {
            return Err("Active task, debugger or remote context blocks closing".into());
        }
    }
    Ok(())
}
fn review_body(c: &Connection, id: &str) -> R<Value> {
    let point = crate::checkpoints::get(c, id)?;
    let project = &point["project"];
    let mut steps = Vec::new();
    for entry in point["entries"]
        .as_array()
        .ok_or("Checkpoint is unreadable")?
    {
        let mut step = entry.clone();
        step["state"] = json!("retained");
        step["eligible"] = json!(false);
        if ["chrome", "vscode"].contains(&entry["kind"].as_str().unwrap_or("")) {
            let result = (|| -> R<Value> {
                let s = crate::store::snapshot(
                    c,
                    entry["snapshotId"].as_str().ok_or("Capture is missing")?,
                )?;
                if s["adapter"] != entry["kind"] {
                    return Err("Capture adapter differs from the checkpoint entry".into());
                }
                Ok(s)
            })();
            match result {
                Ok(s) => {
                    let status = eligible(c, &s, project);
                    step["eligible"] = json!(status.is_ok());
                    step["detail"]=json!(status.err().unwrap_or_else(||if s["adapter"]=="chrome"{"Review website work; live browser checks will run before any close".into()}else{"Native verification requires one workbench window in its app process and observes both window disappearance and process exit".into()}));
                    step["snapshot"] = s;
                }
                Err(e) => step["detail"] = json!(e),
            }
        } else {
            step["detail"] = json!(if entry["kind"] == crate::adapters::GENERIC {
                "Keep open — no exact window lifetime, saved-work reconstruction, or conditional close contract is proven. Application relaunch alone is insufficient for Sleep."
            } else {
                "Automatic close is not supported for this context."
            });
        }
        steps.push(step);
    }
    for (i, capture) in point["captures"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if capture["state"] != "captured" {
            steps.push(json!({"id":format!("capture-failure:{i}"),"kind":"unknown","label":"Uncaptured application context","eligible":false,"state":"retained","detail":capture["detail"]}));
        }
    }
    if let Some(services) = point["services"].as_array().filter(|v| !v.is_empty()) {
        let status = crate::services::validate_checkpoint_hibernate(services);
        steps.push(json!({"id":"services","kind":"services","label":"Development services","eligible":status.is_ok(),"state":"retained","detail":status.err().unwrap_or_else(||"Services that ContextSleep started and verified can be stopped gracefully. No force termination is used.".into())}));
    }
    Ok(
        json!({"checkpointId":id,"project":project,"steps":steps,"phase":"review","expiresAt":crate::now()+300000,"detail":"Only selected supported windows and verified development services can hibernate. All selected parts are checked before side effects; a failure stops later work."}),
    )
}
pub fn prepare(checkpoint: &str) -> R<Value> {
    let c = crate::store::open()?;
    prepare_at(&c, checkpoint)
}
fn prepare_at(c: &Connection, checkpoint: &str) -> R<Value> {
    let body = review_body(c, checkpoint)?;
    let id = uuid::Uuid::new_v4().to_string();
    let created = crate::now();
    c.execute(
        "UPDATE hibernation_runs SET state='expired' WHERE checkpoint_id=? AND state='review'",
        [checkpoint],
    )
    .map_err(|e| e.to_string())?;
    c.execute(
        "INSERT INTO hibernation_runs VALUES(?,?,?,'review',?)",
        params![
            id,
            checkpoint,
            created,
            crate::protection::encode(&journal_body(&body))?
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({"id":id,"created":created,"state":"review","body":body}))
}
fn claim(c: &mut Connection, id: &str, selected: &[String], acknowledged: bool) -> R<Value> {
    if !acknowledged {
        return Err("Review the captured windows and confirm their work is saved".into());
    }
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let (state, raw): (String, String) = tx
        .query_row(
            "SELECT state,body FROM hibernation_runs WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let mut body = crate::protection::decode(&raw)?;
    if state != "review" || body["expiresAt"].as_u64().unwrap_or(0) < crate::now() {
        return Err("This review expired or was already used. Open a fresh project review.".into());
    }
    let unique: HashSet<_> = selected.iter().collect();
    let steps = body["steps"].as_array().ok_or("Review is unreadable")?;
    if selected.is_empty()
        || selected.len() > 20
        || unique.len() != selected.len()
        || selected.iter().any(|id| {
            !steps
                .iter()
                .any(|s| s["id"] == *id && s["eligible"] == true)
        })
    {
        return Err("Select distinct supported windows from this review".into());
    }
    let mut snapshots = HashSet::new();
    for step in steps
        .iter()
        .filter(|s| selected.iter().any(|id| s["id"] == *id))
    {
        if step["kind"] == "services" {
            continue;
        }
        let sid = step["snapshotId"].as_str().ok_or("Capture is missing")?;
        if !snapshots.insert(sid) {
            return Err("Duplicate capture in close review".into());
        }
        eligible(&tx, &crate::store::snapshot(&tx, sid)?, &body["project"])?;
    }
    for step in body["steps"].as_array_mut().unwrap() {
        step["state"] = json!(if selected.iter().any(|id| step["id"] == *id) {
            "pending"
        } else {
            "retained"
        });
    }
    body["phase"] = json!("preflight");
    body["approvedAt"] = json!(crate::now());
    write(&tx, id, "running", &body)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(body)
}
// The dialog receives snapshot content, but the durable journal keeps only its
// reference. Deleting a snapshot must not leave a second full copy in a review.
fn journal_body(body: &Value) -> Value {
    let mut journal = body.clone();
    if let Some(steps) = journal["steps"].as_array_mut() {
        for step in steps {
            if let Some(object) = step.as_object_mut() {
                object.remove("snapshot");
            }
        }
    }
    journal
}
fn write(c: &Connection, id: &str, state: &str, body: &Value) -> R<()> {
    c.execute(
        "UPDATE hibernation_runs SET state=?,body=? WHERE id=?",
        params![state, crate::protection::encode(&journal_body(body))?, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn stop(c: &Connection, id: &str, body: &mut Value, state: &str) -> R<String> {
    for step in body["steps"].as_array_mut().unwrap() {
        if ["pending", "ready"].contains(&step["state"].as_str().unwrap_or("")) {
            step["state"] = json!("not_attempted");
        }
    }
    body["phase"] = json!("finished");
    write(c, id, state, body)?;
    Ok(id.into())
}
pub fn close(id: &str, selected: Vec<String>, acknowledged: bool) -> R<String> {
    let _guard = crate::checkpoints::OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    let mut c = crate::store::open()?;
    let mut body = claim(&mut c, id, &selected, acknowledged)?;
    let checkpoint_id = body["checkpointId"]
        .as_str()
        .ok_or("Project review is missing its checkpoint")?;
    let checkpoint = crate::checkpoints::get(&c, checkpoint_id)?;
    let checkpoint_services = checkpoint["services"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for i in 0..body["steps"].as_array().unwrap().len() {
        if body["steps"][i]["state"] != "pending" {
            continue;
        }
        body["steps"][i]["state"] = json!("checking");
        write(&c, id, "running", &body)?;
        let outcome = (|| -> R<Value> {
            if body["steps"][i]["kind"] == "services" {
                crate::services::validate_checkpoint_hibernate(&checkpoint_services)?;
                return Ok(json!({"checked":true}));
            }
            let s = crate::store::snapshot(
                &c,
                body["steps"][i]["snapshotId"]
                    .as_str()
                    .ok_or("Capture missing")?,
            )?;
            let job = crate::store::enqueue(
                s["client"].as_str().ok_or("Client missing")?.into(),
                "preflight_close".into(),
                None,
                Some(s["id"].as_str().ok_or("Capture missing")?.into()),
            )?;
            body["steps"][i]["preflightJobId"] = json!(job);
            write(&c, id, "running", &body)?;
            let result = crate::checkpoints::await_job(&job)?["result"].clone();
            if result["checked"] != true || result["snapshotId"] != body["steps"][i]["snapshotId"] {
                return Err("Integration did not return a matching close safety check. Update or reconnect it before retrying.".into());
            }
            Ok(result)
        })();
        match outcome {
            Ok(v) => {
                body["steps"][i]["preflight"] = v;
                body["steps"][i]["state"] = json!("ready");
                write(&c, id, "running", &body)?;
            }
            Err(e) => {
                body["steps"][i]["state"] = json!("blocked");
                body["steps"][i]["detail"] = json!(e);
                return stop(&c, id, &mut body, "blocked");
            }
        }
    }
    body["phase"] = json!("closing");
    body["before"] = match crate::platform::memory() {
        Ok(observation) => observation,
        Err(e) => {
            body["detail"] = json!(format!(
                "Resource baseline unavailable; no close was dispatched: {e}"
            ));
            return stop(&c, id, &mut body, "blocked");
        }
    };
    write(&c, id, "running", &body)?;
    for i in 0..body["steps"].as_array().unwrap().len() {
        if body["steps"][i]["state"] != "ready" {
            continue;
        }
        body["steps"][i]["state"] = json!("closing");
        write(&c, id, "running", &body)?;
        let result = (|| -> R<Value> {
            if body["steps"][i]["kind"] == "services" {
                return crate::services::hibernate_checkpoint(&checkpoint_services);
            }
            let sid = body["steps"][i]["snapshotId"]
                .as_str()
                .ok_or("Capture missing")?
                .to_string();
            let job = crate::store::close_for_run(sid.clone(), id)?;
            body["steps"][i]["jobId"] = json!(job);
            write(&c, id, "running", &body)?;
            let result = crate::checkpoints::await_job(&job)?;
            let snapshot = crate::store::snapshot(&c, &sid)?;
            if snapshot["state"] != "closed" || result["result"]["closed"] != true {
                return Err("Close completion is not verified. Inspect this application before proceeding with the rest of the project.".into());
            }
            Ok(
                json!({"adapterResult":result["result"],"before":snapshot["before"],"after":snapshot["after"]}),
            )
        })();
        match result {
            Ok(v) => {
                body["steps"][i]["state"] = json!("closed");
                body["steps"][i]["result"] = v;
            }
            Err(e) => {
                body["steps"][i]["state"] = json!("unverified");
                body["steps"][i]["detail"] = json!(e);
                body["after"] = crate::platform::memory().unwrap_or(Value::Null);
                return stop(&c, id, &mut body, "partial");
            }
        }
        write(&c, id, "running", &body)?;
    }
    body["after"] = crate::platform::memory().unwrap_or(Value::Null);
    stop(&c, id, &mut body, "closed-selected")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-review-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap()
    }
    fn fixture(c: &Connection) -> String {
        let p = crate::projects::active(c).unwrap();
        let body = json!({"version":1,"adapter":"chrome","project":p,"tabs":[]});
        c.execute(
            "INSERT INTO snapshots VALUES('s','browser','chrome','captured',?,?,'{}',NULL,NULL)",
            params![crate::now(), crate::protection::encode(&body).unwrap()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO reports VALUES('browser','chrome',?,'{}')",
            [crate::now()],
        )
        .unwrap();
        let checkpoint = json!({"project":p,"entries":[{"id":"s","snapshotId":"s","kind":"chrome","label":"Browser"},{"id":"t","kind":"terminal","label":"Shell"}],"captures":[],"services":[]});
        c.execute(
            "INSERT INTO checkpoints VALUES('point',?,1,'captured',?)",
            params![
                p["id"].as_str(),
                crate::protection::encode(&checkpoint).unwrap()
            ],
        )
        .unwrap();
        prepare_at(c, "point").unwrap()["id"]
            .as_str()
            .unwrap()
            .into()
    }
    #[test]
    fn durable_review_keeps_snapshot_references_without_duplicate_payloads() {
        let c = db();
        fixture(&c);
        let review = prepare_at(&c, "point").unwrap();
        assert!(review["body"]["steps"][0]["snapshot"].is_object());
        let raw: String = c
            .query_row(
                "SELECT body FROM hibernation_runs WHERE id=?",
                [review["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let persisted = crate::protection::decode(&raw).unwrap();
        assert_eq!(persisted["steps"][0]["snapshotId"], "s");
        assert!(persisted["steps"][0].get("snapshot").is_none());
    }
    #[test]
    fn reviews_are_scoped_single_use_and_leave_unsupported_contexts_visible() {
        let mut c = db();
        let id = fixture(&c);
        assert!(claim(&mut c, &id, &["s".into()], false).is_err());
        assert!(claim(&mut c, &id, &["t".into()], true).is_err());
        assert!(claim(&mut c, &id, &["foreign".into()], true).is_err());
        assert!(claim(&mut c, &id, &["s".into(), "s".into()], true).is_err());
        let body = claim(&mut c, &id, &["s".into()], true).unwrap();
        assert_eq!(body["steps"][1]["state"], "retained");
        assert!(claim(&mut c, &id, &["s".into()], true).is_err());
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn changed_snapshot_or_project_blocks_before_any_job() {
        let mut c = db();
        let id = fixture(&c);
        c.execute("UPDATE snapshots SET state='approved'", [])
            .unwrap();
        assert!(claim(&mut c, &id, &["s".into()], true).is_err());
        c.execute(
            "UPDATE snapshots SET state='captured',body=?",
            [
                crate::protection::encode(&json!({"version":1,"project":{"id":"foreign"}}))
                    .unwrap(),
            ],
        )
        .unwrap();
        assert!(claim(&mut c, &id, &["s".into()], true).is_err());
    }
    #[test]
    fn startup_interrupts_queued_project_closes_without_replaying() {
        let mut c = db();
        let id = fixture(&c);
        claim(&mut c, &id, &["s".into()], true).unwrap();
        c.execute("UPDATE snapshots SET state='approved'", [])
            .unwrap();
        c.execute(
            "INSERT INTO jobs VALUES('j','browser','close',NULL,'s','queued',1,NULL)",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO hibernation_jobs VALUES('j',?)", [&id])
            .unwrap();
        recover(&c).unwrap();
        assert_eq!(list(&c, "point").unwrap()[0]["state"], "interrupted");
        assert_eq!(crate::store::snapshot(&c, "s").unwrap()["state"], "partial");
        assert_eq!(
            c.query_row("SELECT state FROM jobs WHERE id='j'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "interrupted"
        );
    }
}
