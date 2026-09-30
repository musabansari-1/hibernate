//! Owned Windows process-state mechanism. Authority is issued by deep_freeze,
//! never by serialized identities or journals. No count-based resume fallback.
use crate::app_windows::ProcessIdentity;
pub mod journal;
use serde::{Deserialize, Serialize};
use std::{ffi::c_void, mem::size_of, ptr::null_mut};
use windows::{
    core::{BOOL, PCSTR, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, HMODULE, WAIT_TIMEOUT},
        System::{ProcessStatus::*, SystemInformation::*, Threading::*},
    },
};
type R<T> = Result<T, String>;
// Resolve loaded ntdll only. No DLL search paths, system-call numbers or fallback.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: PCWSTR) -> HMODULE;
    fn GetProcAddress(module: HMODULE, name: PCSTR) -> *mut c_void;
}
type Create =
    unsafe extern "system" fn(*mut HANDLE, u32, *mut ObjectAttributes, HANDLE, u32) -> i32;
type Change = unsafe extern "system" fn(HANDLE, HANDLE, u32, *mut c_void, usize, u32) -> i32;
#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root: HANDLE,
    name: *mut c_void,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_qos: *mut c_void,
}
struct Handle(HANDLE);
// Kernel process/state handles have no thread affinity. Each wrapper uniquely owns
// its handle; the controller serializes use and drops through a Mutex. Not Sync.
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn functions() -> R<(Create, Change)> {
    unsafe {
        let module = GetModuleHandleW(windows::core::w!("ntdll.dll"));
        if module.is_invalid() {
            return Err("Loaded ntdll is unavailable".into());
        }
        let create = GetProcAddress(module, windows::core::s!("NtCreateProcessStateChange"));
        let change = GetProcAddress(module, windows::core::s!("NtChangeProcessState"));
        if create.is_null() || change.is_null() {
            return Err("Windows process-state APIs unavailable; no suspension fallback".into());
        }
        Ok((
            std::mem::transmute::<*mut c_void, Create>(create),
            std::mem::transmute::<*mut c_void, Change>(change),
        ))
    }
}
/// Handle-release behavior has been exercised on this build only. Export
/// availability alone is not evidence that crash recovery is safe elsewhere.
pub(crate) fn validated_os() -> R<()> {
    #[repr(C)]
    struct Version {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service: [u16; 128],
    }
    unsafe {
        let module = GetModuleHandleW(windows::core::w!("ntdll.dll"));
        let address = GetProcAddress(module, windows::core::s!("RtlGetVersion"));
        if address.is_null() {
            return Err("Windows version evidence is unavailable".into());
        }
        let query = std::mem::transmute::<
            *mut c_void,
            unsafe extern "system" fn(*mut Version) -> i32,
        >(address);
        let mut version = Version {
            size: size_of::<Version>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service: [0; 128],
        };
        status(query(&mut version), "Read Windows version")?;
        if !cfg!(target_arch = "x86_64") || version.major != 10 || version.build != 26200 {
            return Err(
                "Deep Freeze has not passed crash-release validation on this Windows build".into(),
            );
        }
    }
    Ok(())
}
fn status(value: i32, operation: &str) -> R<()> {
    // Require STATUS_SUCCESS, not informational/pending statuses.
    if value == 0 {
        Ok(())
    } else {
        Err(format!("{operation}: NTSTATUS {:#010x}", value as u32))
    }
}
fn identity(handle: HANDLE, pid: u32) -> R<ProcessIdentity> {
    unsafe {
        if WaitForSingleObject(handle, 0) != WAIT_TIMEOUT {
            return Err("Target ended or liveness unavailable".into());
        }
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user)
            .map_err(|e| e.to_string())?;
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
        .map_err(|e| e.to_string())?;
        Ok(ProcessIdentity {
            pid,
            process_started: (((created.dwHighDateTime as u64) << 32)
                | created.dwLowDateTime as u64)
                .to_string(),
            executable: String::from_utf16_lossy(&buffer[..length as usize]),
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEvidence {
    pub at: u64,
    pub working_set: u64,
    pub private_commit: u64,
    pub available_physical: u64,
    pub total_physical: u64,
    pub page_fault_count: u32,
}
/// A held exact process and a unique, non-inherited state object. Dropping the
/// object is expected to release only its freeze; the harness verifies this.
pub struct Lease {
    state: Option<Handle>,
    process: Handle,
    expected: ProcessIdentity,
    change: Change,
    frozen: bool,
}
impl Lease {
    /// Validation only. The caller must own a disposable process and verify its
    /// launch receipt. This is not a production safety assessment.
    #[cfg(feature = "deep-freeze-lab")]
    pub fn open_fixture(expected: &ProcessIdentity) -> R<Self> {
        Self::open(expected)
    }
    pub(crate) fn open(expected: &ProcessIdentity) -> R<Self> {
        if expected.pid == std::process::id() || expected.pid <= 4 {
            return Err("Cannot freeze self/system".into());
        }
        let (create, change) = functions()?;
        unsafe {
            let process = Handle(
                OpenProcess(
                    PROCESS_QUERY_INFORMATION
                        | PROCESS_VM_READ
                        | PROCESS_SET_INFORMATION
                        | PROCESS_SUSPEND_RESUME
                        | PROCESS_SET_QUOTA
                        | PROCESS_ACCESS_RIGHTS(SYNCHRONIZATION_SYNCHRONIZE.0),
                    false,
                    expected.pid,
                )
                .map_err(|e| e.to_string())?,
            );
            if identity(process.0, expected.pid)? != *expected {
                return Err("Process identity changed".into());
            }
            let mut critical = BOOL(1);
            IsProcessCritical(process.0, &mut critical).map_err(|e| e.to_string())?;
            if critical.as_bool() {
                return Err("Critical process blocked".into());
            }
            let mut attrs = ObjectAttributes {
                length: size_of::<ObjectAttributes>() as u32,
                root: HANDLE::default(),
                name: null_mut(),
                attributes: 0x20, /* OBJ_EXCLUSIVE, no inheritance */
                security_descriptor: null_mut(),
                security_qos: null_mut(),
            };
            let mut state = HANDLE::default();
            status(
                create(
                    &mut state, 1, /* STATECHANGE_SET_ATTRIBUTES */
                    &mut attrs, process.0, 0,
                ),
                "Create process state",
            )?;
            if state.is_invalid() {
                return Err("Native API returned an invalid state handle".into());
            }
            Ok(Self {
                state: Some(Handle(state)),
                process,
                expected: expected.clone(),
                change,
                frozen: false,
            })
        }
    }
    /// All security observations use the retained process handle. Successful open
    /// alone is insufficient: even an elevated controller refuses elevated apps.
    pub(crate) fn security(&self) -> R<bool> {
        use windows::Win32::{
            Foundation::APPMODEL_ERROR_NO_PACKAGE, Security::*,
            Storage::Packaging::Appx::GetPackageFullName, System::JobObjects::IsProcessInJob,
        };
        unsafe fn token_value<T: Default>(h: HANDLE, class: TOKEN_INFORMATION_CLASS) -> R<T> {
            let mut value = T::default();
            let mut length = 0;
            GetTokenInformation(
                h,
                class,
                Some(&mut value as *mut _ as *mut _),
                size_of::<T>() as u32,
                &mut length,
            )
            .map_err(|e| e.to_string())?;
            if length as usize != size_of::<T>() {
                return Err("Unexpected security evidence size".into());
            }
            Ok(value)
        }
        unsafe fn token(h: HANDLE) -> R<Handle> {
            let mut t = HANDLE::default();
            OpenProcessToken(h, TOKEN_QUERY, &mut t).map_err(|e| e.to_string())?;
            Ok(Handle(t))
        }
        unsafe {
            if identity(self.process.0, self.expected.pid)? != self.expected {
                return Err("Identity changed".into());
            }
            let target = token(self.process.0)?;
            let current = token(GetCurrentProcess())?;
            let elevation: TOKEN_ELEVATION = token_value(target.0, TokenElevation)?;
            if elevation.TokenIsElevated != 0 {
                return Err("Elevated apps are unavailable for Deep Freeze".into());
            }
            let target_stats: TOKEN_STATISTICS = token_value(target.0, TokenStatistics)?;
            let current_stats: TOKEN_STATISTICS = token_value(current.0, TokenStatistics)?;
            let target_session: u32 = token_value(target.0, TokenSessionId)?;
            let current_session: u32 = token_value(current.0, TokenSessionId)?;
            if target_stats.AuthenticationId != current_stats.AuthenticationId
                || target_session == 0
                || target_session != current_session
            {
                return Err("Same interactive logon and session are required".into());
            }
            let mut protection = PROCESS_PROTECTION_LEVEL_INFORMATION::default();
            GetProcessInformation(
                self.process.0,
                ProcessProtectionLevelInfo,
                &mut protection as *mut _ as *mut _,
                size_of::<PROCESS_PROTECTION_LEVEL_INFORMATION>() as u32,
            )
            .map_err(|e| e.to_string())?;
            if protection.ProtectionLevel != PROTECTION_LEVEL_NONE {
                return Err("Protected apps are excluded".into());
            }
            let mut job = BOOL(1);
            IsProcessInJob(self.process.0, None, &mut job).map_err(|e| e.to_string())?;
            let mut length = 0;
            if GetPackageFullName(self.process.0, &mut length, None) != APPMODEL_ERROR_NO_PACKAGE {
                return Err("Packaged or unknown app identity is excluded".into());
            }
            // Observation only: the caller must validate an independent app
            // contract. No job is opened, reassigned, modified or suspended.
            Ok(job.as_bool())
        }
    }
    pub fn freeze(&mut self) -> R<()> {
        if self.frozen {
            return Err("Already frozen by this lease".into());
        }
        if identity(self.process.0, self.expected.pid)? != self.expected {
            return Err("Process identity changed".into());
        }
        let state = self.state.as_ref().ok_or("Lease was released")?;
        let result = unsafe {
            status(
                (self.change)(state.0, self.process.0, 0, null_mut(), 0, 0),
                "Freeze",
            )
        };
        if let Err(error) = result {
            // Even an uncertain native outcome cannot leave an untracked freeze.
            self.state.take();
            return Err(error);
        }
        self.frozen = true;
        Ok(())
    }
    pub fn wake(&mut self) -> R<()> {
        if !self.frozen {
            return Ok(());
        }
        let state = self.state.as_ref().ok_or("Lease was released")?;
        // Use the retained handle, never reopen by PID and never decrement generic counts.
        let result = unsafe {
            status(
                (self.change)(state.0, self.process.0, 1, null_mut(), 0, 0),
                "Wake",
            )
        };
        self.frozen = false;
        self.state.take();
        result
    }
    pub fn measure(&self) -> R<MemoryEvidence> {
        measure(self.process.0, &self.expected)
    }
    pub fn trim(&self) -> R<()> {
        if !self.frozen || self.state.is_none() {
            return Err("Trim requires this lease's confirmed freeze".into());
        }
        if identity(self.process.0, self.expected.pid)? != self.expected {
            return Err("Process identity changed".into());
        }
        unsafe { EmptyWorkingSet(self.process.0).map_err(|e| e.to_string()) }
    }
}
pub(crate) fn observe_memory(expected: &ProcessIdentity) -> R<MemoryEvidence> {
    let h = Handle(
        unsafe {
            OpenProcess(
                PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_READ
                    | PROCESS_ACCESS_RIGHTS(SYNCHRONIZATION_SYNCHRONIZE.0),
                false,
                expected.pid,
            )
        }
        .map_err(|e| e.to_string())?,
    );
    measure(h.0, expected)
}
fn measure(process: HANDLE, expected: &ProcessIdentity) -> R<MemoryEvidence> {
    if identity(process, expected.pid)? != *expected {
        return Err("Process identity changed".into());
    }
    unsafe {
        let mut p = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        GetProcessMemoryInfo(
            process,
            &mut p as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
        .map_err(|e| e.to_string())?;
        let mut m = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        GlobalMemoryStatusEx(&mut m).map_err(|e| e.to_string())?;
        Ok(MemoryEvidence {
            at: crate::now(),
            working_set: p.WorkingSetSize as u64,
            private_commit: p.PrivateUsage as u64,
            available_physical: m.ullAvailPhys,
            total_physical: m.ullTotalPhys,
            page_fault_count: p.PageFaultCount,
        })
    }
}
