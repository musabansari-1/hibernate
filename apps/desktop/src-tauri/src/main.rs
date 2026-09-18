#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use hibernate_native::store;
use tauri::Manager;
#[tauri::command]
async fn capture_checkpoint(project_id:String,targets:Vec<(String,String)>,include_context:bool)->Result<String,String>{tauri::async_runtime::spawn_blocking(move||hibernate_native::checkpoints::capture(&project_id,targets,include_context)).await.map_err(|e|e.to_string())?}
#[tauri::command]
async fn restore_checkpoint(id:String,selected:Vec<String>,acknowledged_services:bool)->Result<String,String>{tauri::async_runtime::spawn_blocking(move||hibernate_native::checkpoints::restore(&id,selected,acknowledged_services)).await.map_err(|e|e.to_string())?}
#[tauri::command]
async fn review_checkpoint_close(id:String)->Result<serde_json::Value,String>{tauri::async_runtime::spawn_blocking(move||hibernate_native::hibernation::prepare(&id)).await.map_err(|e|e.to_string())?}
#[tauri::command]
async fn close_checkpoint(review_id:String,selected:Vec<String>,acknowledged:bool)->Result<String,String>{tauri::async_runtime::spawn_blocking(move||hibernate_native::hibernation::close(&review_id,selected,acknowledged)).await.map_err(|e|e.to_string())?}
#[tauri::command]
async fn dashboard() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(store::dashboard)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn export_diagnostics() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(hibernate_native::diagnostics::export)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn enqueue(
    client: String,
    action: String,
    target: Option<String>,
    snapshot_id: Option<String>,
) -> Result<String, String> {
    store::enqueue(client, action, target, snapshot_id)
}
#[tauri::command]
fn save_workspace(name: String, project_path: String) -> Result<(), String> {
    store::save_workspace(name, project_path)
}
#[tauri::command]
fn close_reviewed(snapshot_id:String,acknowledged:bool,quick_sleep:bool)->Result<String,String>{store::close_reviewed(snapshot_id,acknowledged,quick_sleep)}
#[tauri::command]
fn create_project(name:String,project_path:String)->Result<serde_json::Value,String>{
    let c=store::open()?;let value=hibernate_native::projects::save(&c,None,&name,&project_path)?;
    hibernate_native::projects::select(&c,value["id"].as_str().ok_or("Missing project ID")?)
}
#[tauri::command]
fn select_project(id:String)->Result<serde_json::Value,String>{hibernate_native::projects::select(&store::open()?,&id)}
#[tauri::command]
fn assign_project(client:String,target:String,project_id:Option<String>)->Result<(),String>{hibernate_native::projects::assign(&store::open()?,&client,&target,project_id.as_deref())}
#[tauri::command]
fn inspect_project(path:String)->Result<serde_json::Value,String>{hibernate_native::repository::inspect(&path)}
#[tauri::command]
async fn discover_projects()->Result<serde_json::Value,String>{tauri::async_runtime::spawn_blocking(hibernate_native::discovery::discover).await.map_err(|e|e.to_string())?}
#[tauri::command]
fn configure_chrome_launch(client:String,profile_directory:String,executable:Option<String>,user_data_dir:Option<String>)->Result<serde_json::Value,String>{hibernate_native::context::configure_chrome_launch(&client,&profile_directory,executable.as_deref(),user_data_dir.as_deref())}
#[tauri::command]
fn save_service(project_id:String,name:String,url:String)->Result<serde_json::Value,String>{hibernate_native::health::save(&project_id,&name,&url)}
#[tauri::command]
fn configure_service_launch(id:String,definition:hibernate_native::services::Launch)->Result<serde_json::Value,String>{hibernate_native::services::configure(&id,definition)}
#[tauri::command]
async fn start_service(id:String,reviewed:hibernate_native::services::Launch,acknowledged:bool)->Result<serde_json::Value,String>{tauri::async_runtime::spawn_blocking(move||hibernate_native::services::start(&id,reviewed,acknowledged)).await.map_err(|e|e.to_string())?}
#[tauri::command]
fn snapshot_history(project_id:String,limit:u32,archived:bool)->Result<Vec<serde_json::Value>,String>{hibernate_native::history::list(&store::open()?,&project_id,limit,archived)}
#[tauri::command]
fn archive_snapshot(id:String,archived:bool)->Result<(),String>{hibernate_native::history::archive(&id,archived)}
#[tauri::command]
fn export_snapshot(id:String)->Result<String,String>{hibernate_native::history::export(&id)}
#[tauri::command]
fn import_snapshot(path:String)->Result<String,String>{hibernate_native::history::import(&path)}
#[tauri::command]
fn export_checkpoint(id:String)->Result<String,String>{hibernate_native::checkpoints::export(&id)}
#[tauri::command]
fn import_checkpoint(path:String)->Result<String,String>{hibernate_native::checkpoints::import(&path)}
#[tauri::command]
fn delete_snapshot(id:String,confirmation:String)->Result<(),String>{hibernate_native::history::delete(&id,&confirmation)}
#[tauri::command]
async fn verify_services(project_id:String)->Result<serde_json::Value,String>{tauri::async_runtime::spawn_blocking(move||hibernate_native::health::verify_project(&project_id)).await.map_err(|e|e.to_string())?}
#[tauri::command]
async fn capture_context() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(hibernate_native::context::capture_context)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn restore_folder(kind: String, path: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move||hibernate_native::context::restore_folder(&kind,&path)).await.map_err(|e|e.to_string())?
}
#[tauri::command]
async fn restore_context_item(
    snapshot_id: String,
    kind: String,
    index: usize,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        hibernate_native::context::restore_context_item(&snapshot_id, &kind, index)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_vscode_snapshot(snapshot_id: String) -> Result<serde_json::Value, String> {
    hibernate_native::context::open_vscode_snapshot(&snapshot_id)
}

fn integration_repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..\\..\\..")
}

