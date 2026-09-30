//! Generic app context: native selections, protected snapshots and bounded launch authority.
//! No close primitive exists here. Snapshot/import data can never authorize a launch.
use crate::{
    adapters::{GENERIC, NATIVE_CLIENT},
    app_windows::{self, Window},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
type R<T> = Result<T, String>;
const TTL: u64 = 15_000;
#[derive(Default)]
struct SelectionCache {
    at: u64,
    items: HashMap<String, Window>,
}
static CACHE: OnceLock<Mutex<SelectionCache>> = OnceLock::new();
fn cache() -> &'static Mutex<SelectionCache> {
    CACHE.get_or_init(Default::default)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Executable {
    path: String,
    pub(crate) sha256: String,
    bytes: u64,
    #[serde(default)]
    file_identity: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Context {
    version: u32,
    adapter: String,
    captured_at: u64,
    project: Value,
    window: Window,
    launch: Option<Executable>,
    launch_limitation: String,
    #[serde(default)]
    reconstruction: Option<Value>,
}

pub fn migrate(c: &Connection) -> R<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS generic_origins(snapshot_id TEXT PRIMARY KEY,body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS generic_launch_attempts(origin TEXT PRIMARY KEY,snapshot_id TEXT NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL);").map_err(|e|e.to_string())
}

fn refresh_cache(cache: &mut SelectionCache, rows: Vec<Window>, now: u64) -> Vec<Value> {
    let previous = if now.saturating_sub(cache.at) < TTL {
        std::mem::take(&mut cache.items)
    } else {
        HashMap::new()
    };
    let mut result = Vec::new();
    for w in rows {
        let id = previous
            .iter()
            .find(|(_, old)| app_windows::same_observation(old, &w))
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let executable = w.owner.as_ref().map(|p| p.executable.as_str());
        let label = executable
            .and_then(|s| s.rsplit(['\\', '/']).next())
            .unwrap_or("Identity unavailable");
        result.push(
            json!({"id":id,"label":label,"window":w,"capabilities":app_windows::capabilities(&w),
            "enhancement":executable.and_then(crate::adapters::enhancement),
            "group":w.owner.as_ref().map(|p|format!("{}:{}",p.pid,p.process_started))}),
        );
        cache.items.insert(id, w);
    }
    cache.at = now;
    result
}
pub fn discover() -> R<Value> {
    // Serialize observation + publication so an older concurrent scan cannot replace a newer lease.
    let mut cache = cache()
        .lock()
        .map_err(|_| "App discovery cache unavailable")?;
    let rows = app_windows::inventory()?;
    let now = crate::now();
    let items = refresh_cache(&mut cache, rows, now);
    Ok(json!({"at":now,"items":items,"error":null}))
}
fn selection(cache: &SelectionCache, id: &str, now: u64) -> R<Window> {
    if now.saturating_sub(cache.at) >= TTL {
        return Err("App discovery expired. Refresh and select the current window.".into());
    }
    cache
        .items
        .get(id)
        .filter(|w| w.owner.is_some())
        .cloned()
        .ok_or("Window identity is unavailable or this selection expired".into())
}

pub(crate) fn launch_policy(w: &Window) -> R<&str> {
    let owner = w
        .owner
        .as_ref()
        .ok_or("Executable identity is unavailable")?;
    if w.elevated != Some(false) {
        return Err("Elevated or unreadable security context: relaunch requires application-specific support".into());
    }
    if !matches!(
        w.evidence.package,
        crate::reconstruction::PackageIdentity::Unpackaged
    ) {
        return Err("Package identity is packaged or unavailable; a validated activation contract is required".into());
    }
    if !matches!(
        w.evidence.restart,
        crate::reconstruction::RestartRegistration::Absent
    ) {
        return Err("Application restart settings require a validated state and working-directory contract; a no-argument fallback is blocked".into());
    }
    if crate::adapters::enhancement(&owner.executable).is_some() {
        return Err(
            "Use the specialized adapter to preserve this app's profile or workspace".into(),
        );
    }
    if w.app_user_model_id.is_some() || w.relaunch_metadata != "absent" {
        return Err("App-specific launch metadata requires a validated adapter; commands are never replayed".into());
    }
    let name = owner
        .executable
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "conhost.exe"
            | "openconsole.exe"
            | "wsl.exe"
            | "bash.exe"
            | "rundll32.exe"
            | "dllhost.exe"
            | "applicationframehost.exe"
            | "runtimebroker.exe"
            | "mshta.exe"
            | "wscript.exe"
            | "cscript.exe"
            | "node.exe"
            | "python.exe"
            | "pythonw.exe"
            | "python3.exe"
            | "java.exe"
            | "javaw.exe"
            | "msiexec.exe"
            | "taskmgr.exe"
            | "regedit.exe"
            | "mmc.exe"
            | "contextsleep.exe"
            | "project-hibernate.exe"
            | "hibernate-host.exe"
    ) {
        return Err(
            "Shells, runtimes, brokers and management tools require a specialized adapter".into(),
        );
    }
    Ok(&owner.executable)
}

fn local_executable(path: &str) -> R<()> {
    let bytes = path.as_bytes();
    if path.len() > 32700
        || bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
        || path[2..].contains([':', '\0', '\r', '\n'])
        || !path.to_ascii_lowercase().ends_with(".exe")
    {
        return Err("Relaunch requires an absolute local desktop executable".into());
    }
    let canonical = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let canonical = canonical
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .replace('/', "\\")
        .to_ascii_lowercase();
    // Reject aliases/junctions and redirected drives rather than following an unreviewed target.
    if canonical != path.replace('/', "\\").to_ascii_lowercase() {
        return Err("Executable path is redirected or ambiguous".into());
    }
    let drive: Vec<u16> = path[..3].encode_utf16().chain(Some(0)).collect();
    unsafe {
        use windows::Win32::Storage::FileSystem::GetDriveTypeW;
        // DRIVE_FIXED = 3 in the Windows SDK.
        if GetDriveTypeW(windows::core::PCWSTR(drive.as_ptr())) != 3 {
            return Err("Only a fixed local drive can supply a generic launch".into());
        }
    }
    let windows = std::env::var("WINDIR")
        .map_err(|_| "Windows directory unavailable")?
        .replace('/', "\\")
        .to_ascii_lowercase();
    if canonical.starts_with(&(windows.trim_end_matches('\\').to_string() + "\\"))
        || canonical.contains("\\windowsapps\\")
    {
        return Err(
            "Windows components and packaged apps require a specialized launch contract".into(),
        );
    }
    Ok(())
}

pub(crate) fn executable(path: &str) -> R<(Executable, File)> {
    local_executable(path)?;
    // Deny write/delete sharing through capture/launch. A digest change (including an app update)
    // requires a fresh capture. This is file continuity, not publisher authentication.
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|e| e.to_string())?;
    let file_identity = unsafe {
        use windows::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
        };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)
            .map_err(|e| e.to_string())?;
        format!(
            "{:08x}:{:08x}{:08x}",
            info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
        )
    };
    let bytes = file.metadata().map_err(|e| e.to_string())?.len();
    if !(128..=536_870_912).contains(&bytes) {
        return Err("Executable size is outside the generic validation bound".into());
    }
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos).map_err(|e| e.to_string())?;
    if &dos[..2] != b"MZ" {
        return Err("Not a Windows executable".into());
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if offset > bytes.saturating_sub(96) {
        return Err("Invalid PE header".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut header = [0u8; 96];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..4] != b"PE\0\0"
        || !matches!(u16::from_le_bytes([header[24], header[25]]), 0x10b | 0x20b)
        || u16::from_le_bytes([header[92], header[93]]) != 2
        || u16::from_le_bytes([header[22], header[23]]) & 0x2000 != 0
    {
        return Err("Only a desktop GUI executable can use generic relaunch".into());
    }
    file.rewind().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok((
        Executable {
            path: path.into(),
            sha256: format!("{:x}", hash.finalize()),
            bytes,
            file_identity: Some(file_identity),
        },
        file,
    ))
}

pub fn validate(body: &Value) -> R<()> {
    let c: Context = serde_json::from_value(body.clone())
        .map_err(|e| format!("Invalid generic context: {e}"))?;
    let w = &c.window;
    let owner = w
        .owner
        .as_ref()
        .ok_or("Generic capture lacks process identity")?;
    if c.adapter != GENERIC
        || c.version != 1
        || owner.pid == 0
        || owner.pid != w.owner_pid
        || owner
            .process_started
            .parse::<u64>()
            .ok()
            .filter(|v| *v > 0)
            .is_none()
        || w.handle.parse::<usize>().ok().filter(|v| *v > 0).is_none()
        || w.thread_id == 0
        || w.title.len() > 8192
        || w.class.len() > 1024
        || owner.executable.len() > 32700
        || w.same_owner_windows > 16384
        || w.app_user_model_id.as_ref().is_some_and(|s| s.len() > 1024)
        || c.launch_limitation.len() > 4096
        || w.evidence
            .owner_hwnd
            .as_ref()
            .is_some_and(|s| s.parse::<usize>().is_err())
        || w.evidence
            .parent_hwnd
            .as_ref()
            .is_some_and(|s| s.parse::<usize>().is_err())
        || matches!(&w.evidence.package, crate::reconstruction::PackageIdentity::Packaged { name } if name.len() > 4096)
        || matches!(&w.evidence.restart, crate::reconstruction::RestartRegistration::Registered { characters, .. } if *characters > 1025)
        || !matches!(
            w.relaunch_metadata.as_str(),
            "absent" | "unavailable" | "present-not-executed"
        )
    {
        return Err("Invalid generic observation bounds".into());
    }
    if let Some(l) = &c.launch {
        if l.path != owner.executable
            || l.sha256.len() != 64
            || !l.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            || l.bytes > 536_870_912
            || l.file_identity.as_ref().is_some_and(|id| {
                id.len() != 25 || !id.bytes().all(|c| c.is_ascii_hexdigit() || c == b':')
            })
        {
            return Err("Invalid generic executable evidence".into());
        }
    }
    Ok(())
}

pub fn capture(project: &str, id: &str) -> R<String> {
    let saved = selection(
        &*cache().lock().map_err(|_| "Discovery unavailable")?,
        id,
        crate::now(),
    )?;
    let fresh = app_windows::inventory()?
        .into_iter()
        .find(|w| app_windows::same_observation(&saved, w))
        .ok_or("Window changed since selection. Refresh and capture again.")?;
    let (launch, limitation) = match launch_policy(&fresh).and_then(executable) {
        Ok((definition,_)) => (Some(definition),"May reopen the app with no arguments. Documents and internal state are not reconstructed.".to_string()),
        Err(e) => (None,e),
    };
    let mut c = crate::store::open()?;
    let assessment = crate::reconstruction::assess(
        &fresh,
        if launch.is_some() {
            crate::reconstruction::Recipe::Available
        } else {
            crate::reconstruction::Recipe::Blocked(limitation.clone())
        },
    );
    let context = Context {
        version: 1,
        adapter: GENERIC.into(),
        captured_at: crate::now(),
        project: crate::projects::get(&c, project)?,
        window: fresh,
        launch,
        launch_limitation: limitation,
        reconstruction: Some(serde_json::to_value(assessment).map_err(|e| e.to_string())?),
    };
    let body = serde_json::to_value(context).map_err(|e| e.to_string())?;
    validate(&body)?;
    let sid = uuid::Uuid::new_v4().to_string();
    let before = crate::platform::memory()?;
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let encoded = crate::protection::encode(&body)?;
    tx.execute(
        "INSERT INTO snapshots VALUES(?,?,?,'captured',?,?,?,NULL,NULL)",
        params![
            sid,
            NATIVE_CLIENT,
            GENERIC,
            crate::now(),
            encoded,
            before.to_string()
        ],
    )
    .map_err(|e| e.to_string())?;
    crate::history::index(&tx, &sid, Some(project))?;
    tx.execute(
        "INSERT INTO generic_origins VALUES(?,?)",
        params![sid, encoded],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(sid)
}

pub fn assessment(body: &Value) -> R<Value> {
    validate(body)?;
    let c: Context = serde_json::from_value(body.clone()).map_err(|e| e.to_string())?;
    let recipe = match launch_policy(&c.window) {
        Err(reason) => crate::reconstruction::Recipe::Blocked(reason),
        Ok(_) if c.launch.is_some() => crate::reconstruction::Recipe::Available,
        Ok(_) => crate::reconstruction::Recipe::Blocked(c.launch_limitation),
    };
    serde_json::to_value(crate::reconstruction::assess(&c.window, recipe))
        .map_err(|e| e.to_string())
}

fn origin(c: &Connection, sid: &str, body: &Value) -> R<()> {
    let saved: Option<String> = c
        .query_row(
            "SELECT body FROM generic_origins WHERE snapshot_id=?",
            [sid],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if saved
        .map(|v| crate::protection::decode(&v))
        .transpose()?
        .as_ref()
        != Some(body)
    {
        return Err("This context has no matching local capture authority. Imported context never authorizes executable launch.".into());
    }
    Ok(())
}
fn claim_launch(c: &Connection, sid: &str, key: &str) -> R<()> {
    c.execute("INSERT INTO generic_launch_attempts VALUES(?,?,'attempted',?)",params![key,sid,crate::protection::encode(&json!({"requestedAt":crate::now(),"detail":"Launch intent committed. Uncertain launches are never retried or replayed."}))?]).map_err(|_|"A launch was already attempted for this captured app instance. Inspect the app and capture again; automatic retry is blocked.".to_string())?;
    Ok(())
}
fn presence(executable: &str) -> R<Vec<Window>> {
    Ok(app_windows::inventory()?
        .into_iter()
        .filter(|w| {
            w.owner
                .as_ref()
                .is_some_and(|o| o.executable.eq_ignore_ascii_case(executable))
        })
        .collect())
}
fn observation(status: &str, windows: &[Window]) -> Value {
    json!({"status":status,"verified":false,"checkedAt":crate::now(),"observedWindows":windows.len(),
        "detail":"Application presence is the only observation. Documents, unsaved work and internal app state are not reconstructed or verified; workspace readiness remains incomplete.",
        "facts":[{"capability":"Application window presence","status":if windows.is_empty(){"unverified"}else{"observed"}},
                 {"capability":"Saved working state","status":"unsupported"}]})
}

/// Called only by the checkpoint coordinator under its operation lock and durable running step.
pub fn restore(sid: &str) -> R<Value> {
    let c = crate::store::open()?;
    let s = crate::store::snapshot(&c, sid)?;
    crate::adapters::validate_snapshot(GENERIC, &s["body"])?;
    let body: Context = serde_json::from_value(s["body"].clone()).map_err(|e| e.to_string())?;
    let owner = body
        .window
        .owner
        .as_ref()
        .ok_or("Missing process identity")?;
    let present = presence(&owner.executable)?;
    if !present.is_empty() {
        let original: Vec<_> = present
            .iter()
            .filter(|w| w.owner.as_ref() == Some(owner))
            .cloned()
            .collect();
        let mut result = observation(
            if original.is_empty() {
                "different-app-instance-present"
            } else {
                "app-present-state-unverified"
            },
            &present,
        );
        result["capturedProcessPresent"] = json!(!original.is_empty());
        result["capturedWindowLifetimeVerified"] = json!(false);
        return Ok(result);
    }
    origin(&c, sid, &s["body"])?;
    let launch = body.launch.as_ref().ok_or(body.launch_limitation)?;
    launch_policy(&body.window)?;
    if !crate::platform::process_ended(owner.pid, &owner.process_started)? {
        return Err("Original app process is still running without an observed window; duplicate launch is blocked".into());
    }
    // Include background processes and unreadable matching candidates. A window scan alone
    // cannot prove a single-instance Electron application has stopped.
    let name = Path::new(&launch.path)
        .file_name()
        .ok_or("Missing executable name")?
        .to_string_lossy();
    for (pid, (_, candidate)) in app_windows::process_table()? {
        if candidate.eq_ignore_ascii_case(&name) {
            let running = app_windows::identity(pid)
                .ok_or("Matching process identity unreadable; duplicate launch is blocked")?;
            if running.executable.eq_ignore_ascii_case(&launch.path) {
                return Err(
                    "This application is already running; duplicate launch is blocked".into(),
                );
            }
        }
    }
    let (checked, _file_lock) = executable(&launch.path)?;
    if checked != *launch {
        return Err("Executable changed since capture. Open the app and capture it again.".into());
    }
    let key = format!("{}:{}", owner.pid, owner.process_started);
    claim_launch(&c, sid, &key)?;
    let child = crate::app_launch::launch(&launch.path, &body.window)?;
    let launched = child.identity();
    let end = Instant::now() + Duration::from_secs(8);
    let mut result = loop {
        let seen = presence(&launch.path)?;
        // Do not credit a launcher exit / singleton handoff with a window owned by someone else.
        let exact: Vec<_> = seen
            .into_iter()
            .filter(|w| launched.is_some() && w.owner == launched)
            .collect();
        if !exact.is_empty() {
            let mut result = observation("launched-app-observed-state-unverified", &exact);
            result["placement"] = crate::app_launch::placement_result(
                &body.window,
                &exact,
                child.placement_requested,
            );
            break result;
        }
        if Instant::now() >= end {
            break observation("launched-unverified", &[]);
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    result["reconstruction"] = serde_json::to_value(crate::reconstruction::assess(
        &body.window,
        crate::reconstruction::Recipe::Available,
    ))
    .map_err(|e| e.to_string())?;
    c.execute(
        "UPDATE generic_launch_attempts SET state='observed',body=? WHERE origin=?",
        params![crate::protection::encode(&result)?, key],
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn window() -> Window {
        Window {
            handle: "12".into(),
            thread_id: 3,
            title: "Fixture".into(),
            class: "Fixture".into(),
            visible: true,
            owner: Some(app_windows::ProcessIdentity {
                pid: 10,
                process_started: "20".into(),
                executable: "D:\\fixture.exe".into(),
            }),
            owner_pid: 10,
            parent_pid: Some(4),
            same_owner_windows: 2,
            elevated: Some(false),
            app_user_model_id: None,
            relaunch_metadata: "absent".into(),
            placement: [0, 0, 300, 200],
            evidence: crate::reconstruction::NativeEvidence {
                package: crate::reconstruction::PackageIdentity::Unpackaged,
                restart: crate::reconstruction::RestartRegistration::Absent,
                ..Default::default()
            },
        }
    }
    #[test]
    fn discovery_revokes_changed_disappeared_and_expired_selections() {
        let mut c = SelectionCache::default();
        let w = window();
        let rows = refresh_cache(&mut c, vec![w.clone()], 100);
        let id = rows[0]["id"].as_str().unwrap();
        assert!(selection(&c, id, 101).is_ok());
        assert!(selection(&c, id, 15100).is_err());
        let mut reused = w.clone();
        reused.owner.as_mut().unwrap().process_started = "21".into();
        refresh_cache(&mut c, vec![reused], 102);
        assert!(selection(&c, id, 103).is_err());
        refresh_cache(&mut c, vec![], 104);
        assert!(c.items.is_empty());
    }
    #[test]
    fn unreadable_identity_is_not_capture_or_close_authority() {
        let mut w = window();
        w.owner = None;
        let mut c = SelectionCache::default();
        let rows = refresh_cache(&mut c, vec![w.clone()], 100);
        assert!(selection(&c, rows[0]["id"].as_str().unwrap(), 101).is_err());
        assert_eq!(app_windows::capabilities(&w)["contextCapture"], false);
    }
    #[test]
    fn shared_windows_are_distinct_and_never_closeable() {
        let a = window();
        let mut b = a.clone();
        b.handle = "13".into();
        b.visible = false;
        assert!(!app_windows::same_observation(&a, &b));
        assert_eq!(app_windows::capabilities(&a)["reviewedClose"], false);
        assert_eq!(app_windows::capabilities(&b)["semanticRestore"], false);
    }
    #[test]
    fn metadata_elevation_and_specialized_adapters_block_generic_launch() {
        let mut w = window();
        assert!(launch_policy(&w).is_ok());
        w.app_user_model_id = Some("hosted.app".into());
        assert!(launch_policy(&w).is_err());
        w.app_user_model_id = None;
        w.relaunch_metadata = "present-not-executed".into();
        assert!(launch_policy(&w).is_err());
        w.relaunch_metadata = "absent".into();
        w.elevated = None;
        assert!(launch_policy(&w).is_err());
        w.elevated = Some(true);
        assert!(launch_policy(&w).is_err());
        w.elevated = Some(false);
        for name in [
            "Code.exe",
            "chrome.exe",
            "explorer.exe",
            "WindowsTerminal.exe",
            "cmd.exe",
            "pythonw.exe",
        ] {
            w.owner.as_mut().unwrap().executable = format!("D:\\{name}");
            assert!(launch_policy(&w).is_err());
        }
    }
    #[test]
    fn imported_forged_or_changed_bodies_cannot_acquire_launch_authority() {
        let c = Connection::open_in_memory().unwrap();
        migrate(&c).unwrap();
        let body = json!({"payload":"original"});
        assert!(origin(&c, "imported", &body).is_err());
        c.execute(
            "INSERT INTO generic_origins VALUES('local',?)",
            [crate::protection::encode(&body).unwrap()],
        )
        .unwrap();
        assert!(origin(&c, "local", &body).is_ok());
        assert!(origin(&c, "local", &json!({"payload":"changed"})).is_err());
        claim_launch(&c, "local", "10:20").unwrap();
        assert!(claim_launch(&c, "another-snapshot", "10:20").is_err());
    }
    #[test]
    fn observation_cannot_promote_launch_to_workspace_verification() {
        assert_eq!(observation("launched", &[window()])["verified"], false);
        assert_eq!(observation("missing", &[])["verified"], false);
    }
    #[test]
    fn changed_executable_is_detected_and_file_is_locked_during_validation() {
        let path =
            std::env::temp_dir().join(format!("contextsleep-fixture-{}.exe", uuid::Uuid::new_v4()));
        let mut pe = vec![0u8; 512];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&128u32.to_le_bytes());
        pe[128..132].copy_from_slice(b"PE\0\0");
        pe[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
        pe[220..222].copy_from_slice(&2u16.to_le_bytes());
        std::fs::write(&path, &pe).unwrap();
        let (first, lock) = executable(path.to_str().unwrap()).unwrap();
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        drop(lock);
        pe[400] = 1;
        std::fs::write(&path, &pe).unwrap();
        let (changed, lock) = executable(path.to_str().unwrap()).unwrap();
        assert_ne!(first, changed);
        drop(lock);
        let replacement = path.with_extension("replacement");
        std::fs::write(&replacement, &pe).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::rename(&replacement, &changed.path).unwrap();
        let (replaced, lock) = executable(&changed.path).unwrap();
        assert_eq!(changed.sha256, replaced.sha256);
        assert_ne!(
            changed.file_identity, replaced.file_identity,
            "Identical bytes do not mean the same file object"
        );
        drop(lock);
        std::fs::remove_file(&changed.path).unwrap();
        assert!(executable(&changed.path).is_err());
    }
    #[test]
    fn recycled_hwnd_with_identical_metadata_is_never_lifetime_proof() {
        let w = window();
        assert!(app_windows::same_observation(&w, &w));
        let d = crate::reconstruction::assess(&w, crate::reconstruction::Recipe::Available);
        assert!(!d.identity_proven && !d.safe_close_allowed);
        let mut changed = w.clone();
        changed.thread_id += 1;
        assert!(!app_windows::same_observation(&w, &changed));
        changed = w.clone();
        changed.same_owner_windows += 1;
        assert!(!app_windows::same_observation(&w, &changed));
    }
    #[test]
    fn restart_package_and_legacy_unknown_evidence_never_fall_back_to_bare_launch() {
        use crate::reconstruction::*;
        let mut w = window();
        w.evidence.restart = RestartRegistration::Registered {
            characters: 40,
            flags: 0,
        };
        assert!(launch_policy(&w).is_err());
        w.evidence.restart = RestartRegistration::Unknown;
        assert!(launch_policy(&w).is_err());
        w.evidence.restart = RestartRegistration::Absent;
        w.evidence.package = PackageIdentity::Packaged {
            name: "fixture".into(),
        };
        assert!(launch_policy(&w).is_err());
        w.evidence = NativeEvidence::default();
        assert!(launch_policy(&w).is_err());
    }
    #[test]
    fn unsafe_paths_and_console_images_cannot_be_generic_launches() {
        for path in [
            "relative.exe",
            "\\\\server\\app.exe",
            "C:\\app.exe:stream",
            "C:\\app.cmd",
            "shell:AppsFolder\\app",
            "C:\\app.exe\0",
        ] {
            assert!(local_executable(path).is_err());
        }
        assert!(
            executable(std::env::current_exe().unwrap().to_str().unwrap()).is_err(),
            "The console test harness is not a GUI launch target"
        );
    }
}
