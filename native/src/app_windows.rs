//! Query-only native inventory. Every identity is an observation, never a window lease.
use crate::reconstruction::{NativeEvidence, PackageIdentity, RestartRegistration};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, mem::size_of};
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{CloseHandle, ERROR_NO_MORE_FILES, HANDLE, HWND, LPARAM, RECT},
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::{Diagnostics::ToolHelp::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
};

type R<T> = Result<T, String>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub process_started: String,
    pub executable: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Window {
    pub handle: String,
    pub thread_id: u32,
    pub title: String,
    pub class: String,
    pub visible: bool,
    pub owner: Option<ProcessIdentity>,
    pub owner_pid: u32,
    pub parent_pid: Option<u32>,
    pub same_owner_windows: usize,
    pub elevated: Option<bool>,
    pub app_user_model_id: Option<String>,
    pub relaunch_metadata: String,
    pub placement: [i32; 4],
    #[serde(default)]
    pub evidence: NativeEvidence,
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn process_table() -> R<HashMap<u32, (u32, String)>> {
    unsafe {
        let h = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?);
        let mut p = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        Process32FirstW(h.0, &mut p).map_err(|e| e.to_string())?;
        let mut rows = HashMap::new();
        loop {
            let end = p
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(p.szExeFile.len());
            rows.insert(
                p.th32ProcessID,
                (
                    p.th32ParentProcessID,
                    String::from_utf16_lossy(&p.szExeFile[..end]),
                ),
            );
            if let Err(e) = Process32NextW(h.0, &mut p) {
                if e.code() == windows::core::HRESULT::from_win32(ERROR_NO_MORE_FILES.0) {
                    return Ok(rows);
                }
                return Err(e.to_string());
            }
        }
    }
}
pub fn identity(pid: u32) -> Option<ProcessIdentity> {
    serde_json::from_value(crate::platform::process_identity(pid)?).ok()
}
fn elevated(pid: u32) -> Option<bool> {
    unsafe {
        let p = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut token = HANDLE::default();
        OpenProcessToken(p.0, TOKEN_QUERY, &mut token).ok()?;
        let token = Handle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0;
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .ok()?;
        Some(elevation.TokenIsElevated != 0)
    }
}

/// Documented OS metadata only. Restart probing requests VM_READ as required by
/// GetApplicationRestartSettings, but never reads memory or collects/replays arguments.
fn activation(pid: u32, readable_user: bool) -> (PackageIdentity, RestartRegistration) {
    use windows::Win32::{
        Foundation::*, Storage::Packaging::Appx::GetPackageFullName,
        System::Recovery::GetApplicationRestartSettings,
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return (PackageIdentity::Unknown, RestartRegistration::Unknown);
        };
        let handle = Handle(handle);
        let mut length = 0;
        let status = GetPackageFullName(handle.0, &mut length, None);
        let package = if status == APPMODEL_ERROR_NO_PACKAGE {
            PackageIdentity::Unpackaged
        } else if status == ERROR_INSUFFICIENT_BUFFER && (1..=1024).contains(&length) {
            let mut text = vec![0u16; length as usize];
            if GetPackageFullName(
                handle.0,
                &mut length,
                Some(windows::core::PWSTR(text.as_mut_ptr())),
            ) == ERROR_SUCCESS
            {
                PackageIdentity::Packaged {
                    name: String::from_utf16_lossy(
                        &text[..text.iter().position(|c| *c == 0).unwrap_or(text.len())],
                    ),
                }
            } else {
                PackageIdentity::Unknown
            }
        } else {
            PackageIdentity::Unknown
        };
        let mut restart = RestartRegistration::Unknown;
        if readable_user {
            if let Ok(handle) = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                false,
                pid,
            ) {
                let handle = Handle(handle);
                let mut size = 0;
                let mut flags = 0;
                match GetApplicationRestartSettings(handle.0, None, &mut size, Some(&mut flags)) {
                    Ok(()) if size <= 1025 => {
                        restart = RestartRegistration::Registered {
                            characters: size,
                            flags,
                        }
                    }
                    Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_NOT_FOUND.0) => {
                        restart = RestartRegistration::Absent
                    }
                    _ => {}
                }
            }
        }
        (package, restart)
    }
}

