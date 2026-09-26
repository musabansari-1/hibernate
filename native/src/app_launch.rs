//! Creation-time placement hints only. No HWND mutation or generic close primitive.
use crate::app_windows::{ProcessIdentity, Window};
use serde_json::{json, Value};
use std::{mem::size_of, path::Path};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::{Environment::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
};
type R<T> = Result<T, String>;

pub struct Launched {
    handle: HANDLE,
    pid: u32,
    executable: String,
    pub placement_requested: bool,
}
impl Drop for Launched {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
impl Launched {
    pub fn identity(&self) -> Option<ProcessIdentity> {
        unsafe {
            let mut created = FILETIME::default();
            let mut exited = created;
            let mut kernel = created;
            let mut user = created;
            GetProcessTimes(
                self.handle,
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
            .ok()?;
            Some(ProcessIdentity {
                pid: self.pid,
                executable: self.executable.clone(),
                process_started: (((created.dwHighDateTime as u64) << 32)
                    | created.dwLowDateTime as u64)
                    .to_string(),
            })
        }
    }
}

fn inherited_environment() -> R<Vec<u16>> {
    unsafe {
        let block = GetEnvironmentStringsW();
        if block.is_null() {
            return Err("Environment unavailable".into());
        }
        struct Environment(PWSTR);
        impl Drop for Environment {
            fn drop(&mut self) {
                unsafe {
                    let _ = FreeEnvironmentStringsW(PCWSTR(self.0 .0));
                }
            }
        }
        let _owned = Environment(block);
        let mut result = Vec::new();
        let mut offset = 0;
        while offset < 1_048_576 {
            if *block.0.add(offset) == 0 {
                result.push(0);
                if result.len() == 1 {
                    result.push(0);
                }
                return Ok(result);
            }
            let start = offset;
            while offset < 1_048_576 && *block.0.add(offset) != 0 {
                offset += 1;
            }
            if offset == 1_048_576 {
                break;
            }
            let entry = std::slice::from_raw_parts(block.0.add(start), offset - start);
            if !String::from_utf16_lossy(entry)
                .to_ascii_uppercase()
                .starts_with("ELECTRON_RUN_AS_NODE=")
            {
                result.extend_from_slice(entry);
                result.push(0);
            }
            offset += 1;
        }
        Err("Environment exceeds launch bounds".into())
    }
}

/// Caller holds the executable file lock and has durably claimed the launch.
pub fn launch(path: &str, window: &Window) -> R<Launched> {
    let executable: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut command: Vec<u16> = format!("\"{path}\"")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let cwd: Vec<u16> = Path::new(path)
        .parent()
        .ok_or("Missing executable directory")?
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let environment = inherited_environment()?;
    let dpi = window
        .evidence
        .geometry_space
        .as_ref()
        .and_then(|g| crate::app_dpi::enter(g.dpi_context));
    unsafe {
        let mut info = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let requested = crate::reconstruction::placement_request(window).filter(|rect| {
            dpi.is_some()
                && window.evidence.geometry_space.as_ref().is_some_and(|g| {
                    g.primary_size == [GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)]
                })
                && rect[2] <= GetSystemMetrics(SM_CXSCREEN)
                && rect[3] <= GetSystemMetrics(SM_CYSCREEN)
        });
        if let Some([left, top, right, bottom]) = requested {
            info.dwFlags = STARTF_USEPOSITION | STARTF_USESIZE | STARTF_USESHOWWINDOW;
            info.dwX = left as u32;
            info.dwY = top as u32;
            info.dwXSize = (right - left) as u32;
            info.dwYSize = (bottom - top) as u32;
            info.wShowWindow = SW_SHOWNOACTIVATE.0 as u16;
        }
        let mut process = PROCESS_INFORMATION::default();
        CreateProcessW(
            PCWSTR(executable.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            Some(environment.as_ptr().cast()),
            PCWSTR(cwd.as_ptr()),
            &info,
            &mut process,
        )
        .map_err(|e| e.to_string())?;
        let _ = CloseHandle(process.hThread);
        Ok(Launched {
            handle: process.hProcess,
            pid: process.dwProcessId,
            executable: path.into(),
            placement_requested: requested.is_some(),
        })
    }
}

pub fn placement_result(saved: &Window, observed: &[Window], requested: bool) -> Value {
    let matched = requested
        && saved.evidence.geometry_space.is_some()
        && observed.len() == 1
        && observed[0].same_owner_windows == 1
        && observed[0].class == saved.class
        && observed[0].placement == saved.placement
        && observed[0].evidence.geometry_space == saved.evidence.geometry_space
        && observed[0].evidence.show_state == Some(1);
    json!({"requested":requested,"observedMatch":matched,"expectedRectangle":saved.placement,
        "observedRectangles":observed.iter().map(|w| w.placement).collect::<Vec<_>>(),
        "coordinateSpace":saved.evidence.geometry_space,
        "status":if matched {"geometry-observed"} else if requested {"unverified"} else {"not-attempted"},
        "detail":"Creation-time geometry hint only. An observed rectangle in the launched process does not verify logical window identity, documents, layout or working state."})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_or_different_windows_cannot_verify_geometry() {
        let mut saved = crate::app_context::tests::window();
        saved.same_owner_windows = 1;
        saved.evidence.show_state = Some(1);
        saved.evidence.geometry_space = Some(crate::reconstruction::GeometrySpace {
            dpi_context: -1,
            window_dpi: 96,
            primary_size: [1920, 1080],
        });
        assert_eq!(
            placement_result(&saved, &[saved.clone()], true)["observedMatch"],
            true
        );
        assert_eq!(
            placement_result(&saved, &[saved.clone()], false)["observedMatch"],
            false
        );
        let mut other = saved.clone();
        other.evidence.geometry_space.as_mut().unwrap().window_dpi = 144;
        assert_eq!(
            placement_result(&saved, &[other], true)["observedMatch"],
            false
        );
        let mut other = saved.clone();
        other.class = "Different".into();
        assert_eq!(
            placement_result(&saved, &[other], true)["observedMatch"],
            false
        );
        assert_eq!(
            placement_result(&saved, &[saved.clone(), saved.clone()], true)["observedMatch"],
            false
        );
    }
}
