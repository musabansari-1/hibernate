//! Durable project identity and explicit membership. Runtime window IDs are observations only.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
type R<T> = Result<T, String>;

pub fn migrate(c: &Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,body TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS job_context(job_id TEXT PRIMARY KEY,project TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS memberships(client TEXT NOT NULL,target TEXT NOT NULL,project_id TEXT NOT NULL,PRIMARY KEY(client,target));
      CREATE TABLE IF NOT EXISTS services(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,body TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS project_activity(project_id TEXT PRIMARY KEY,at INTEGER NOT NULL,source TEXT NOT NULL);").map_err(|e|e.to_string())?;
    let legacy: Option<String> = c
        .query_row("SELECT body FROM meta WHERE key='workspace'", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if count == 0 {
        let value = legacy.map(|s|crate::protection::decode(&s)).transpose()?.unwrap_or_else(||json!({"id":uuid::Uuid::new_v4().to_string(),"name":"Developer workspace","projectPath":""}));
        let id = value["id"]
            .as_str()
            .ok_or("Legacy workspace identity is invalid")?;
        c.execute(
            "INSERT INTO projects VALUES(?,?)",
            params![id, value.to_string()],
        )
        .map_err(|e| e.to_string())?;
        c.execute(
            "INSERT OR REPLACE INTO meta VALUES('activeProject',?)",
            [id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn get(c: &Connection, id: &str) -> R<Value> {
    let s: String = c
        .query_row("SELECT body FROM projects WHERE id=?", [id], |r| r.get(0))
        .map_err(|_| "Project no longer exists".to_string())?;
    crate::protection::decode(&s)
}
pub fn active(c: &Connection) -> R<Value> {
    let id: String = c
        .query_row("SELECT body FROM meta WHERE key='activeProject'", [], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?;
    get(c, &id)
}
pub fn list(c: &Connection) -> R<Vec<Value>> {
    let mut q = c
        .prepare("SELECT p.body,(SELECT COUNT(*) FROM snapshot_index i WHERE i.project_id=p.id AND i.archived=0),a.at FROM projects p LEFT JOIN project_activity a ON a.project_id=p.id ORDER BY p.rowid")
        .map_err(|e| e.to_string())?;
    let result = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u64>(1)?,
                r.get::<_, Option<u64>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .map(|s| {
            let (raw, count, at) = s.map_err(|e| e.to_string())?;
            let mut p = crate::protection::decode(&raw)?;
            p["snapshotCount"] = json!(count);
            p["lastObservedActive"] = json!(at);
            Ok(p)
        })
        .collect();
    result
}
pub fn save(c: &Connection, id: Option<&str>, name: &str, path: &str) -> R<Value> {
    if name.trim().is_empty()
        || name.len() > 200
        || path.len() > 32700
        || path.contains(['\0', '\n', '\r'])
    {
        return Err("Enter a project name and valid folder path".into());
    }
    let mut value = if let Some(id) = id {
        get(c, id)?
    } else {
        json!({"id":uuid::Uuid::new_v4().to_string()})
    };
    value["name"] = json!(name.trim());
    value["projectPath"] = json!(path);
    value["updatedAt"] = json!(crate::now());
    c.execute(
        "INSERT INTO projects VALUES(?,?) ON CONFLICT(id) DO UPDATE SET body=excluded.body",
        params![value["id"].as_str(), crate::protection::encode(&value)?],
    )
    .map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn select(c: &Connection, id: &str) -> R<Value> {
    let value = get(c, id)?;
    c.execute("UPDATE meta SET body=? WHERE key='activeProject'", [id])
        .map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn members(c: &Connection) -> R<Vec<Value>> {
    let mut q = c
        .prepare("SELECT client,target,project_id FROM memberships")
        .map_err(|e| e.to_string())?;
    let result=q.query_map([],|r|Ok(json!({"client":r.get::<_,String>(0)?,"target":r.get::<_,String>(1)?,"projectId":r.get::<_,String>(2)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string());
    result
}
pub fn assign(c: &Connection, client: &str, target: &str, project: Option<&str>) -> R<()> {
    if client.is_empty() || client.len() > 200 || target.is_empty() || target.len() > 200 {
        return Err("Invalid application identity".into());
    }
    if let Some(id) = project {
        get(c, id)?;
        c.execute("INSERT INTO memberships VALUES(?,?,?) ON CONFLICT(client,target) DO UPDATE SET project_id=excluded.project_id",params![client,target,id]).map_err(|e|e.to_string())?;
    } else {
        c.execute(
            "DELETE FROM memberships WHERE client=? AND target=?",
            params![client, target],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// Only an explicit, session-correct window membership establishes project activity.
// Missing focus reports never imply inactivity (offline integrations and polling have gaps).
pub fn observe_activity(c: &Connection, client: &str, body: &Value) -> R<()> {
    for item in body["items"].as_array().into_iter().flatten().take(200) {
        if item["focused"] != true {
            continue;
        }
        let Some(target) = item.get("membershipKey").unwrap_or(&item["id"]).as_str() else {
            continue;
        };
        c.execute("INSERT INTO project_activity(project_id,at,source) SELECT project_id,?,? FROM memberships WHERE client=? AND target=? ON CONFLICT(project_id) DO UPDATE SET at=excluded.at,source=excluded.source", params![crate::now(),client,client,target]).map_err(|e|e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_observation_requires_explicit_session_membership() {
        let c = crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-activity-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let p = active(&c).unwrap();
        let id = p["id"].as_str().unwrap();
        let focused = json!({"items":[{"id":"1","membershipKey":"new-session:1","focused":true}]});
        observe_activity(&c, "browser", &focused).unwrap();
        assert!(list(&c).unwrap()[0]["lastObservedActive"].is_null());
        assign(&c, "browser", "old-session:1", Some(id)).unwrap();
        assign(&c, "browser", "1", Some(id)).unwrap();
        observe_activity(&c, "browser", &focused).unwrap();
        assert!(list(&c).unwrap()[0]["lastObservedActive"].is_null());
        assign(&c, "browser", "new-session:1", Some(id)).unwrap();
        observe_activity(
            &c,
            "browser",
            &json!({"items":[{"id":"1","membershipKey":"new-session:1","focused":false}]}),
        )
        .unwrap();
        assert!(list(&c).unwrap()[0]["lastObservedActive"].is_null());
        observe_activity(&c, "browser", &focused).unwrap();
        assert!(list(&c).unwrap()[0]["lastObservedActive"].as_u64().unwrap() > 0);
    }
    #[test]
    fn project_switch_and_membership_survive_reopen() {
        let p =
            std::env::temp_dir().join(format!("hibernate-projects-{}.db", uuid::Uuid::new_v4()));
        let c = crate::store::open_at(p.clone()).unwrap();
        let first = active(&c).unwrap();
        let second = save(&c, None, "Research", "D:\\research").unwrap();
        let id = second["id"].as_str().unwrap();
        assign(&c, "chrome-profile", "window-session", Some(id)).unwrap();
        select(&c, id).unwrap();
        drop(c);
        let c = crate::store::open_at(p).unwrap();
        assert_eq!(active(&c).unwrap()["id"], id);
        assert_eq!(list(&c).unwrap().len(), 2);
        assert_eq!(members(&c).unwrap()[0]["projectId"], id);
        assert_eq!(
            get(&c, first["id"].as_str().unwrap()).unwrap()["name"],
            "Developer workspace"
        );
    }
}