/// Read bounded scalar window properties in an initialized COM apartment.
/// Never collect or execute a relaunch command string.
unsafe fn properties(hwnd: HWND) -> (Option<String>, String) {
    use windows::Win32::{
        Foundation::PROPERTYKEY,
        System::Com::StructuredStorage::PropVariantToString,
        UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
    };
    let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd) else {
        return (None, "unavailable".into());
    };
    let format = windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3);
    let mut buffer = [0u16; 256];
    let app_id = store
        .GetValue(&PROPERTYKEY {
            fmtid: format,
            pid: 5,
        })
        .ok()
        .and_then(|v| {
            PropVariantToString(&v, &mut buffer).ok()?;
            let end = buffer.iter().position(|v| *v == 0)?;
            (end > 0).then(|| String::from_utf16_lossy(&buffer[..end]))
        });
    let relaunch = match store.GetValue(&PROPERTYKEY {
        fmtid: format,
        pid: 2,
    }) {
        Ok(v) => {
            if v.is_empty() {
                "absent"
            } else {
                "present-not-executed"
            }
        }
        Err(_) => "unavailable",
    };
    (app_id, relaunch.into())
}

pub fn inventory() -> R<Vec<Window>> {
    // Test isolation only narrows read-only inventory. It grants no launch/close authority.
    let scope: Option<Vec<String>> = if std::env::var_os("HIBERNATE_DATA_DIR").is_some() {
        std::env::var("HIBERNATE_TEST_DISCOVERY_EXECUTABLES")
            .ok()
            .map(|v| {
                serde_json::from_str(&v).map_err(|_| "Invalid fixture discovery scope".to_string())
            })
            .transpose()?
    } else {
        None
    };
    if scope.as_ref().is_some_and(Vec::is_empty) {
        return Ok(vec![]);
    }
    std::thread::spawn(move || unsafe {
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let table = process_table()?;
        unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
            let handles = &mut *(data.0 as *mut Vec<usize>);
            handles.push(hwnd.0 as usize);
            BOOL(1)
        }
        let mut handles = Vec::<usize>::new();
        EnumWindows(Some(collect), LPARAM(&mut handles as *mut _ as isize))
            .map_err(|e| e.to_string())?;
        if handles.len() > 16384 {
            return Err("Window inventory exceeded its observation bound".into());
        }
        let mut rows = Vec::new();
        let mut owners = HashMap::new();
        let mut elevations = HashMap::new();
        let mut activations = HashMap::new();
        for handle in handles {
            let hwnd = HWND(handle as *mut _);
            let mut pid = 0;
            let tid = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if tid == 0 || pid == std::process::id() {
                continue;
            }
            let owner = owners.entry(pid).or_insert_with(|| identity(pid)).clone();
            if scope.as_ref().is_some_and(|paths| {
                !owner
                    .as_ref()
                    .is_some_and(|o| paths.iter().any(|p| p.eq_ignore_ascii_case(&o.executable)))
            }) {
                continue;
            }
            let mut class = [0u16; 256];
            let n = GetClassNameW(hwnd, &mut class).max(0) as usize;
            let class = String::from_utf16_lossy(&class[..n]);
            let elevated = *elevations.entry(pid).or_insert_with(|| elevated(pid));
            let (package, restart) = activations
                .entry(pid)
                .or_insert_with(|| activation(pid, elevated == Some(false)))
                .clone();
            let mut title = [0u16; 2048];
            let n = GetWindowTextW(hwnd, &mut title).max(0) as usize;
            // GetWindowRect is DPI-virtualized for its caller. Observe it in the
            // target window's coordinate space, not the Tauri worker's DPI context.
            let dpi = crate::app_dpi::for_window(hwnd);
            let geometry_space = dpi.as_ref().and_then(|(_, context)| {
                let value = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd);
                (value > 0).then_some(crate::reconstruction::GeometrySpace {
                    dpi_context: *context,
                    window_dpi: value,
                    primary_size: [GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)],
                })
            });
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                continue;
            }
            let visible = IsWindowVisible(hwnd).as_bool();
            let mut placement = WINDOWPLACEMENT {
                length: size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let has_placement = GetWindowPlacement(hwnd, &mut placement).is_ok();
            drop(dpi);
            let owner_hwnd = GetWindow(hwnd, GW_OWNER)
                .ok()
                .map(|h| (h.0 as usize).to_string());
            // GetAncestor(GA_PARENT) returns the desktop for unowned top-level windows;
            // GetParent returns zero there and the owner for owned popup windows.
            let parent_hwnd = GetParent(hwnd).ok().map(|h| (h.0 as usize).to_string());
            let (app_id, relaunch) = if visible {
                properties(hwnd)
            } else {
                (None, "unavailable".into())
            };
            let mut checked = 0;
            if GetWindowThreadProcessId(hwnd, Some(&mut checked)) != tid || checked != pid {
                continue;
            }
            let owner = owner.filter(|o| {
                crate::platform::process_started(pid).as_deref() == Some(&o.process_started)
            });
            rows.push(Window {
                handle: handle.to_string(),
                thread_id: tid,
                title: String::from_utf16_lossy(&title[..n]),
                class,
                visible,
                owner,
                owner_pid: pid,
                parent_pid: table.get(&pid).map(|r| r.0),
                same_owner_windows: 0,
                elevated,
                app_user_model_id: app_id,
                relaunch_metadata: relaunch,
                placement: [rect.left, rect.top, rect.right, rect.bottom],
                evidence: NativeEvidence {
                    geometry_space,
                    package,
                    restart,
                    owner_hwnd: Some(owner_hwnd.unwrap_or_else(|| "0".into())),
                    parent_hwnd: Some(parent_hwnd.unwrap_or_else(|| "0".into())),
                    show_state: has_placement.then_some(placement.showCmd),
                    normal_rect: has_placement.then_some([
                        placement.rcNormalPosition.left,
                        placement.rcNormalPosition.top,
                        placement.rcNormalPosition.right,
                        placement.rcNormalPosition.bottom,
                    ]),
                },
            });
        }
        // Include hidden and untitled windows in process sharing evidence. This count
        // is deliberately conservative, and is never proof of exclusive ownership.
        let mut counts = HashMap::new();
        for w in &rows {
            *counts.entry(w.owner_pid).or_insert(0usize) += 1;
        }
        for w in &mut rows {
            w.same_owner_windows = counts[&w.owner_pid];
        }
        rows.retain(|w| {
            w.visible
                && !w.title.is_empty()
                && !matches!(
                    w.class.as_str(),
                    "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
                )
        });
        Ok(rows)
    })
    .join()
    .map_err(|_| "Windows inventory worker failed".to_string())?
}