fn integration_package_root() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate ContextSleep executable: {e}"))?;
    let dir = exe.parent().ok_or("Cannot locate ContextSleep package folder")?.to_path_buf();
    if dir.join("hibernate-host.exe").is_file() { Ok(dir) } else { Ok(integration_repo_root()) }
}

fn integration_stable_root() -> Result<std::path::PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    Ok(std::path::PathBuf::from(local).join("ContextSleep").join("integrations"))
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| format!("Cannot create {}: {e}", target.display()))?;
    for entry in std::fs::read_dir(source).map_err(|e| format!("Cannot read {}: {e}", source.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() { copy_tree(&from, &to)?; }
        else { std::fs::copy(&from, &to).map_err(|e| format!("Cannot copy {}: {e}", from.display()))?; }
    }
    Ok(())
}

fn find_vsix(root: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(root).ok()?.filter_map(Result::ok).map(|e| e.path())
        .find(|p| p.extension().and_then(|v| v.to_str()).is_some_and(|v| v.eq_ignore_ascii_case("vsix")) && p.file_name().and_then(|v| v.to_str()).is_some_and(|v| v.to_ascii_lowercase().contains("contextsleep")))
        .or_else(|| {
            let dev = integration_repo_root().join("extensions\\vscode");
            std::fs::read_dir(dev).ok()?.filter_map(Result::ok).map(|e| e.path())
                .find(|p| p.extension().and_then(|v| v.to_str()).is_some_and(|v| v.eq_ignore_ascii_case("vsix")))
        })
}

