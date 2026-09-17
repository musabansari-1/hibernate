//! Instrumented PowerShell contexts launched in Windows Terminal. No captured command execution.
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
type R<T> = Result<T, String>;
pub fn migrate(c: &rusqlite::Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS terminal_sessions(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,pid INTEGER,body TEXT NOT NULL,updated INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS terminal_members(session_id TEXT PRIMARY KEY,client TEXT NOT NULL);").map_err(|e|e.to_string())
}
pub(crate) fn bridge() -> R<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    for base in exe.ancestors().skip(1) {
        for relative in [
            "hibernate-host.exe",
            "native/target/debug/hibernate-host.exe",
            "native/target/release/hibernate-host.exe",
        ] {
            let p = base.join(relative);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err("Native terminal bridge is missing. Install hibernate-host.exe alongside the desktop application.".into())
}
pub fn report(session: &str, pid: u32, path: &str) -> R<()> {
    let (actual, exe) =
        crate::platform::parent_process().ok_or("Cannot identify terminal reporter")?;
    if actual != pid
        || !exe
            .to_lowercase()
            .ends_with("\\windowspowershell\\v1.0\\powershell.exe")
    {
        return Err(
            "Terminal reports must come from the launched Windows PowerShell process".into(),
        );
    }
    let cwd = PathBuf::from(path);
    if !cwd.is_absolute() || !cwd.is_dir() {
        return Err("Terminal working directory is unavailable".into());
    }
    let c = crate::store::open()?;
    let process_started =
        crate::platform::process_started(pid).ok_or("Cannot observe terminal process identity")?;
    let original: Option<u32> = c
        .query_row(
            "SELECT pid FROM terminal_sessions WHERE id=?",
            [session],
            |r| r.get(0),
        )
        .map_err(|_| "Unknown terminal session")?;
    if original.is_some_and(|p| p != pid) {
        return Err("Terminal session is already bound to a different process".into());
    }
    if original.is_some() {
        let body: String = c
            .query_row(
                "SELECT body FROM terminal_sessions WHERE id=?",
                [session],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if crate::protection::decode(&body)?["processStarted"] != process_started {
            return Err(
                "Terminal process identity changed. Open a new instrumented terminal context."
                    .into(),
            );
        }
    }
    let value = json!({"id":session,"pid":pid,"processStarted":process_started,"cwd":path,"shell":"Windows PowerShell -NoProfile","observedAt":crate::now(),"detail":"Working directory observed at a shell prompt. Running commands and terminal history are not captured."});
    c.execute(
        "UPDATE terminal_sessions SET pid=?,body=?,updated=? WHERE id=?",
        params![
            pid,
            crate::protection::encode(&value)?,
            crate::now(),
            session
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
struct Launch {
    id: String,
    shell: PathBuf,
    args: Vec<String>,
}
fn prepare(folder: &Path, project_id: Option<&str>) -> R<Launch> {
    let native = bridge()?;
    let base = crate::store::path()?
        .parent()
        .ok_or("Missing data folder")?
        .join("runtime");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let script = base.join("terminal-context-v1.ps1");
    let expected = include_str!("../../scripts/terminal-context.ps1");
    if script.exists() {
        if std::fs::read_to_string(&script).map_err(|e| e.to_string())? != expected {
            return Err("Terminal helper differs from this build. Reinstall its runtime helper before launching.".into());
        }
    } else {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script)
            .map_err(|e| e.to_string())?;
        f.write_all(expected.as_bytes())
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    let c = crate::store::open()?;
    let project = match project_id {
        Some(id) => crate::projects::get(&c, id)?,
        None => crate::projects::active(&c)?,
    };
    let id = uuid::Uuid::new_v4().to_string();
    c.execute(
        "INSERT INTO terminal_sessions VALUES(?,?,NULL,?,?)",
        params![
            id,
            project["id"].as_str(),
            crate::protection::encode(
                &json!({"id":id,"cwd":folder.to_string_lossy(),"status":"launch-pending"})
            )?,
            crate::now()
        ],
    )
    .map_err(|e| e.to_string())?;
    let system = PathBuf::from(std::env::var_os("WINDIR").ok_or("WINDIR missing")?);
    Ok(Launch {
        id: id.clone(),
        shell: system.join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        args: vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NoExit".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            script.to_string_lossy().into_owned(),
            "-BridgePath".into(),
            native.to_string_lossy().into_owned(),
            "-SessionId".into(),
            id,
            "-ProjectDirectory".into(),
            folder.to_string_lossy().into_owned(),
            "-DataDirectory".into(),
            crate::store::path()?
                .parent()
                .ok_or("Missing data folder")?
                .to_string_lossy()
                .into_owned(),
        ],
    })
}
pub fn prepare_vscode(
    c: &rusqlite::Connection,
    client: &str,
    snapshot_id: &str,
    kind: &str,
    index: usize,
) -> R<Value> {
    let snapshot = crate::store::snapshot(c, snapshot_id)?;
    if snapshot["client"] != client
        || snapshot["adapter"] != "vscode"
        || snapshot["body"]["version"] != 1
    {
        return Err("Terminal definition must belong to this VS Code integration".into());
    }
    let raw = match kind {
        "folder" => snapshot["body"]["folders"][index].as_str(),
        "terminal" => snapshot["body"]["terminals"][index]["cwd"].as_str(),
        _ => None,
    }
    .ok_or("Saved terminal directory is unknown")?;
    let uri = url::Url::parse(raw).map_err(|e| e.to_string())?;
    if uri.scheme() != "file" || uri.host_str().is_some() {
        return Err("Only local-drive terminal contexts are supported".into());
    }
    let folder = uri.to_file_path().map_err(|_| "Invalid local folder URI")?;
    let folder = crate::context::local_folder(&folder.to_string_lossy())?;
    let launch = prepare(&folder, snapshot["body"]["project"]["id"].as_str())?;
    c.execute(
        "INSERT INTO terminal_members VALUES(?,?)",
        params![launch.id, client],
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({"id":launch.id,"shellPath":launch.shell,"shellArgs":launch.args,"cwd":folder}))
}
pub fn observations(c: &rusqlite::Connection, client: &str) -> R<Vec<Value>> {
    let mut q=c.prepare("SELECT s.body FROM terminal_sessions s JOIN terminal_members m ON m.session_id=s.id WHERE m.client=? AND s.pid IS NOT NULL ORDER BY s.updated DESC LIMIT 200").map_err(|e|e.to_string())?;
    let values = q
        .query_map([client], |r| crate::protection::sql_decode(r.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(live(values))
}
// A prompt report proves a past cwd observation, not present idleness or window ownership.
fn live_identity(value: &Value) -> bool {
    value["pid"]
        .as_u64()
        .and_then(|p| u32::try_from(p).ok())
        .and_then(crate::platform::process_started)
        .is_some_and(|started| value["processStarted"] == started)
}
fn normalized_cwd(path: &str) -> String {
    path.replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn restored_prompt(value: &Value, session: &str, expected: &str) -> bool {
    value["id"] == session
        && live_identity(value)
        && value["cwd"]
            .as_str()
            .is_some_and(|p| normalized_cwd(p) == normalized_cwd(expected))
}
fn live(values: Vec<Value>) -> Vec<Value> {
    values.into_iter().filter(live_identity).collect()
}
pub fn live_contexts(c: &rusqlite::Connection) -> R<Vec<Value>> {
    let mut q=c.prepare("SELECT body FROM terminal_sessions WHERE pid IS NOT NULL ORDER BY updated DESC LIMIT 200").map_err(|e|e.to_string())?;
    let values = q
        .query_map([], |r| crate::protection::sql_decode(r.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(live(values))
}
pub fn live_contexts_for_project(c: &rusqlite::Connection, project_id: &str) -> R<Vec<Value>> {
    let mut q=c.prepare("SELECT body FROM terminal_sessions WHERE project_id=? AND pid IS NOT NULL ORDER BY updated DESC LIMIT 100").map_err(|e|e.to_string())?;
    let values = q
        .query_map([project_id], |r| crate::protection::sql_decode(r.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(live(values))
}
pub fn restore_for(folder: &Path, project_id: Option<&str>) -> R<Value> {
    let launch = prepare(folder, project_id)?;
    let id = launch.id;
    let c = crate::store::open()?;
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA missing")?);
    std::process::Command::new(local.join("Microsoft/WindowsApps/wt.exe"))
        .args(["-w", "new", "new-tab", "-d"])
        .arg(folder)
        .arg(launch.shell)
        .args(launch.args)
        .spawn()
        .map_err(|e| format!("Windows Terminal launch failed: {e}"))?;
    let expected = folder.to_string_lossy().into_owned();
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(200));
        let raw: Option<String> = c
            .query_row(
                "SELECT body FROM terminal_sessions WHERE id=? AND pid IS NOT NULL",
                [&id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(raw) = raw {
            let actual = crate::protection::decode(&raw)?;
            if restored_prompt(&actual, &id, &expected) {
                return Ok(
                    json!({"status":"cwd-verified","expected":1,"observed":1,"sessionId":id,"pid":actual["pid"],"cwd":actual["cwd"],"detail":"Windows Terminal opened an instrumented PowerShell prompt in the expected working directory. No prior commands, panes or history were restored."}),
                );
            }
        }
    }
    Ok(
        json!({"status":"launched-unverified","sessionId":id,"detail":"Windows Terminal launch requested, but the expected shell working-directory handshake was not observed."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restored_prompt_requires_exact_session_cwd_and_live_nontruncated_pid() {
        let pid = std::process::id();
        let mut value = json!({"id":"fresh","pid":pid,"processStarted":crate::platform::process_started(pid).unwrap(),"cwd":"D:\\Fixture\\"});
        assert!(restored_prompt(&value, "fresh", "d:\\fixture"));
        assert!(restored_prompt(&value, "fresh", "d:/fixture"));
        assert!(!restored_prompt(&value, "other", "d:\\fixture"));
        assert!(!restored_prompt(&value, "fresh", "d:\\other"));
        value["pid"] = json!(u64::from(pid) + (1u64 << 32));
        assert!(!restored_prompt(&value, "fresh", "d:\\fixture"));
        value["pid"] = json!(pid);
        value["processStarted"] = json!("reused-or-exited");
        assert!(!restored_prompt(&value, "fresh", "d:\\fixture"));
    }
    #[test]
    fn observations_require_client_and_live_process_creation_identity() {
        let c = crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-terminal-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let pid = std::process::id();
        let mut body =
            json!({"id":"session","pid":pid,"processStarted":"old-process","cwd":"D:\\fixture"});
        c.execute(
            "INSERT INTO terminal_sessions VALUES('session','project',?,?,1)",
            params![pid, crate::protection::encode(&body).unwrap()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO terminal_members VALUES('session','editor')",
            [],
        )
        .unwrap();
        assert!(observations(&c, "editor").unwrap().is_empty());
        body["processStarted"] = json!(crate::platform::process_started(pid).unwrap());
        c.execute(
            "UPDATE terminal_sessions SET body=?",
            [crate::protection::encode(&body).unwrap()],
        )
        .unwrap();
        assert_eq!(observations(&c, "editor").unwrap().len(), 1);
        assert!(observations(&c, "other-editor").unwrap().is_empty());
    }
    #[test]
    fn live_contexts_for_project_only_returns_verified_processes_for_that_project() {
        let c = crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-terminal-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let pid = std::process::id();
        let started = crate::platform::process_started(pid).unwrap();
        for (id, project, cwd) in [
            ("a", "project-a", "D:\\project-a"),
            ("b", "project-b", "D:\\project-b"),
        ] {
            let body = json!({"id":id,"pid":pid,"processStarted":started,"cwd":cwd});
            c.execute(
                "INSERT INTO terminal_sessions VALUES(?,?,?,?,1)",
                params![id, project, pid, crate::protection::encode(&body).unwrap()],
            )
            .unwrap();
        }
        let project_a = live_contexts_for_project(&c, "project-a").unwrap();
        assert_eq!(project_a.len(), 1);
        assert_eq!(project_a[0]["cwd"], "D:\\project-a");
        assert!(live_contexts_for_project(&c, "missing-project")
            .unwrap()
            .is_empty());
    }
    #[test]
    fn terminal_launch_definition_cannot_cross_clients_or_guess_unknown_cwd() {
        let mut c = crate::store::open_at(
            std::env::temp_dir().join(format!("hibernate-terminal-{}.db", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let saved=crate::store::handle(&mut c,&json!({"op":"save","client":"editor","body":{"version":1,"adapter":"vscode","folders":["file://server/share"],"terminals":[{"cwd":null}]}})).unwrap();
        let id = saved["id"].as_str().unwrap();
        assert!(prepare_vscode(&c, "other", id, "folder", 0).is_err());
        assert!(prepare_vscode(&c, "editor", id, "terminal", 0).is_err());
        assert!(prepare_vscode(&c, "editor", id, "folder", 0).is_err());
    }
}