pub fn same_observation(a: &Window, b: &Window) -> bool {
    a.owner.is_some()
        && a.owner == b.owner
        && a.handle == b.handle
        && a.thread_id == b.thread_id
        && a.class == b.class
        && a.title == b.title
        && a.app_user_model_id == b.app_user_model_id
        && a.relaunch_metadata == b.relaunch_metadata
        && a.elevated == b.elevated
        && a.same_owner_windows == b.same_owner_windows
        && a.evidence.package == b.evidence.package
        && a.evidence.owner_hwnd == b.evidence.owner_hwnd
        && a.evidence.parent_hwnd == b.evidence.parent_hwnd
}

pub fn capabilities(w: &Window) -> Value {
    let known = w.owner.is_some();
    json!({"discovery":true,"contextCapture":known,"reviewedClose":false,
    "relaunch":"checked-at-capture-and-restore","semanticRestore":false,"verification":"application-presence-only",
    "limitation":"Window and executable metadata only. Saved documents, unsaved work and internal app state are unknown. This window stays open during Sleep.",
    "reconstruction":crate::reconstruction::assess(w, match crate::app_context::launch_policy(w) {
        Ok(_) => crate::reconstruction::Recipe::Unchecked,
        Err(reason) => crate::reconstruction::Recipe::Blocked(reason),
    })})
}
