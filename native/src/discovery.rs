//! Bounded, read-only project suggestions from reported local folders. No crawl or inferred port ownership.
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
type R<T> = Result<T, String>;

fn folder(raw: &str) -> Option<std::path::PathBuf> {
    let path = if raw.to_ascii_lowercase().starts_with("file:") {
        let uri = url::Url::parse(raw).ok()?;
        if uri.host_str().is_some() || uri.query().is_some() || uri.fragment().is_some() {
            return None;
        }
        uri.to_file_path().ok()?
    } else {
        raw.into()
    };
    crate::context::local_folder(&path.to_string_lossy()).ok()
}
fn key(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn candidates(signals: &[Value], projects: &[Value]) -> Vec<Value> {
    let mut found: BTreeMap<String, Value> = BTreeMap::new();
    let mut roots: BTreeMap<String, Option<Value>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for signal in signals.iter().take(300) {
        if !matches!(
            signal["source"].as_str(),
            Some("vscode" | "terminal" | "explorer")
        ) {
            continue;
        }
        let Some(path) = signal["path"].as_str().and_then(folder) else {
            continue;
        };
        let path_key = key(&path);
        if !roots.contains_key(&path_key) {
            if roots.len() >= 40 {
                continue;
            }
            roots.insert(
                path_key.clone(),
                crate::repository::inspect(&path.to_string_lossy()).ok(),
            );
        }
        let Some(repo) = roots.get(&path_key).and_then(Option::as_ref) else {
            continue;
        };
        let Some(root) = repo["root"].as_str().map(std::path::Path::new) else {
            continue;
        };
        let root_key = key(root);
        let candidate=found.entry(root_key.clone()).or_insert_with(||{
            let existing=projects.iter().find(|p|p["projectPath"].as_str().and_then(folder).is_some_and(|p|key(&p)==root_key));
            json!({"name":root.file_name().map(|s|s.to_string_lossy().into_owned()).unwrap_or_else(||root.to_string_lossy().into_owned()),"projectPath":root.to_string_lossy(),"existingProjectId":existing.and_then(|p|p["id"].as_str()),"repository":repo,"evidence":[]})
        });
        // One source can report the same folder repeatedly (for example several shells).
        if seen.insert((root_key, signal["source"].to_string(), path_key)) {
            candidate["evidence"].as_array_mut().unwrap().push(json!({"source":signal["source"],"label":signal["label"].as_str().unwrap_or("Reported folder").chars().take(200).collect::<String>(),"path":path.to_string_lossy()}));
        }
    }
    found.into_values().collect()
}
pub fn discover() -> R<Value> {
    let c = crate::store::open()?;
    let explorer = crate::context::explorer().unwrap_or_else(|e| json!({"items":[],"error":e}));
    discover_with(&c, &explorer)
}
fn discover_with(c: &Connection, explorer: &Value) -> R<Value> {
    let mut q = c
        .prepare("SELECT body FROM reports WHERE at>? ORDER BY at DESC LIMIT 100")
        .map_err(|e| e.to_string())?;
    let reports = q
        .query_map([crate::now().saturating_sub(15000)], |r| {
            crate::protection::sql_decode(r.get(0)?)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut signals: Vec<Value> = reports
        .iter()
        .flat_map(|r| r["signals"].as_array().into_iter().flatten())
        .filter(|s| {
            matches!(
                s["source"].as_str(),
                Some("vscode" | "terminal" | "explorer")
            )
        })
        .take(200)
        .cloned()
        .collect();
    signals.extend(
        explorer["items"]
            .as_array()
            .into_iter()
            .flatten()
            .take(50)
            .map(|i| json!({"source":"explorer","label":"Open Explorer folder","path":i["url"]})),
    );
    signals.extend(crate::terminal::live_contexts(c)?.iter().take(50).map(|i|json!({"source":"terminal","label":"Instrumented shell: last prompt folder","path":i["cwd"]})));
    Ok(
        json!({"checkedAt":crate::now(),"candidates":candidates(&signals,&crate::projects::list(c)?),"explorerError":explorer["error"],"detail":"Suggestions use up to 40 reported local folders and their Git ancestors. No recursive disk scan, commands or automatic assignments. A localhost URL alone does not identify a project."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn repo() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hibernate-discovery-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(p.join(".git")).unwrap();
        std::fs::create_dir_all(p.join("frontend")).unwrap();
        std::fs::write(p.join(".git/HEAD"), "ref: refs/heads/discovery\n").unwrap();
        p
    }
    #[test]
    fn related_folders_group_into_one_known_repository_without_assignments() {
        let p = repo();
        let signals = vec![
            json!({"source":"vscode","path":url::Url::from_directory_path(&p).unwrap().to_string()}),
            json!({"source":"terminal","path":p.join("frontend")}),
            json!({"source":"explorer","path":p}),
        ];
        let found = candidates(&signals, &[json!({"id":"existing","projectPath":p})]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["existingProjectId"], "existing");
        assert_eq!(found[0]["evidence"].as_array().unwrap().len(), 3);
        assert_eq!(found[0]["repository"]["branch"], "discovery");
    }
    #[test]
    fn remote_browser_and_stale_reports_do_not_invent_projects() {
        let p = repo();
        assert!(candidates(
            &[
                json!({"source":"browser","url":"http://localhost:3000","path":p}),
                json!({"source":"vscode","path":"file://server/share"}),
                json!({"source":"terminal","path":"\\\\server\\share"})
            ],
            &[]
        )
        .is_empty());
        let c = crate::store::open_at(p.join("data/test.db")).unwrap();
        // A large browser session must not consume the folder-discovery input budget.
        let mut signals = vec![json!({"source":"browser","url":"http://localhost:3000/"}); 201];
        signals.push(json!({"source":"vscode","path":p}));
        let body = json!({"signals":signals});
        c.execute(
            "INSERT INTO reports VALUES('editor','vscode',1,?)",
            [crate::protection::encode(&body).unwrap()],
        )
        .unwrap();
        assert!(
            discover_with(&c, &json!({"items":[]})).unwrap()["candidates"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        c.execute("UPDATE reports SET at=?", [crate::now()])
            .unwrap();
        assert_eq!(
            discover_with(&c, &json!({"items":[]})).unwrap()["candidates"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(crate::projects::members(&c).unwrap().is_empty());
    }
}