fn prepare_integration_files() -> Result<serde_json::Value, String> {
    let root = integration_package_root()?;
    let stable = integration_stable_root()?;
    std::fs::create_dir_all(&stable).map_err(|e| e.to_string())?;

    let packaged_host = root.join("hibernate-host.exe");
    let dev_release_host = integration_repo_root().join("native\\target\\release\\hibernate-host.exe");
    let dev_debug_host = integration_repo_root().join("native\\target\\debug\\hibernate-host.exe");
    let host_source = if packaged_host.is_file() { packaged_host } else if dev_release_host.is_file() { dev_release_host } else { dev_debug_host };
    if !host_source.is_file() { return Err("hibernate-host.exe is missing. Rebuild the ContextSleep package first.".into()); }
    let host = stable.join("hibernate-host.exe");
    std::fs::copy(&host_source, &host).map_err(|e| format!("Cannot prepare native bridge: {e}"))?;

    let packaged_chrome = root.join("chrome-extension");
    let dev_chrome = integration_repo_root().join("extensions\\chrome\\dist");
    let chrome_source = if packaged_chrome.is_dir() { packaged_chrome } else { dev_chrome };
    if !chrome_source.is_dir() { return Err("Chrome integration files are missing. Rebuild the ContextSleep package first.".into()); }
    let chrome = stable.join("chrome-extension");
    if chrome.exists() { std::fs::remove_dir_all(&chrome).map_err(|e| format!("Cannot refresh Chrome integration folder: {e}"))?; }
    copy_tree(&chrome_source, &chrome)?;

    let vsix_source = find_vsix(&root).ok_or("ContextSleep VS Code extension (.vsix) is missing")?;
    let vsix = stable.join("contextsleep-0.1.0.vsix");
    std::fs::copy(&vsix_source, &vsix).map_err(|e| format!("Cannot prepare VS Code extension: {e}"))?;

    let packaged_script = root.join("scripts\\register-native-host.ps1");
    let dev_script = integration_repo_root().join("scripts\\register-native-host.ps1");
    let script_source = if packaged_script.is_file() { packaged_script } else { dev_script };
    let scripts = stable.join("scripts");
    std::fs::create_dir_all(&scripts).map_err(|e| e.to_string())?;
    let script = scripts.join("register-native-host.ps1");
    std::fs::copy(&script_source, &script).map_err(|e| format!("Cannot prepare Chrome registration helper: {e}"))?;

    Ok(serde_json::json!({"root":stable,"hostPath":host,"chromeExtensionPath":chrome,"vsixPath":vsix,"registrationScript":script}))
}

#[tauri::command]
fn prepare_chrome_integration() -> Result<serde_json::Value, String> {
    let info = prepare_integration_files()?;
    let folder = info["chromeExtensionPath"].as_str().ok_or("Prepared Chrome folder is unavailable")?;
    let _ = std::process::Command::new("explorer.exe").arg(folder).spawn();
    let candidates = [
        std::env::var_os("PROGRAMFILES").map(std::path::PathBuf::from).map(|p| p.join("Google\\Chrome\\Application\\chrome.exe")),
        std::env::var_os("PROGRAMFILES(X86)").map(std::path::PathBuf::from).map(|p| p.join("Google\\Chrome\\Application\\chrome.exe")),
        std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).map(|p| p.join("Google\\Chrome\\Application\\chrome.exe")),
    ];
    if let Some(chrome) = candidates.into_iter().flatten().find(|p| p.is_file()) {
        let _ = std::process::Command::new(chrome).arg("chrome://extensions/").spawn();
    } else {
        let _ = std::process::Command::new("cmd.exe").args(["/C", "start", "", "chrome://extensions/"]).spawn();
    }
    Ok(info)
}

