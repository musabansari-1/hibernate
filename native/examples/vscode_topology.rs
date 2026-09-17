//! Read-only fixture probe, deliberately outside the shipped host/desktop API.
use hibernate_native::{platform, store};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    mem::size_of,
};
use windows::Win32::{Foundation::CloseHandle, System::Diagnostics::ToolHelp::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let owner = platform::vscode_owner()?;
    let owner_pid = owner["pid"].as_u64().ok_or("Missing owner")? as u32;
    let mut parents = HashMap::new();
    let mut code = HashSet::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            let len = entry
                .szExeFile
                .iter()
                .position(|n| *n == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case("Code.exe") {
                code.insert(entry.th32ProcessID);
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        CloseHandle(snapshot)?;
    }
    let mut ancestry = vec![];
    let mut cursor = std::process::id();
    for _ in 0..32 {
        ancestry.push(platform::process_identity(cursor));
        if cursor == owner_pid {
            break;
        }
        cursor = *parents.get(&cursor).ok_or("Missing parent")?;
    }
    let mut processes = vec![];
    for pid in code {
        let mut cursor = pid;
        for _ in 0..32 {
            if cursor == owner_pid {
                processes.push(json!({"identity":platform::process_identity(pid),"parent":parents.get(&pid),"windows":platform::vscode_windows(pid)?}));
                break;
            }
            let Some(parent) = parents.get(&cursor).filter(|p| **p != cursor) else {
                break;
            };
            cursor = *parent;
        }
    }
    let c = store::open()?;
    let observations: i64 =
        c.query_row("SELECT COUNT(*) FROM native_close_observations", [], |r| {
            r.get(0)
        })?;
    println!(
        "{}",
        json!({"owner":owner,"windows":platform::vscode_windows(owner_pid)?,"ancestry":ancestry,"processes":processes,"nativeCloseObservationCount":observations})
    );
    Ok(())
}
