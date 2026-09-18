//! Snapshot history and account-bound portable backups. Import never closes or launches anything.
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::Path,
};
type R<T> = Result<T, String>;
pub fn migrate(c: &Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS snapshot_index(id TEXT PRIMARY KEY,project_id TEXT,archived INTEGER NOT NULL DEFAULT 0);
 CREATE INDEX IF NOT EXISTS snapshot_project ON snapshot_index(project_id,archived);").map_err(|e|e.to_string())?;
    let mut q=c.prepare("SELECT s.id,s.body FROM snapshots s LEFT JOIN snapshot_index i ON i.id=s.id WHERE i.id IS NULL").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (id, raw) in rows {
        let body = crate::protection::decode(&raw).ok();
        index(
            c,
            &id,
            body.as_ref().and_then(|b| b["project"]["id"].as_str()),
        )?;
    }
    Ok(())
}
pub fn index(c: &Connection, id: &str, project: Option<&str>) -> R<()> {
    c.execute(
        "INSERT OR IGNORE INTO snapshot_index(id,project_id) VALUES(?,?)",
        params![id, project],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn list(c: &Connection, project: &str, limit: u32, archived: bool) -> R<Vec<Value>> {
    let mut q=c.prepare("SELECT s.id,s.created FROM snapshots s JOIN snapshot_index i ON i.id=s.id WHERE (i.project_id=? OR i.project_id IS NULL) AND i.archived=? ORDER BY s.created DESC,s.rowid DESC LIMIT ?").map_err(|e|e.to_string())?;
    let ids = q
        .query_map(params![project, archived, limit.clamp(1, 500)], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids.into_iter().map(|(id,created)|{let mut s=crate::store::snapshot(c,&id).unwrap_or_else(|error|json!({"id":id,"created":created,"adapter":"unreadable","state":"unreadable","client":"","body":{"error":error},"before":null,"after":null,"result":null}));s["archived"]=json!(archived);s}).collect())
}
pub fn archive(id: &str, archived: bool) -> R<()> {
    let c = crate::store::open()?;
    let count = c
        .execute(
            "UPDATE snapshot_index SET archived=? WHERE id=?",
            params![archived, id],
        )
        .map_err(|e| e.to_string())?;
    if count != 1 {
        return Err("Snapshot not found".into());
    }
    Ok(())
}
pub fn export(id: &str) -> R<String> {
    let c = crate::store::open()?;
    let snapshot = crate::store::snapshot(&c, id)?;
    let payload = crate::protection::encode(
        &json!({"format":"project-hibernate-snapshot","version":1,"exportedAt":crate::now(),"snapshot":snapshot}),
    )?;
    let dir = crate::store::path()?
        .parent()
        .ok_or("Missing data folder")?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(format!(
        "{}-{}.hibernate",
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
        return Err("Choose an absolute path to a .hibernate snapshot backup".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 4_000_000 {
        return Err("Backup is too large".into());
    }
    let mut raw = String::new();
    file.take(4_000_001)
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
    if envelope["format"] != "project-hibernate-snapshot" || envelope["version"] != 1 {
        return Err("Unsupported backup format".into());
    }
    let s = &envelope["snapshot"];
    let adapter = s["adapter"]
        .as_str()
        .filter(|a| ["chrome", "vscode", "context"].contains(a))
        .ok_or("Unsupported snapshot adapter")?;
    if s["body"]["version"] != 1 || s["body"]["adapter"] != adapter {
        return Err("Unsupported or mismatched adapter schema".into());
    }
    let client = s["client"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 200)
        .ok_or("Missing integration identity")?;
    let project = &s["body"]["project"];
    let project_id = project["id"].as_str().ok_or("Missing project identity")?;
    uuid::Uuid::parse_str(project_id).map_err(|_| "Invalid project identity")?;
    if project["name"]
        .as_str()
        .is_none_or(|s| s.is_empty() || s.len() > 200)
        || project["projectPath"]
            .as_str()
            .is_none_or(|s| s.len() > 32700)
    {
        return Err("Invalid project metadata".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO projects VALUES(?,?)",
        params![project_id, crate::protection::encode(project)?],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO snapshots VALUES(?,?,?,'imported',?,?,?,NULL,?)",params![id,client,adapter,crate::now(),crate::protection::encode(&s["body"])?,s["before"].to_string(),crate::protection::encode(&json!({"detail":"Imported backup. No close authority or pending commands were imported; restoration requires your action."}))?]).map_err(|e|e.to_string())?;
    index(&tx, &id, Some(project_id))?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}
pub fn delete(id: &str, confirmation: &str) -> R<()> {
    if confirmation != id {
        return Err("Confirm the exact snapshot ID before permanent deletion".into());
    }
    let mut c = crate::store::open()?;
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let active: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM jobs WHERE snapshot_id=? AND state IN ('queued','running')",
            [id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if active > 0 {
        return Err("Wait for the pending snapshot operation before deleting it".into());
    }
    tx.execute("DELETE FROM snapshot_index WHERE id=?", [id])
        .map_err(|e| e.to_string())?;
    if tx
        .execute("DELETE FROM snapshots WHERE id=?", [id])
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("Snapshot not found".into());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-history-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap()
    }
    #[test]
    fn import_preserves_context_without_importing_close_authority() {
        let mut c = db();
        let project = crate::projects::active(&c).unwrap();
        let value = json!({"format":"project-hibernate-snapshot","version":1,"snapshot":{"adapter":"chrome","client":"profile","state":"approved","body":{"version":1,"adapter":"chrome","project":project,"tabs":[]},"before":{}}});
        let id = import_value(&mut c, &value).unwrap();
        let restored = crate::store::snapshot(&c, &id).unwrap();
        assert_eq!(restored["state"], "imported");
        assert_eq!(restored["body"], value["snapshot"]["body"]);
        assert_eq!(
            list(&c, project["id"].as_str().unwrap(), 30, false)
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn unknown_backup_versions_are_refused() {
        let mut c = db();
        assert!(import_value(
            &mut c,
            &json!({"format":"project-hibernate-snapshot","version":99})
        )
        .is_err());
    }
    #[test]
    fn history_is_project_scoped_and_survives_one_corrupt_snapshot() {
        let mut c = db();
        let p = crate::projects::active(&c).unwrap();
        for _ in 0..2 {
            let s = crate::store::handle(
                &mut c,
                &json!({"op":"save","client":"a","body":{"version":1,"adapter":"chrome"}}),
            )
            .unwrap();
            index(&c, s["id"].as_str().unwrap(), p["id"].as_str()).unwrap();
        }
        let first = list(&c, p["id"].as_str().unwrap(), 30, false).unwrap()[0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        c.execute("UPDATE snapshots SET body='broken' WHERE id=?", [first])
            .unwrap();
        let found = list(&c, p["id"].as_str().unwrap(), 30, false).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(
            found.iter().filter(|s| s["state"] == "unreadable").count(),
            1
        );
        assert!(list(&c, "another-project", 30, false).unwrap().is_empty());
    }
}