#[tauri::command]
fn register_chrome_integration(extension_id: String) -> Result<serde_json::Value, String> {
    let id = extension_id.trim().to_ascii_lowercase();
    if id.len() != 32 || !id.chars().all(|c| ('a'..='p').contains(&c)) { return Err("Paste the 32-character Chrome extension ID shown on chrome://extensions.".into()); }
    let info = prepare_integration_files()?;
    let script = info["registrationScript"].as_str().ok_or("Registration helper is unavailable")?;
    let host = info["hostPath"].as_str().ok_or("Native bridge is unavailable")?;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-ExtensionId", &id, "-HostPath", host])
        .output().map_err(|e| format!("Cannot run Chrome setup: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if detail.is_empty() { stdout } else { detail });
    }
    Ok(serde_json::json!({"registered":true,"extensionId":id,"hostPath":host,"detail":"Chrome bridge registered. In Chrome, click the ContextSleep extension and choose Connect / refresh."}))
}

#[tauri::command]
fn install_vscode_integration() -> Result<serde_json::Value, String> {
    let info = prepare_integration_files()?;
    let vsix = info["vsixPath"].as_str().ok_or("VS Code extension package is unavailable")?.to_string();
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") { candidates.push(std::path::PathBuf::from(local).join("Programs\\Microsoft VS Code\\bin\\code.cmd")); }
    if let Some(program_files) = std::env::var_os("PROGRAMFILES") { candidates.push(std::path::PathBuf::from(program_files).join("Microsoft VS Code\\bin\\code.cmd")); }
    if let Some(program_files) = std::env::var_os("PROGRAMFILES(X86)") { candidates.push(std::path::PathBuf::from(program_files).join("Microsoft VS Code\\bin\\code.cmd")); }
    let output = if let Some(code) = candidates.into_iter().find(|p| p.is_file()) {
        std::process::Command::new(code).args(["--install-extension", &vsix, "--force"]).output()
    } else {
        std::process::Command::new("cmd.exe").args(["/C", "code", "--install-extension", &vsix, "--force"]).output()
    }.map_err(|e| format!("Could not start the VS Code installer: {e}. Install the VSIX manually if VS Code is not on PATH."))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() { "VS Code extension installation failed. Install the packaged VSIX manually.".into() } else { detail });
    }
    Ok(serde_json::json!({"installed":true,"hostPath":info["hostPath"],"detail":"VS Code integration installed. Open or reload a trusted local project; ContextSleep will connect automatically."}))
}
fn main() {
    let Some(_lease)=hibernate_native::platform::desktop_lease().expect("Cannot establish desktop instance ownership") else {return;};
    tauri::Builder::default()
        .setup(|app|{
            hibernate_native::checkpoints::recover().map_err(std::io::Error::other)?;
            let _observer = hibernate_native::native_close::start_observer().map_err(std::io::Error::other)?;
            use tauri::{menu::{Menu,MenuItem},tray::TrayIconBuilder};
            let open=MenuItem::with_id(app,"open","Open ContextSleep",true,None::<&str>)?;
            let project_folder=MenuItem::with_id(app,"project_folder","Open active project folder",true,None::<&str>)?;
            let project_terminal=MenuItem::with_id(app,"project_terminal","Open active project terminal",true,None::<&str>)?;
            let memory=MenuItem::with_id(app,"memory","Reading Windows memory...",false,None::<&str>)?;
            let quit=MenuItem::with_id(app,"quit","Quit ContextSleep",true,None::<&str>)?;
            let menu=Menu::with_items(app,&[&open,&project_folder,&project_terminal,&memory,&quit])?;
            let mut tray=TrayIconBuilder::with_id("hibernate").tooltip("ContextSleep - your saved contexts").menu(&menu).on_menu_event(|app,event|match event.id.as_ref(){
                "open"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}},
                "project_folder"|"project_terminal"=>{let kind=if event.id.as_ref()=="project_folder"{"explorer"}else{"terminal"}.to_string();let handle=app.clone();std::thread::spawn(move||{let opened=store::open().and_then(|c|hibernate_native::projects::active(&c)).ok().and_then(|p|p["projectPath"].as_str().filter(|s|!s.trim().is_empty()).map(str::to_string)).is_some_and(|path|hibernate_native::context::restore_folder(&kind,&path).is_ok());if !opened{if let Some(w)=handle.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}}});},
                "quit"=>app.exit(0),_=>()});
            if let Some(icon)=app.default_window_icon(){tray=tray.icon(icon.clone());}
            tray.build(app)?;
            std::thread::spawn(move||loop{if let Ok(m)=hibernate_native::platform::memory(){if let(Some(total),Some(available))=(m["totalPhysical"].as_f64(),m["availablePhysical"].as_f64()){let _=memory.set_text(format!("Memory in use: {:.0}%",100.0*(1.0-available/total)));}}std::thread::sleep(std::time::Duration::from_secs(30));});
            Ok(())
        })
        .on_window_event(|window,event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=window.hide();}})
        .invoke_handler(tauri::generate_handler![
            dashboard,
            export_diagnostics,
            capture_checkpoint,
            restore_checkpoint,
            review_checkpoint_close,
            close_checkpoint,
            enqueue,
            close_reviewed,
            save_workspace,
            create_project,
            select_project,
            assign_project,
            inspect_project,
            discover_projects,
            configure_chrome_launch,
            save_service,
            configure_service_launch,
            start_service,
            snapshot_history,
            archive_snapshot,
            export_snapshot,
            import_snapshot,
            export_checkpoint,
            import_checkpoint,
            delete_snapshot,
            verify_services,
            capture_context,
            restore_folder,
            restore_context_item,
            open_vscode_snapshot,
            prepare_chrome_integration,
            register_chrome_integration,
            install_vscode_integration
        ])
        .run(tauri::generate_context!())
        .expect("Cannot start ContextSleep");
}

