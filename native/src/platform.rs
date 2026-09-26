//! Read-only Windows observations. No process termination, suspension or working-set trimming.
use serde_json::{json, Value};
use std::mem::size_of;
use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT},
        System::{Diagnostics::ToolHelp::*, ProcessStatus::*, SystemInformation::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
};

struct OwnedHandle(HANDLE);
/// A complete Toolhelp snapshot plus creation identity distinguishes an exited
/// process from a reused PID. Access failures are unknown, never exit evidence.
pub fn process_ended(pid: u32, expected_start: &str) -> Result<bool, String> {
    unsafe {
        let snapshot = OwnedHandle(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        Process32FirstW(snapshot.0, &mut entry).map_err(|e| e.to_string())?;
        loop {
            if entry.th32ProcessID == pid {
                return process_started(pid)
                    .map(|started| started != expected_start)
                    .ok_or("Process identity unreadable; exit is unverified".into());
            }
            if let Err(e) = Process32NextW(snapshot.0, &mut entry) {
                if e.code()
                    == windows::core::HRESULT::from_win32(
                        windows::Win32::Foundation::ERROR_NO_MORE_FILES.0,
                    )
                {
                    return Ok(true);
                }
                return Err(e.to_string());
            }
        }
    }
}
pub struct DesktopLease {
    _handle: OwnedHandle,
}
fn isolated_desktop_name(
    directory: &std::path::Path,
    normal: &std::path::Path,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let canonical = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
    let key = |p: &std::path::Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    let path = key(&canonical);
    // Aliases must share one journal owner; overrides must never bypass the normal lease.
    if std::fs::canonicalize(normal).is_ok_and(|normal| key(&normal) == path) {
        return Err(
            "An isolated data directory must differ from the normal application data directory"
                .into(),
        );
    }
    Ok(format!(
        "Local\\ProjectHibernate.Desktop.{:x}",
        Sha256::digest(path.as_bytes())
    ))
}

/// Hold one desktop journal owner per Windows session. The host remains independently callable.
pub fn desktop_lease() -> Result<Option<DesktopLease>, String> {
    unsafe {
        // Separate databases must not activate or reset a personal desktop session.
        // The same isolated database still has exactly one journal owner.
        let isolated = std::env::var_os("HIBERNATE_DATA_DIR").is_some();
        let name = if isolated {
            let database = crate::store::path()?;
            let normal = std::path::PathBuf::from(
                std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is missing")?,
            )
            .join("ProjectHibernate");
            isolated_desktop_name(
                database.parent().ok_or("Missing database directory")?,
                &normal,
            )?
        } else {
            "Local\\ProjectHibernate.Desktop".into()
        };
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = CreateMutexW(None, false, windows::core::PCWSTR(name.as_ptr()))
            .map_err(|e| e.to_string())?;
        let exists = windows::Win32::Foundation::GetLastError()
            == windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
        let owned = OwnedHandle(handle);
        if exists {
            if isolated {
                return Ok(None);
            }
            // Window titles are matching keys for single-instance focus. The current
            // ASCII title is tried first; earlier titles stay as fallbacks so an older
            // window still gets focused instead of a second instance appearing.
            let window = FindWindowW(None, windows::core::w!("ContextSleep - Private Beta"))
                .or_else(|_| FindWindowW(None, windows::core::w!("ContextSleep · Private Beta")))
                .or_else(|_| {
                    FindWindowW(None, windows::core::w!("Project Hibernate · Private Beta"))
                })
                .or_else(|_| {
                    FindWindowW(
                        None,
                        windows::core::w!("Project Hibernate · Technical Preview"),
                    )
                });
            if let Ok(window) = window {
                let _ = ShowWindow(window, SW_RESTORE);
                let _ = SetForegroundWindow(window);
            }
            return Ok(None);
        }
        Ok(Some(DesktopLease { _handle: owned }))
    }
}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn process(pid: u32) -> Option<(String, Option<(usize, usize)>)> {
    unsafe {
        let h = OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut path = vec![0u16; 32768];
        let mut len = path.len() as u32;
        QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut len)
            .ok()?;
        let mut m = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let memory = GetProcessMemoryInfo(
            h.0,
            &mut m as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
        .ok()
        .map(|_| (m.WorkingSetSize, m.PrivateUsage));
        Some((String::from_utf16_lossy(&path[..len as usize]), memory))
    }
}
pub fn process_started(pid: u32) -> Option<String> {
    unsafe {
        let h = OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut created = windows::Win32::Foundation::FILETIME::default();
        let mut exited = created;
        let mut kernel = created;
        let mut user = created;
        GetProcessTimes(h.0, &mut created, &mut exited, &mut kernel, &mut user).ok()?;
        Some((((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64).to_string())
    }
}
pub fn process_identity(pid: u32) -> Option<Value> {
    // Read creation time before and after the path query to reject PID reuse during observation.
    let started = process_started(pid)?;
    let (executable, _) = process(pid)?;
    if process_started(pid).as_deref() != Some(&started) {
        return None;
    }
    Some(json!({"pid":pid,"processStarted":started,"executable":executable}))
}
fn app(path: &str) -> &str {
    match path
        .rsplit('\\')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "chrome.exe" => "Chrome",
        "code.exe" => "VS Code",
        "windowsterminal.exe" => "Windows Terminal",
        "explorer.exe" => "Explorer",
        _ => "Unsupported",
    }
}
pub fn parent_process() -> Option<(u32, String)> {
    unsafe {
        let h = OwnedHandle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(h.0, &mut entry).is_ok();
        while more {
            if entry.th32ProcessID == std::process::id() {
                let id = entry.th32ParentProcessID;
                return process(id).map(|(path, _)| (id, path));
            }
            more = Process32NextW(h.0, &mut entry).is_ok();
        }
        None
    }
}
/// Resolve the bridge's own Code.exe ancestry. Never infer this from window titles.
pub fn vscode_owner() -> Result<Value, String> {
    let mut parents = std::collections::HashMap::new();
    unsafe {
        let snapshot = OwnedHandle(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot.0, &mut entry).is_ok();
        while more {
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            more = Process32NextW(snapshot.0, &mut entry).is_ok();
        }
    }
    let mut child =
        process_identity(std::process::id()).ok_or("Bridge process identity unavailable")?;
    for _ in 0..32 {
        let pid = child["pid"].as_u64().ok_or("Missing process identity")? as u32;
        let Some(parent) = parents
            .get(&pid)
            .filter(|parent| **parent != 0 && **parent != pid)
        else {
            break;
        };
        let Some(identity) = process_identity(*parent) else {
            break;
        };
        let path = identity["executable"]
            .as_str()
            .ok_or("Missing executable")?;
        if !path
            .rsplit('\\')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("Code.exe"))
        {
            break;
        }
        if identity["processStarted"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            > child["processStarted"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
        {
            return Err("Process ancestry changed during observation".into());
        }
        // A Code.exe CLI launcher can be above the actual GUI process. Select
        // the nearest ancestor owning a workbench window, including hidden ones.
        if !vscode_windows(*parent)?
            .as_array()
            .is_none_or(Vec::is_empty)
        {
            return Ok(identity);
        }
        child = identity;
    }
    Err("No supported VS Code window owner was found in this bridge's process ancestry".into())
}

pub fn vscode_windows(owner: u32) -> Result<Value, String> {
    struct Observation {
        owner: u32,
        windows: Vec<Value>,
    }
    unsafe extern "system" fn collect(hwnd: HWND, context: LPARAM) -> BOOL {
        let observation = &mut *(context.0 as *mut Observation);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == observation.owner {
            let mut class = [0u16; 256];
            let len = GetClassNameW(hwnd, &mut class);
            let class = String::from_utf16_lossy(&class[..len.max(0) as usize]);
            // Current Windows VS Code workbench windows use Electron's class.
            // Unknown future classes fail closed; no title-based guessing.
            if class == "Chrome_WidgetWin_1" {
                observation.windows.push(json!({"handle":(hwnd.0 as usize).to_string(),"pid":pid,"app":"VS Code","class":class,"visible":IsWindowVisible(hwnd).as_bool()}));
            }
        }
        BOOL(1)
    }
    let mut observation = Observation {
        owner,
        windows: vec![],
    };
    unsafe {
        EnumWindows(Some(collect), LPARAM(&mut observation as *mut _ as isize))
            .map_err(|e| e.to_string())?;
    }
    Ok(json!(observation.windows))
}

/// An absent HWND is observed separately from an unreadable or changing owner.
pub fn native_window_identity(handle: usize) -> Result<Option<Value>, String> {
    unsafe {
        let hwnd = HWND(handle as *mut std::ffi::c_void);
        if !IsWindow(Some(hwnd)).as_bool() {
            return Ok(None);
        }
        let mut pid = 0;
        if GetWindowThreadProcessId(hwnd, Some(&mut pid)) == 0 {
            return Err("Window changed during observation".into());
        }
        let identity = process_identity(pid).ok_or("Window owner identity unavailable")?;
        let mut checked = 0;
        if GetWindowThreadProcessId(hwnd, Some(&mut checked)) == 0 || checked != pid {
            return Err("Window owner changed during observation".into());
        }
        Ok(Some(identity))
    }
}
unsafe extern "system" fn enumerate(hwnd: HWND, param: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut title = [0u16; 2048];
    let len = GetWindowTextW(hwnd, &mut title);
    if len == 0 {
        return BOOL(1);
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if let Some((path, memory)) = process(pid) {
        if app(&path) == "Unsupported" {
            return BOOL(1);
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return BOOL(1);
        }
        let items = &mut *(param.0 as *mut Vec<Value>);
        items.push(json!({"handle": format!("{}", hwnd.0 as usize), "pid": pid, "title": String::from_utf16_lossy(&title[..len as usize]), "app": app(&path), "executable": path, "foreground": hwnd == GetForegroundWindow(), "left": rect.left, "top": rect.top, "width": rect.right - rect.left, "height": rect.bottom - rect.top, "workingSet": memory.map(|m| m.0), "privateCommit": memory.map(|m| m.1)}));
    }
    BOOL(1)
}
pub fn windows() -> Result<Value, String> {
    let mut items: Vec<Value> = vec![];
    unsafe {
        EnumWindows(Some(enumerate), LPARAM(&mut items as *mut _ as isize))
            .map_err(|e| e.to_string())?;
    }
    Ok(json!(items))
}
pub fn memory() -> Result<Value, String> {
    unsafe {
        let mut m = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        GlobalMemoryStatusEx(&mut m).map_err(|e| e.to_string())?;
        let snapshot = OwnedHandle(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let (mut working, mut commit, mut unreadable) = (0usize, 0usize, 0usize);
        let mut processes = std::collections::HashMap::new();
        let mut more = Process32FirstW(snapshot.0, &mut entry).is_ok();
        while more {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let is_chrome = String::from_utf16_lossy(&entry.szExeFile[..end])
                .eq_ignore_ascii_case("chrome.exe");
            let measured = if is_chrome {
                process(entry.th32ProcessID).and_then(|(_, m)| m)
            } else {
                None
            };
            if is_chrome {
                match measured {
                    Some((w, c)) => {
                        working += w;
                        commit += c;
                    }
                    None => unreadable += 1,
                }
            }
            processes.insert(
                entry.th32ProcessID,
                (entry.th32ParentProcessID, is_chrome, measured),
            );
            more = Process32NextW(snapshot.0, &mut entry).is_ok();
        }
        // A Chrome-launched native host can attribute to its ancestor browser, not to individual tabs.
        // Other callers intentionally get no source-browser attribution.
        let mut cursor = std::process::id();
        let mut source = None;
        for _ in 0..32 {
            if let Some((parent, chrome, _)) = processes.get(&cursor) {
                if *chrome {
                    source = Some(cursor);
                    break;
                }
                if *parent == cursor {
                    break;
                }
                cursor = *parent;
            } else {
                break;
            }
        }
        let (mut source_working, mut source_commit, mut source_unreadable) =
            (0usize, 0usize, 0usize);
        if let Some(root) = source {
            for (&pid, &(_, chrome, measured)) in &processes {
                if !chrome {
                    continue;
                }
                let mut cursor = pid;
                let mut belongs = false;
                for _ in 0..32 {
                    if cursor == root {
                        belongs = true;
                        break;
                    }
                    if let Some((parent, _, _)) = processes.get(&cursor) {
                        if *parent == cursor {
                            break;
                        }
                        cursor = *parent;
                    } else {
                        break;
                    }
                }
                if belongs {
                    match measured {
                        Some((w, c)) => {
                            source_working += w;
                            source_commit += c
                        }
                        None => source_unreadable += 1,
                    }
                }
            }
        }
        Ok(
            json!({"at": crate::now(), "totalPhysical": m.ullTotalPhys, "availablePhysical": m.ullAvailPhys, "chromeWorkingSet": working, "chromePrivateCommit": commit, "unreadableProcesses": unreadable,"sourceBrowserPid":source,"sourceBrowserWorkingSet":source.map(|_|source_working),"sourceBrowserPrivateCommit":source.map(|_|source_commit),"sourceBrowserUnreadable":source.map(|_|source_unreadable)}),
        )
    }
}

#[cfg(test)]
mod lease_tests {
    use super::isolated_desktop_name;
    #[test]
    fn isolated_database_aliases_share_a_lease_and_cannot_bypass_normal_ownership() {
        let root = std::env::temp_dir().join(format!("hibernate-lease-{}", uuid::Uuid::new_v4()));
        let normal = root.join("normal");
        let isolated = root.join("isolated");
        std::fs::create_dir_all(&normal).unwrap();
        let name = isolated_desktop_name(&isolated, &normal).unwrap();
        assert_eq!(
            name,
            isolated_desktop_name(&isolated.join("."), &normal).unwrap()
        );
        assert_eq!(
            name,
            isolated_desktop_name(&isolated.join("..").join("isolated"), &normal).unwrap()
        );
        assert!(isolated_desktop_name(&normal.join("."), &normal).is_err());
        std::fs::remove_dir(&isolated).unwrap();
        std::fs::remove_dir(&normal).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
