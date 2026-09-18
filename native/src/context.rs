//! Supported Shell automation exposes folders, but does not prove all Windows 11 tabs were captured.
use serde_json::{json, Value};
use windows::{
    core::Interface,
    Win32::{
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL,
                COINIT_APARTMENTTHREADED,
            },
            Variant::VARIANT,
        },
        UI::Shell::{IShellWindows, IWebBrowser2, ShellWindows},
    },
};
pub fn explorer() -> Result<Value, String> {
    // COM objects never cross a thread boundary. An independent STA also avoids Tauri's apartment mode.
    std::thread::spawn(||unsafe{
        CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?;
        struct Apartment;impl Drop for Apartment{fn drop(&mut self){unsafe{CoUninitialize()}}}let _apartment=Apartment;
        let shell:IShellWindows=CoCreateInstance(&ShellWindows,None,CLSCTX_ALL).map_err(|e|e.to_string())?;
        let mut items=vec![];
        for index in 0..shell.Count().map_err(|e|e.to_string())?{
            if let Ok(dispatch)=shell.Item(&VARIANT::from(index)){
                if let Ok(browser)=dispatch.cast::<IWebBrowser2>(){
                    if let (Ok(url),Ok(hwnd))=(browser.LocationURL(),browser.HWND()){
                        let url=url.to_string();if url.to_lowercase().starts_with("file:///"){items.push(json!({"handle":hwnd.0.to_string(),"url":url,"compatibility":"Context Restore","limitation":"Only Shell-exposed folder context; Explorer tabs and selection are not guaranteed. Automatic close is blocked."}));}
                    }
                }
            }
        }
        Ok(json!({"items":items,"error":null}))
    }).join().map_err(|_|"Explorer enumeration thread failed".to_string())?
}
pub(crate) fn local_folder(path: &str) -> Result<std::path::PathBuf, String> {
    if path.len() > 32700
        || path.contains(['\0', '\r', '\n', ';'])
        || path.as_bytes().get(1).is_none_or(|b| *b != b':')
    {
        return Err("Choose an absolute local drive folder without command separators.".into());
    }
    let p = std::path::PathBuf::from(path);
    if !p.is_absolute() || !p.is_dir() {
        return Err("Project folder does not exist or is not an absolute local path.".into());
    }
    Ok(p)
}
fn normalized_local_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
pub fn restore_folder(kind: &str, path: &str) -> Result<Value, String> {
    restore_folder_for(kind, path, None)
}
fn restore_folder_for(kind: &str, path: &str, project: Option<&str>) -> Result<Value, String> {
    let folder = local_folder(path)?;
    if kind == "terminal" {
        return crate::terminal::restore_for(&folder, project);
    }
    let mut command = match kind {
        "explorer" => {
            let root = std::env::var_os("WINDIR").ok_or("WINDIR missing")?;
            let mut c =
                std::process::Command::new(std::path::PathBuf::from(root).join("explorer.exe"));
            c.arg(&folder);
            c
        }
        _ => return Err("Unsupported context type".into()),
    };
    command
        .spawn()
        .map_err(|e| format!("Could not launch {kind}: {e}"))?;
    Ok(
        json!({"status":"launched-unverified","detail":format!("{kind} launch requested for {}. Working directory, shell readiness and application state have not been verified.",folder.display())}),
    )
}
pub fn capture_context() -> Result<String, String> {
    let c = crate::store::open()?;
    let project = crate::store::workspace(&c)?;
    capture_context_for(project["id"].as_str().ok_or("Missing project")?)
}
pub fn capture_context_for(project_id: &str) -> Result<String, String> {
    let c = crate::store::open()?;
    let project = crate::projects::get(&c, project_id)?;
    let root_path = local_folder(
        project["projectPath"]
            .as_str()
            .ok_or("Set the project folder first")?,
    )?;
    let root = normalized_local_path(&root_path);
    let mut folders = explorer()?;
    if let Some(items) = folders["items"].as_array_mut() {
        items.retain(|item| {
            item["url"]
                .as_str()
                .and_then(|s| url::Url::parse(s).ok())
                .and_then(|u| u.to_file_path().ok())
                .is_some_and(|p| {
                    let path = normalized_local_path(&p);
                    path == root || path.starts_with(&(root.clone() + "\\"))
                })
        });
    }
    let mut terminal_items: Vec<Value> =
        crate::terminal::live_contexts_for_project(&c, project_id)?
            .into_iter()
            .filter_map(|item| {
                let cwd = item["cwd"].as_str()?;
                let folder = local_folder(cwd).ok()?;
                let normalized = normalized_local_path(&folder);
                if normalized != root && !normalized.starts_with(&(root.clone() + "\\")) {
                    return None;
                }
                Some(json!({
                    "cwd": folder.to_string_lossy(),
                    "shell": "Windows PowerShell -NoProfile",
                    "source": "verified instrumented terminal prompt",
                    "verifiedPrompt": true,
                    "commandReplay": false
                }))
            })
            .take(20)
            .collect();
    if terminal_items.is_empty() {
        terminal_items.push(json!({
            "cwd": root_path.to_string_lossy(),
            "shell": "Windows PowerShell -NoProfile",
            "source": "project-root fallback; no live instrumented terminal prompt was available",
            "verifiedPrompt": false,
            "commandReplay": false
        }));
    }
    let body = json!({"version":1,"adapter":"context","project":project,"explorer":folders,"terminal":{"items":terminal_items,"commandReplay":false},"limitations":["Only Explorer folders within the selected project are captured. Automatic close is blocked because tab coverage and active file operations are unknown.","Live instrumented terminal prompt working directories are captured when available; otherwise one project-root terminal is defined. Commands, panes, history and process state are never captured or replayed."]});
    let id = uuid::Uuid::new_v4().to_string();
    let before = crate::platform::memory()?;
    c.execute(
        "INSERT INTO snapshots VALUES(?,'local-context','context','captured',?,?,?,NULL,NULL)",
        rusqlite::params![
            id,
            crate::now(),
            crate::protection::encode(&body)?,
            before.to_string()
        ],
    )
    .map_err(|e| e.to_string())?;
    crate::history::index(&c, &id, project["id"].as_str())?;
    Ok(id)
}
pub fn restore_context_item(snapshot_id: &str, kind: &str, index: usize) -> Result<Value, String> {
    let c = crate::store::open()?;
    let snapshot = crate::store::snapshot(&c, snapshot_id)?;
    if snapshot["adapter"] != "context" || snapshot["body"]["version"] != 1 {
        return Err("Unsupported context snapshot".into());
    }
    let path = match kind {
        "explorer" => url::Url::parse(
            snapshot["body"]["explorer"]["items"][index]["url"]
                .as_str()
                .ok_or("Folder missing")?,
        )
        .map_err(|e| e.to_string())?
        .to_file_path()
        .map_err(|_| "Not a local folder URL")?
        .to_string_lossy()
        .to_string(),
        "terminal" => snapshot["body"]["terminal"]["items"]
            .as_array()
            .and_then(|items| items.get(index))
            .and_then(|item| item["cwd"].as_str())
            .or_else(|| snapshot["body"]["terminal"]["cwd"].as_str())
            .ok_or("Terminal folder missing")?
            .to_string(),
        _ => return Err("Unsupported context kind".into()),
    };
    let mut result = restore_folder_for(kind, &path, snapshot["body"]["project"]["id"].as_str())?;
    if kind == "explorer" {
        let expected = normalized_local_path(&local_folder(&path)?);
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let found = explorer()?["items"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["url"]
                        .as_str()
                        .and_then(|s| url::Url::parse(s).ok())
                        .and_then(|u| u.to_file_path().ok())
                        .is_some_and(|p| normalized_local_path(&p) == expected)
                })
            });
            if found {
                result = json!({"status":"folder-verified","expected":1,"observed":1,"detail":"Expected Explorer folder is present. Tabs, geometry and selection are not verified."});
                break;
            }
        }
    }
    c.execute(
        "UPDATE snapshots SET state='incomplete',result=? WHERE id=?",
        rusqlite::params![crate::protection::encode(&result)?, snapshot_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}
fn chrome_executable(explicit: Option<&str>) -> Result<std::path::PathBuf, String> {
    if let Some(raw) = explicit.filter(|s| !s.trim().is_empty()) {
        let path = std::path::PathBuf::from(raw);
        if !path.is_absolute()
            || !path.is_file()
            || path
                .file_name()
                .and_then(|v| v.to_str())
                .is_none_or(|v| !v.eq_ignore_ascii_case("chrome.exe"))
        {
            return Err("Choose an absolute local chrome.exe path.".into());
        }
        return Ok(path);
    }
    let candidates = [
        std::env::var_os("LOCALAPPDATA")
            .map(|p| std::path::PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
        std::env::var_os("ProgramFiles")
            .map(|p| std::path::PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
        std::env::var_os("ProgramFiles(x86)")
            .map(|p| std::path::PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
    ];
    candidates.into_iter().flatten().find(|p| p.is_file()).ok_or_else(||"Google Chrome Stable was not found in a standard location. Save its chrome.exe path for this profile.".to_string())
}
fn chrome_profile_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || value
            .chars()
            .any(|c| c == '/' || c == '\\' || c == ':' || c == '\0')
    {
        return Err("Enter the Chrome profile directory name, for example Default or Profile 1, without path separators.".into());
    }
    Ok(value.into())
}
pub fn configure_chrome_launch(
    client: &str,
    profile_directory: &str,
    executable: Option<&str>,
    user_data_dir: Option<&str>,
) -> Result<Value, String> {
    let c = crate::store::open()?;
    let (adapter, raw): (String, String) = c
        .query_row(
            "SELECT adapter,body FROM reports WHERE client=?",
            [client],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| {
            "Connect this Chrome profile once before configuring cold-start.".to_string()
        })?;
    if adapter != "chrome" {
        return Err("Cold-start profile binding is only available for Chrome integrations.".into());
    }
    let report = crate::protection::decode(&raw)?;
    let extension_id = report["extensionId"]
        .as_str()
        .filter(|v| v.len() == 32 && v.chars().all(|c| c.is_ascii_lowercase()))
        .ok_or("Reconnect the updated Chrome extension once before saving a cold-start binding.")?;
    let profile_directory = chrome_profile_name(profile_directory)?;
    let executable = chrome_executable(executable)?;
    let user_data_dir = user_data_dir
        .filter(|s| !s.trim().is_empty())
        .map(|raw| {
            let path = std::path::PathBuf::from(raw.trim());
            if !path.is_absolute() || !path.is_dir() {
                return Err(
                    "Chrome user-data root must be an existing absolute local directory."
                        .to_string(),
                );
            }
            Ok(path.to_string_lossy().into_owned())
        })
        .transpose()?;
    let value = json!({"client":client,"profileDirectory":profile_directory,"executable":executable.to_string_lossy(),"userDataDir":user_data_dir,"extensionId":extension_id,"configuredAt":crate::now()});
    c.execute(
        "INSERT OR REPLACE INTO meta VALUES(?,?)",
        rusqlite::params![
            format!("chrome-launch:{client}"),
            crate::protection::encode(&value)?
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn chrome_launch_binding(c: &rusqlite::Connection, client: &str) -> Option<Value> {
    let raw: String = c
        .query_row(
            "SELECT body FROM meta WHERE key=?",
            [format!("chrome-launch:{client}")],
            |r| r.get(0),
        )
        .ok()?;
    crate::protection::decode(&raw).ok()
}
pub(crate) fn import_chrome_launch_binding(
    c: &rusqlite::Connection,
    binding: &Value,
) -> Result<(), String> {
    let client = binding["client"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() < 200)
        .ok_or("Saved Chrome binding has an invalid integration identity")?;
    if let Some(existing) = chrome_launch_binding(c, client) {
        if existing["profileDirectory"] == binding["profileDirectory"]
            && existing["extensionId"] == binding["extensionId"]
            && existing["userDataDir"] == binding["userDataDir"]
        {
            return Ok(());
        }
        return Err("A different Chrome cold-start binding already exists for this integration identity; the existing binding was left unchanged.".into());
    }
    let profile = chrome_profile_name(
        binding["profileDirectory"]
            .as_str()
            .ok_or("Saved Chrome profile directory is missing")?,
    )?;
    let executable = chrome_executable(binding["executable"].as_str())?;
    let extension_id = binding["extensionId"]
        .as_str()
        .filter(|v| v.len() == 32 && v.chars().all(|c| c.is_ascii_lowercase()))
        .ok_or("Saved Chrome extension identity is invalid")?;
    let user_data_dir = binding["userDataDir"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|raw| {
            let path = std::path::PathBuf::from(raw);
            if !path.is_absolute() || !path.is_dir() {
                return Err("Saved Chrome user-data root is unavailable".to_string());
            }
            Ok(path.to_string_lossy().into_owned())
        })
        .transpose()?;
    let value = json!({"client":client,"profileDirectory":profile,"executable":executable.to_string_lossy(),"userDataDir":user_data_dir,"extensionId":extension_id,"configuredAt":crate::now()});
    c.execute(
        "INSERT OR IGNORE INTO meta VALUES(?,?)",
        rusqlite::params![
            format!("chrome-launch:{client}"),
            crate::protection::encode(&value)?
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn open_chrome_profile(client: &str) -> Result<Value, String> {
    let c = crate::store::open()?;
    let binding = chrome_launch_binding(&c, client).ok_or("This Chrome profile has no cold-start binding. Connect it once and save its profile directory before relying on reboot restore.")?;
    let profile = chrome_profile_name(
        binding["profileDirectory"]
            .as_str()
            .ok_or("Saved Chrome profile directory is missing")?,
    )?;
    let executable = chrome_executable(binding["executable"].as_str())?;
    let extension_id=binding["extensionId"].as_str().filter(|v|v.len()==32&&v.chars().all(|c|c.is_ascii_lowercase())).ok_or("Saved Chrome extension identity is missing. Reconnect the profile and save its cold-start binding again.")?;
    let mut command = std::process::Command::new(executable);
    command
        .arg(format!("--profile-directory={profile}"))
        .arg("--no-first-run");
    if let Some(root) = binding["userDataDir"].as_str().filter(|s| !s.is_empty()) {
        let path = std::path::PathBuf::from(root);
        if !path.is_absolute() || !path.is_dir() {
            return Err("Saved Chrome user-data root is missing or no longer available.".into());
        }
        command.arg(format!("--user-data-dir={}", path.to_string_lossy()));
    }
    if std::env::var_os("HIBERNATE_DATA_DIR").is_some() {
        if let Some(extension) = std::env::var_os("HIBERNATE_TEST_CHROME_EXTENSION")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_dir())
        {
            command
                .arg(format!(
                    "--disable-extensions-except={}",
                    extension.to_string_lossy()
                ))
                .arg(format!("--load-extension={}", extension.to_string_lossy()));
        }
        if let Some(port) = std::env::var_os("HIBERNATE_TEST_CHROME_DEBUG_PORT")
            .and_then(|v| v.to_string_lossy().parse::<u16>().ok())
            .filter(|p| *p > 0)
        {
            command.arg(format!("--remote-debugging-port={port}"));
        }
    }
    command.arg(format!("chrome-extension://{extension_id}/bootstrap.html"));
    command
        .spawn()
        .map_err(|e| format!("Chrome profile could not be launched: {e}"))?;
    Ok(
        json!({"status":"launched-unverified","client":client,"profileDirectory":profile,"detail":"Chrome launch requested for the explicitly configured profile. Restore waits for that profile's existing ContextSleep identity to reconnect before recreating tabs."}),
    )
}
pub fn open_vscode_snapshot(id: &str) -> Result<Value, String> {
    let c = crate::store::open()?;
    let snapshot = crate::store::snapshot(&c, id)?;
    if snapshot["adapter"] != "vscode" || snapshot["body"]["version"] != 1 {
        return Err("Unsupported VS Code snapshot".into());
    }
    let body = &snapshot["body"];
    let uri = body["workspace"]
        .as_str()
        .or_else(|| {
            body["folders"]
                .as_array()
                .filter(|f| f.len() == 1)
                .and_then(|f| f[0].as_str())
        })
        .ok_or("Save a local .code-workspace file for multi-root projects before capture")?;
    let path = url::Url::parse(uri)
        .map_err(|e| e.to_string())?
        .to_file_path()
        .map_err(|_| "Only local VS Code workspaces can be reopened")?;
    if !path.exists()
        || (!path.is_dir() && path.extension().and_then(|e| e.to_str()) != Some("code-workspace"))
    {
        return Err("Saved project folder or .code-workspace file is missing".into());
    }
    let test_mode = std::env::var_os("HIBERNATE_DATA_DIR").is_some();
    let explicit_test_exe = test_mode
        .then(|| std::env::var_os("HIBERNATE_TEST_VSCODE"))
        .flatten()
        .map(std::path::PathBuf::from)
        .filter(|p| {
            p.is_absolute()
                && p.is_file()
                && p.file_name()
                    .and_then(|v| v.to_str())
                    .is_some_and(|v| v.eq_ignore_ascii_case("Code.exe"))
        });
    let candidates = [
        std::env::var_os("LOCALAPPDATA")
            .map(|p| std::path::PathBuf::from(p).join("Programs/Microsoft VS Code/Code.exe")),
        std::env::var_os("ProgramFiles")
            .map(|p| std::path::PathBuf::from(p).join("Microsoft VS Code/Code.exe")),
    ];
    let exe = explicit_test_exe
        .or_else(|| candidates.into_iter().flatten().find(|p| p.is_file()))
        .ok_or(
            "Stable VS Code installation not found in its standard per-user or Program Files location",
        )?;
    let mut command = std::process::Command::new(exe);
    command.arg("--new-window");
    if test_mode {
        if let Some(user_data) = std::env::var_os("HIBERNATE_TEST_VSCODE_USER_DATA") {
            command.arg("--user-data-dir").arg(user_data);
        }
        if let Some(extensions) = std::env::var_os("HIBERNATE_TEST_VSCODE_EXTENSIONS_DIR") {
            command.arg("--extensions-dir").arg(extensions);
        }
        if let Some(extension) = std::env::var_os("HIBERNATE_TEST_VSCODE_EXTENSION") {
            command.arg(format!(
                "--extensionDevelopmentPath={}",
                std::path::PathBuf::from(extension).to_string_lossy()
            ));
        }
        command
            .arg("--skip-welcome")
            .arg("--skip-release-notes")
            .arg("--disable-gpu")
            .arg("--disable-workspace-trust");
    }
    command.arg(path).spawn().map_err(|e| e.to_string())?;
    let result = json!({"status":"launched-unverified","detail":"VS Code project launch requested. When its integration connects, use Resume / verify to reconstruct and check saved text editors. VS Code owns native hot exit, layout and trust decisions."});
    c.execute(
        "UPDATE snapshots SET state='incomplete',result=? WHERE id=?",
        rusqlite::params![crate::protection::encode(&result)?, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_path_normalization_treats_mixed_separators_and_case_as_equal() {
        let forward = std::path::Path::new("D:/Projects/Hibernate/api");
        let backward = std::path::Path::new("d:\\projects\\hibernate\\api\\");
        assert_eq!(normalized_local_path(forward), normalized_local_path(backward));
    }
    #[test]
    fn imported_chrome_binding_never_overwrites_a_different_existing_binding() {
        let root =
            std::env::temp_dir().join(format!("hibernate-chrome-binding-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let chrome = root.join("chrome.exe");
        std::fs::write(&chrome, []).unwrap();
        let c = crate::store::open_at(root.join("state.db")).unwrap();
        let first = json!({"client":"profile","profileDirectory":"Default","executable":chrome,"userDataDir":root,"extensionId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"});
        import_chrome_launch_binding(&c, &first).unwrap();
        let stored = chrome_launch_binding(&c, "profile").unwrap();
        assert_eq!(stored["profileDirectory"], "Default");
        import_chrome_launch_binding(&c, &first).unwrap();
        let mut changed = first.clone();
        changed["profileDirectory"] = json!("Profile 1");
        assert!(import_chrome_launch_binding(&c, &changed).is_err());
        assert_eq!(
            chrome_launch_binding(&c, "profile").unwrap()["profileDirectory"],
            "Default"
        );
    }
}
