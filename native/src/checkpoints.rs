//! Project reconstruction journal. Persist intent before work; never replay an interrupted step.
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};
type R<T> = Result<T, String>;
pub(crate) static OPERATION: Mutex<()> = Mutex::new(());
pub fn migrate(c: &Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS checkpoints(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,created INTEGER NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS reconstruction_runs(id TEXT PRIMARY KEY,checkpoint_id TEXT NOT NULL,created INTEGER NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL);") .map_err(|e|e.to_string())
}
pub fn recover() -> R<()> {
    let c = crate::store::open()?;
    recover_at(&c)?;
    crate::hibernation::recover(&c)
}
fn recover_at(c: &Connection) -> R<()> {
    c.execute_batch("UPDATE checkpoints SET state='interrupted' WHERE state='capturing'; UPDATE reconstruction_runs SET state='interrupted' WHERE state='running';").map_err(|e|e.to_string())
}
fn write_checkpoint(c: &Connection, id: &str, state: &str, body: &Value) -> R<()> {
    c.execute(
        "UPDATE checkpoints SET state=?,body=? WHERE id=?",
        params![state, crate::protection::encode(body)?, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub(crate) fn get(c: &Connection, id: &str) -> R<Value> {
    let raw: String = c
        .query_row("SELECT body FROM checkpoints WHERE id=?", [id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?;
    crate::protection::decode(&raw)
}
pub fn list(c: &Connection, project: &str) -> R<Vec<Value>> {
    let mut q=c.prepare("SELECT id,created,state,body FROM checkpoints WHERE project_id=? ORDER BY created DESC,rowid DESC LIMIT 30").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([project], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    rows.into_iter().map(|(id,created,state,raw)|{
        let body=crate::protection::decode(&raw).unwrap_or_else(|e|json!({"error":e,"entries":[],"captures":[]}));
        let mut q=c.prepare("SELECT id,created,state,body FROM reconstruction_runs WHERE checkpoint_id=? ORDER BY created DESC,rowid DESC LIMIT 5").map_err(|e|e.to_string())?;
        let runs=q.query_map([&id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"created":r.get::<_,u64>(1)?,"state":r.get::<_,String>(2)?,"body":crate::protection::sql_decode(r.get(3)?)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        Ok(json!({"id":id,"created":created,"state":state,"body":body,"runs":runs,"hibernations":crate::hibernation::list(c,&id)?}))
    }).collect()
}
pub fn export(id: &str) -> R<String> {
    let c = crate::store::open()?;
    let checkpoint = get(&c, id)?;
    let mut snapshot_ids = HashSet::new();
    for value in checkpoint["captures"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(checkpoint["entries"].as_array().into_iter().flatten())
    {
        if let Some(snapshot_id) = value["snapshotId"].as_str() {
            snapshot_ids.insert(snapshot_id.to_string());
        }
    }
    if snapshot_ids.len() > 32 {
        return Err("Checkpoint contains too many snapshots to export".into());
    }
    let snapshots = snapshot_ids
        .into_iter()
        .map(|snapshot_id| crate::store::snapshot(&c, &snapshot_id))
        .collect::<Result<Vec<_>, _>>()?;
    let mut chrome_clients = HashSet::new();
    let chrome_launches = snapshots
        .iter()
        .filter(|s| s["adapter"] == "chrome")
        .filter_map(|s| s["client"].as_str())
        .filter(|client| chrome_clients.insert((*client).to_string()))
        .filter_map(|client| crate::context::chrome_launch_binding(&c, client))
        .collect::<Vec<_>>();
    let payload = crate::protection::encode(&json!({
        "format":"project-hibernate-checkpoint",
        "version":1,
        "exportedAt":crate::now(),
        "checkpoint":checkpoint,
        "snapshots":snapshots,
        "chromeLaunches":chrome_launches
    }))?;
    let dir = crate::store::path()?
        .parent()
        .ok_or("Missing data folder")?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(format!(
        "checkpoint-{}-{}.hibernate",
        uuid::Uuid::new_v4(),
        crate::now()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| e.to_string())?;
    file.write_all(payload.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

pub fn import(path: &str) -> R<String> {
    let path = Path::new(path);
    if !path.is_absolute() || path.extension().and_then(|s| s.to_str()) != Some("hibernate") {
        return Err("Choose an absolute path to a .hibernate checkpoint backup".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 16_000_000 {
        return Err("Checkpoint backup is too large".into());
    }
    let mut raw = String::new();
    file.take(16_000_001)
        .read_to_string(&mut raw)
        .map_err(|e| e.to_string())?;
    if !raw.starts_with("dpapi:v1:") {
        return Err("Only Windows-account-protected ContextSleep backups are accepted".into());
    }
    let envelope = crate::protection::decode(&raw)?;
    let mut c = crate::store::open()?;
    import_value(&mut c, &envelope)
}

fn import_value(c: &mut Connection, envelope: &Value) -> R<String> {
    if envelope["format"] != "project-hibernate-checkpoint" || envelope["version"] != 1 {
        return Err("Unsupported checkpoint backup format".into());
    }
    let point = &envelope["checkpoint"];
    if point["version"] != 1 {
        return Err("Unsupported checkpoint schema".into());
    }
    let project = &point["project"];
    let project_id = project["id"].as_str().ok_or("Missing project identity")?;
    uuid::Uuid::parse_str(project_id).map_err(|_| "Invalid project identity")?;
    if project["name"]
        .as_str()
        .is_none_or(|s| s.is_empty() || s.len() > 200)
        || project["projectPath"]
            .as_str()
            .is_none_or(|s| s.len() > 32700)
        || point["captures"].as_array().is_none_or(|v| v.len() > 32)
        || point["services"].as_array().is_none_or(|v| v.len() > 32)
    {
        return Err("Invalid checkpoint metadata".into());
    }
    let snapshots = envelope["snapshots"]
        .as_array()
        .ok_or("Checkpoint backup has no snapshot list")?;
    if snapshots.len() > 32 {
        return Err("Checkpoint backup contains too many snapshots".into());
    }
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if let Ok(existing) = crate::projects::get(&tx, project_id) {
        if existing["projectPath"] != project["projectPath"] || existing["name"] != project["name"]
        {
            return Err("This project identity already exists with different metadata. Import into a clean project record instead of merging identities.".into());
        }
    } else {
        tx.execute(
            "INSERT INTO projects VALUES(?,?)",
            params![project_id, crate::protection::encode(project)?],
        )
        .map_err(|e| e.to_string())?;
    }

    let mut snapshot_map = HashMap::new();
    for snapshot in snapshots {
        let old_id = snapshot["id"]
            .as_str()
            .ok_or("Backup snapshot identity is missing")?;
        uuid::Uuid::parse_str(old_id).map_err(|_| "Backup snapshot identity is invalid")?;
        let adapter = snapshot["adapter"]
            .as_str()
            .filter(|a| ["chrome", "vscode", "context"].contains(a))
            .ok_or("Unsupported backup snapshot adapter")?;
        if snapshot["body"]["version"] != 1
            || snapshot["body"]["adapter"] != adapter
            || snapshot["body"]["project"]["id"] != project_id
        {
            return Err(
                "Backup snapshot schema or project identity does not match the checkpoint".into(),
            );
        }
        let client = snapshot["client"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() < 200)
            .ok_or("Backup snapshot integration identity is invalid")?;
        let new_id = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO snapshots VALUES(?,?,?,'imported',?,?,?,NULL,?)", params![
            new_id,
            client,
            adapter,
            crate::now(),
            crate::protection::encode(&snapshot["body"] )?,
            snapshot["before"].to_string(),
            crate::protection::encode(&json!({"detail":"Imported checkpoint backup. No close authority, pending command or previous verification result was imported."}))?
        ]).map_err(|e| e.to_string())?;
        crate::history::index(&tx, &new_id, Some(project_id))?;
        snapshot_map.insert(old_id.to_string(), new_id);
    }

    let mut services = Vec::new();
    for saved in point["services"].as_array().into_iter().flatten() {
        if saved["projectId"] != project_id {
            return Err("Checkpoint service belongs to another project".into());
        }
        let name = saved["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 120)
            .ok_or("Invalid checkpoint service name")?;
        let url = saved["url"]
            .as_str()
            .ok_or("Invalid checkpoint service endpoint")?;
        crate::health::endpoint(url)?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut body = json!({"id":id,"projectId":project_id,"name":name,"url":url});
        if !saved["launch"].is_null() {
            match serde_json::from_value::<crate::services::Launch>(saved["launch"].clone()) {
                Ok(definition) if crate::services::validate(&definition).is_ok() => body["launch"] = serde_json::to_value(definition).map_err(|e| e.to_string())?,
                _ => body["importWarning"] = json!("The saved startup command is unavailable on this machine and was not approved for automatic restart. Configure it again before including this service in restore."),
            }
        }
        tx.execute(
            "INSERT INTO services VALUES(?,?,?)",
            params![id, project_id, crate::protection::encode(&body)?],
        )
        .map_err(|e| e.to_string())?;
        services.push(body);
    }

    let mut import_warnings = Vec::new();
    if let Some(bindings) = envelope["chromeLaunches"].as_array() {
        if bindings.len() > 16 {
            return Err("Checkpoint backup contains too many Chrome launch bindings".into());
        }
        for binding in bindings {
            if let Err(error) = crate::context::import_chrome_launch_binding(&tx, binding) {
                import_warnings.push(format!(
                    "Chrome cold-start binding was not imported: {error}"
                ));
            }
        }
    }
    let mut imported = point.clone();
    imported["services"] = json!(services);
    if !import_warnings.is_empty() {
        imported["importWarnings"] = json!(import_warnings);
    }
    let mut rebuilt_entries = Vec::new();
    if let Some(captures) = imported["captures"].as_array_mut() {
        for capture in captures {
            if let Some(object) = capture.as_object_mut() {
                object.remove("jobId");
                if let Some(old_id) = object
                    .get("snapshotId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                {
                    if let Some(new_id) = snapshot_map.get(&old_id) {
                        object.insert("snapshotId".into(), json!(new_id));
                        object.insert("state".into(), json!("captured"));
                        object.remove("detail");
                        rebuilt_entries.extend(entries(&tx, project_id, new_id)?);
                    } else {
                        object.remove("snapshotId");
                        object.insert("state".into(), json!("failed"));
                        object.insert(
                            "detail".into(),
                            json!(
                                "The checkpoint backup did not contain this referenced snapshot."
                            ),
                        );
                    }
                }
            }
        }
    }
    imported["entries"] = json!(rebuilt_entries);
    let checkpoint_id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO checkpoints VALUES(?,?,?,'imported',?)",
        params![
            checkpoint_id,
            project_id,
            crate::now(),
            crate::protection::encode(&imported)?
        ],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(checkpoint_id)
}

fn job(id: &str) -> R<Value> {
    let c = crate::store::open()?;
    c.query_row("SELECT state,snapshot_id,result FROM jobs WHERE id=?",[id],|r|Ok(json!({"state":r.get::<_,String>(0)?,"snapshotId":r.get::<_,Option<String>>(1)?,"result":r.get::<_,Option<String>>(2)?.map(crate::protection::sql_decode).transpose()?}))).map_err(|e|e.to_string())
}
pub(crate) fn await_job(id: &str) -> R<Value> {
    let end = Instant::now() + Duration::from_secs(65);
    loop {
        crate::native_close::reconcile(&mut crate::store::open()?)?;
        let value = job(id)?;
        match value["state"].as_str().unwrap_or("") {
            "done" => return Ok(value),
            "failed" | "expired" | "interrupted" => return Err(value["result"].to_string()),
            _ => (),
        }
        if Instant::now() >= end {
            return Err("Operation did not settle within the observation period. Inspect the integration before retrying; it may still finish.".into());
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}
fn entries(c: &Connection, project: &str, id: &str) -> R<Vec<Value>> {
    let s = crate::store::snapshot(c, id)?;
    if s["body"]["project"]["id"] != project {
        return Err("Capture belongs to another project".into());
    }
    let b = &s["body"];
    if s["adapter"] == "context" {
        let mut items = Vec::new();
        if let Some(folders) = b["explorer"]["items"].as_array() {
            for (i, f) in folders.iter().enumerate() {
                items.push(json!({"id":format!("{id}:explorer:{i}"),"snapshotId":id,"kind":"explorer","index":i,"label":format!("Explorer folder - {}",f["url"].as_str().unwrap_or("Unknown folder"))}));
            }
        }
        if let Some(terminals) = b["terminal"]["items"].as_array() {
            for (i, terminal) in terminals.iter().enumerate() {
                if let Some(cwd) = terminal["cwd"].as_str() {
                    items.push(json!({"id":format!("{id}:terminal:{i}"),"snapshotId":id,"kind":"terminal","index":i,"label":format!("Terminal - {cwd}")}));
                }
            }
        } else if b["terminal"]["cwd"].is_string() {
            items.push(json!({"id":format!("{id}:terminal:0"),"snapshotId":id,"kind":"terminal","index":0,"label":format!("New terminal - {}",b["terminal"]["cwd"].as_str().unwrap_or(""))}));
        }
        Ok(items)
    } else {
        Ok(vec![
            json!({"id":id,"snapshotId":id,"kind":s["adapter"],"label":if s["adapter"]=="chrome"{format!("Chrome - {} tabs",b["tabs"].as_array().map_or(0,Vec::len))}else{"VS Code workspace and editors".into()}}),
        ])
    }
}
pub fn capture(project: &str, targets: Vec<(String, String)>, include_context: bool) -> R<String> {
    let _guard = OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    if targets.len() > 20 || targets.is_empty() && !include_context {
        return Err(
            "Select between one and twenty application contexts, or include local folders".into(),
        );
    }
    let mut unique = HashSet::new();
    if targets.iter().any(|t| !unique.insert(t)) {
        return Err("Duplicate capture target".into());
    }
    let c = crate::store::open()?;
    let p = crate::projects::get(&c, project)?;
    let repository = crate::repository::inspect(p["projectPath"].as_str().unwrap_or(""))
        .unwrap_or_else(|e| json!({"status":"unverified","detail":e}));
    let mut body = json!({"version":1,"project":p,"repository":repository,"services":crate::health::list(&c,project)?,"captures":targets.iter().map(|(client,target)|json!({"client":client,"target":target,"state":"pending"})).collect::<Vec<_>>(),"entries":[]});
    if include_context {
        body["captures"]
            .as_array_mut()
            .unwrap()
            .push(json!({"client":"local-context","state":"pending"}));
    }
    let id = uuid::Uuid::new_v4().to_string();
    c.execute(
        "INSERT INTO checkpoints VALUES(?,?,?,'capturing',?)",
        params![id, project, crate::now(), crate::protection::encode(&body)?],
    )
    .map_err(|e| e.to_string())?;
    for i in 0..body["captures"].as_array().unwrap().len() {
        body["captures"][i]["state"] = json!("capturing");
        write_checkpoint(&c, &id, "capturing", &body)?;
        let outcome = (|| -> R<Vec<Value>> {
            let sid = if i < targets.len() {
                let (client, target) = &targets[i];
                let job_id =
                    crate::store::capture_for_project(client.clone(), target.clone(), project)?;
                body["captures"][i]["jobId"] = json!(job_id);
                write_checkpoint(&c, &id, "capturing", &body)?;
                await_job(&job_id)?["snapshotId"]
                    .as_str()
                    .ok_or("Adapter did not save a snapshot")?
                    .to_string()
            } else {
                crate::context::capture_context_for(project)?
            };
            body["captures"][i]["snapshotId"] = json!(sid);
            entries(&c, project, &sid)
        })();
        match outcome {
            Ok(items) => {
                body["entries"].as_array_mut().unwrap().extend(items);
                body["captures"][i]["state"] = json!("captured");
            }
            Err(e) => {
                body["captures"][i]["state"] = json!("failed");
                body["captures"][i]["detail"] = json!(e);
            }
        }
        write_checkpoint(&c, &id, "capturing", &body)?;
    }
    let complete = body["captures"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["state"] == "captured");
    write_checkpoint(
        &c,
        &id,
        if complete { "captured" } else { "partial" },
        &body,
    )?;
    Ok(id)
}
fn connected(client: &str) -> bool {
    crate::store::open()
        .ok()
        .and_then(|c| {
            c.query_row("SELECT at FROM reports WHERE client=?", [client], |r| {
                r.get::<_, u64>(0)
            })
            .ok()
        })
        .is_some_and(|at| crate::now().saturating_sub(at) < 15000)
}
fn restore_entry(entry: &Value, mut record_job: impl FnMut(&str) -> R<()>) -> R<Value> {
    let sid = entry["snapshotId"].as_str().ok_or("Snapshot missing")?;
    let kind = entry["kind"].as_str().ok_or("Adapter missing")?;
    if ["explorer", "terminal"].contains(&kind) {
        return crate::context::restore_context_item(
            sid,
            kind,
            entry["index"].as_u64().unwrap_or(0) as usize,
        );
    }
    let snapshot = crate::store::snapshot(&crate::store::open()?, sid)?;
    let client = snapshot["client"]
        .as_str()
        .ok_or("Integration identity missing")?;
    if !connected(client) && kind == "vscode" {
        crate::context::open_vscode_snapshot(sid)?;
    } else if !connected(client) && kind == "chrome" {
        crate::context::open_chrome_profile(client)?;
    }
    if !connected(client) && ["vscode", "chrome"].contains(&kind) {
        let end = Instant::now() + Duration::from_secs(40);
        while !connected(client) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    if !connected(client) {
        return Err("The original integration did not reconnect after its cold-start launch. Inspect the saved Chrome profile binding or VS Code workspace before retrying this part.".into());
    }
    let id = crate::store::enqueue(client.into(), "resume".into(), None, Some(sid.into()))?;
    record_job(&id)?;
    let done = await_job(&id)?;
    Ok(
        json!({"status":"observed","snapshotState":crate::store::snapshot(&crate::store::open()?,sid)?["state"],"adapterResult":done["result"]}),
    )
}
fn selected_steps(body: &Value, selected: &[String]) -> R<Vec<Value>> {
    let entries = body["entries"]
        .as_array()
        .ok_or("Checkpoint cannot be read")?;
    let unique: HashSet<_> = selected.iter().collect();
    if selected.is_empty()
        || unique.len() != selected.len()
        || selected
            .iter()
            .any(|id| id != "services" && !entries.iter().any(|e| e["id"] == *id))
    {
        return Err("Select valid, distinct checkpoint parts".into());
    }
    if selected.iter().any(|id| id == "services")
        && body["services"].as_array().is_none_or(Vec::is_empty)
    {
        return Err("Checkpoint has no saved service expectations".into());
    }
    let mut steps = entries
        .iter()
        .map(|e| {
            let mut s = e.clone();
            s["state"] = json!(if selected.iter().any(|id| e["id"] == *id) {
                "pending"
            } else {
                "skipped"
            });
            s
        })
        .collect::<Vec<_>>();
    if body["services"].as_array().is_some_and(|s| !s.is_empty()) {
        steps.push(json!({"id":"services","kind":"services","label":"Saved service endpoints","state":if selected.iter().any(|id|id=="services"){"pending"}else{"skipped"}}));
    }
    Ok(steps)
}
fn readiness(checkpoint: &Value, body: &Value) -> Value {
    let mut facts = Vec::new();
    let mut ready = true;
    for step in body["steps"].as_array().into_iter().flatten() {
        let kind = step["kind"].as_str().unwrap_or("unknown");
        let skipped = step["state"] == "skipped";
        let verified = if skipped {
            false
        } else {
            match kind {
                "chrome" | "vscode" => {
                    step["state"] == "observed"
                        && step["result"]["adapterResult"]["verified"] == true
                }
                "services" => step["state"] == "observed" && step["result"]["verified"] == true,
                "explorer" => {
                    step["state"] == "observed" && step["result"]["status"] == "folder-verified"
                }
                "terminal" => {
                    step["state"] == "observed" && step["result"]["status"] == "cwd-verified"
                }
                _ => false,
            }
        };
        ready &= verified;
        facts.push(json!({"capability":step["label"],"kind":kind,"status":if verified{"verified"}else if skipped{"not-restored"}else{"unverified"}}));
    }
    for capture in checkpoint["captures"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|capture| capture["state"] != "captured")
    {
        ready = false;
        let label = capture["target"]
            .as_str()
            .or_else(|| capture["client"].as_str())
            .unwrap_or("Application context");
        facts.push(json!({"capability":format!("Capture - {label}"),"kind":"capture","status":"unverified"}));
    }
    json!({"status":if ready{"ready"}else{"incomplete"},"verified":ready,"facts":facts,"checkedAt":crate::now(),"detail":if ready{"Every restorable capability recorded in this checkpoint was reconstructed and reported its required V1 verification evidence."}else{"At least one checkpoint capability was not restored, failed capture, or lacks required verification evidence. Processing completion alone is not project readiness."}})
}
fn write_run(c: &Connection, id: &str, state: &str, body: &Value) -> R<()> {
    c.execute(
        "UPDATE reconstruction_runs SET state=?,body=? WHERE id=?",
        params![state, crate::protection::encode(body)?, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn restore(id: &str, selected: Vec<String>, acknowledged_services: bool) -> R<String> {
    if selected.iter().any(|part| part == "services") && !acknowledged_services {
        return Err("Review the saved service commands and explicitly approve restarting them from this checkpoint.".into());
    }
    let _guard = OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    let c = crate::store::open()?;
    let checkpoint = get(&c, id)?;
    let mut body = json!({"steps":selected_steps(&checkpoint,&selected)?,"detail":"Each result describes observed capabilities. Completed processing does not mean all working state was restored."});
    let run = uuid::Uuid::new_v4().to_string();
    c.execute(
        "INSERT INTO reconstruction_runs VALUES(?,?,?,'running',?)",
        params![run, id, crate::now(), crate::protection::encode(&body)?],
    )
    .map_err(|e| e.to_string())?;
    for i in 0..body["steps"].as_array().unwrap().len() {
        if body["steps"][i]["state"] == "skipped" {
            continue;
        }
        body["steps"][i]["state"] = json!("running");
        write_run(&c, &run, "running", &body)?;
        let step = body["steps"][i].clone();
        let result = if step["kind"] == "services" {
            crate::services::restart_checkpoint(
                checkpoint["services"]
                    .as_array()
                    .ok_or("Service expectations missing")?,
            )
        } else {
            restore_entry(&step, |job| {
                body["steps"][i]["jobId"] = json!(job);
                write_run(&c, &run, "running", &body)
            })
        };
        match result {
            Ok(value) => {
                body["steps"][i]["state"] = json!("observed");
                body["steps"][i]["result"] = value;
            }
            Err(e) => {
                body["steps"][i]["state"] = json!("failed");
                body["steps"][i]["detail"] = json!(e);
            }
        }
        body["steps"][i]["checkedAt"] = json!(crate::now());
        write_run(&c, &run, "running", &body)?;
    }
    body["readiness"] = readiness(&checkpoint, &body);
    write_run(&c, &run, "finished", &body)?;
    Ok(run)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-checkpoints-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap()
    }
    #[test]
    fn selection_keeps_unselected_parts_visible_and_rejects_foreign_entries() {
        let b = json!({"entries":[{"id":"a"},{"id":"b"}],"services":[{"url":"http://localhost:3000/"}]});
        let s = selected_steps(&b, &["b".into()]).unwrap();
        assert_eq!(s[0]["state"], "skipped");
        assert_eq!(s[1]["state"], "pending");
        assert_eq!(s[2]["state"], "skipped");
        assert!(selected_steps(&b, &["foreign".into()]).is_err());
        assert!(selected_steps(&b, &["a".into(), "a".into()]).is_err());
    }
    #[test]
    fn checkpoint_expectations_survive_edits_and_recovery_never_replays() {
        let c = db();
        let p = crate::projects::active(&c).unwrap();
        let id = p["id"].as_str().unwrap();
        let body = json!({"project":p,"entries":[],"services":[{"url":"http://localhost:3000/"}],"captures":[{"state":"capturing"}]});
        c.execute(
            "INSERT INTO checkpoints VALUES('point',?,1,'capturing',?)",
            params![id, crate::protection::encode(&body).unwrap()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO reconstruction_runs VALUES('run','point',1,'running',?)",
            [crate::protection::encode(&json!({"steps":[{"state":"running"}]})).unwrap()],
        )
        .unwrap();
        crate::projects::save(&c, Some(id), "New name", "D:\\changed").unwrap();
        recover_at(&c).unwrap();
        let saved = list(&c, id).unwrap();
        assert_eq!(saved[0]["body"], body);
        assert_eq!(saved[0]["state"], "interrupted");
        assert_eq!(saved[0]["runs"][0]["state"], "interrupted");
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn readiness_requires_every_checkpoint_capability_not_just_the_selected_subset() {
        let checkpoint = json!({"captures":[{"state":"captured"},{"state":"captured"}]});
        let ready = readiness(
            &checkpoint,
            &json!({"steps":[
                {"kind":"vscode","label":"Editor","state":"observed","result":{"adapterResult":{"verified":true}}},
                {"kind":"services","label":"Services","state":"observed","result":{"verified":true}}
            ]}),
        );
        assert_eq!(ready["status"], "ready");
        assert_eq!(ready["verified"], true);

        let skipped = readiness(
            &checkpoint,
            &json!({"steps":[
                {"kind":"chrome","label":"Browser","state":"observed","result":{"adapterResult":{"verified":true}}},
                {"kind":"terminal","label":"Terminal","state":"skipped"}
            ]}),
        );
        assert_eq!(skipped["status"], "incomplete");
        assert_eq!(skipped["verified"], false);
        assert_eq!(skipped["facts"][1]["status"], "not-restored");

        let failed_capture = readiness(
            &json!({"captures":[{"state":"failed","target":"browser-window"}]}),
            &json!({"steps":[
                {"kind":"explorer","label":"Folder","state":"observed","result":{"status":"folder-verified"}}
            ]}),
        );
        assert_eq!(failed_capture["status"], "incomplete");
        assert_eq!(failed_capture["verified"], false);
        assert!(failed_capture["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["kind"] == "capture"));
    }
    #[test]
    fn checkpoint_import_remaps_ids_and_never_imports_runtime_authority() {
        let mut c = db();
        let project = crate::projects::active(&c).unwrap();
        let project_id = project["id"].as_str().unwrap();
        let old_snapshot = uuid::Uuid::new_v4().to_string();
        let old_service = uuid::Uuid::new_v4().to_string();
        let envelope = json!({
            "format":"project-hibernate-checkpoint",
            "version":1,
            "checkpoint":{
                "version":1,
                "project":project,
                "repository":{},
                "captures":[{"client":"profile","target":"window","state":"captured","snapshotId":old_snapshot,"jobId":"stale-job"}],
                "entries":[{"id":old_snapshot,"snapshotId":old_snapshot,"kind":"chrome","label":"Browser"}],
                "services":[{"id":old_service,"projectId":project_id,"name":"Web","url":"http://localhost:32123/","launchRun":{"state":"verified","identity":{"pid":42,"processStarted":"stale"}}}]
            },
            "snapshots":[{
                "id":old_snapshot,
                "client":"profile",
                "adapter":"chrome",
                "state":"closed",
                "created":1,
                "body":{"version":1,"adapter":"chrome","project":project,"tabs":[]},
                "before":{},
                "after":{},
                "result":{"closed":true}
            }]
        });
        let checkpoint_id = import_value(&mut c, &envelope).unwrap();
        let imported = list(&c, project_id)
            .unwrap()
            .into_iter()
            .find(|v| v["id"] == checkpoint_id)
            .unwrap();
        assert_eq!(imported["state"], "imported");
        assert_eq!(imported["body"]["captures"][0]["state"], "captured");
        assert!(imported["body"]["captures"][0].get("jobId").is_none());
        let new_snapshot = imported["body"]["entries"][0]["snapshotId"]
            .as_str()
            .unwrap();
        assert_ne!(new_snapshot, old_snapshot);
        assert_eq!(
            crate::store::snapshot(&c, new_snapshot).unwrap()["state"],
            "imported"
        );
        assert_ne!(imported["body"]["services"][0]["id"], old_service);
        assert!(imported["body"]["services"][0].get("launchRun").is_none());
        assert_eq!(crate::health::list(&c, project_id).unwrap().len(), 1);
        for table in ["jobs", "hibernation_runs", "reconstruction_runs"] {
            let count: u32 = c
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }
    #[test]
    fn checkpoint_import_refuses_unknown_format_versions() {
        let mut c = db();
        assert!(import_value(
            &mut c,
            &json!({"format":"project-hibernate-checkpoint","version":99})
        )
        .is_err());
    }
    #[test]
    fn context_entries_preserve_multiple_saved_terminal_working_directories() {
        let c = db();
        let project = crate::projects::active(&c).unwrap();
        let project_id = project["id"].as_str().unwrap();
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        let body = json!({
            "version":1,
            "adapter":"context",
            "project":project,
            "explorer":{"items":[]},
            "terminal":{"items":[
                {"cwd":"D:\\project","commandReplay":false},
                {"cwd":"D:\\project\\api","commandReplay":false}
            ]}
        });
        c.execute(
            "INSERT INTO snapshots VALUES(?,'local-context','context','captured',?,?,?,NULL,NULL)",
            params![
                snapshot_id,
                crate::now(),
                crate::protection::encode(&body).unwrap(),
                "{}"
            ],
        )
        .unwrap();
        let saved = entries(&c, project_id, &snapshot_id).unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0]["kind"], "terminal");
        assert_eq!(saved[0]["index"], 0);
        assert_eq!(saved[1]["index"], 1);
        assert!(saved[1]["label"].as_str().unwrap().contains("api"));
    }
    #[test]
    fn project_membership_is_checked_against_saved_snapshot() {
        let mut c = db();
        let s=crate::store::handle(&mut c,&json!({"op":"save","client":"profile","body":{"adapter":"chrome","version":1,"tabs":[]}})).unwrap();
        assert!(entries(&c, "wrong-project", s["id"].as_str().unwrap()).is_err());
        assert_eq!(
            entries(
                &c,
                s["body"]["project"]["id"].as_str().unwrap(),
                s["id"].as_str().unwrap()
            )
            .unwrap()
            .len(),
            1
        );
    }
}
