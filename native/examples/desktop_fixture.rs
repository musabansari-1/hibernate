//! Isolated generic desktop fixture. Its own timer handles cleanup through adjacent marker files.
//! This executable is not packaged or exposed through the production bridge.
#![windows_subsystem = "windows"]
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::WindowsAndMessaging::*,
    },
};
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLOSE {
        // Simulate an application with unresolved work. Generic code must never send this.
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::write(exe.with_extension("close-requested"), b"unexpected close");
        }
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}
fn main() -> windows::core::Result<()> {
    unsafe {
        // Make the fixture's top-level window count deterministic, excluding OS-created IME helpers.
        let _ = windows::Win32::UI::Input::Ime::ImmDisableIME(u32::MAX);
        let exe = std::env::current_exe().unwrap();
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(exe.with_extension("launches"))
            .unwrap();
        use std::io::Write;
        writeln!(log, "{}", std::process::id()).unwrap();
        let scenario: serde_json::Value = std::fs::read(exe.with_extension("scenario.json"))
            .ok()
            .and_then(|v| serde_json::from_slice(&v).ok())
            .unwrap_or_default();
        let single = scenario["single"] == true;
        let startup_geometry = scenario["startupGeometry"] == true;
        if scenario["restart"] == true {
            windows::Win32::System::Recovery::RegisterApplicationRestart(
                w!("--restore-document fixture.txt"),
                windows::Win32::System::Recovery::REGISTER_APPLICATION_RESTART_FLAGS(0),
            )?;
        }
        let mut evidence = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(exe.with_extension("observations.jsonl"))
            .unwrap();
        writeln!(evidence, "{}", serde_json::json!({"pid":std::process::id(), "args":std::env::args().skip(1).collect::<Vec<_>>(),"cwd":std::env::current_dir().unwrap(),"unsaved":scenario["unsaved"] == true})).unwrap();
        let _unsaved = (scenario["unsaved"] == true).then(|| String::from("fixture unsaved edits"));
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            lpszClassName: if scenario["catalogClass"] == true {
                w!("SUMATRA_PDF_FRAME")
            } else {
                w!("ContextSleepIsolatedFixture")
            },
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&class), 0);
        let mut handles = Vec::new();
        for (i, title) in [
            "ContextSleep fixture Alpha",
            "ContextSleep fixture Beta",
            "ContextSleep fixture hidden",
        ]
        .iter()
        .enumerate()
        {
            if single && i > 0 {
                break;
            }
            let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class.lpszClassName,
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW,
                if startup_geometry {
                    CW_USEDEFAULT
                } else {
                    80 + i as i32 * 40
                },
                if startup_geometry {
                    CW_USEDEFAULT
                } else {
                    80 + i as i32 * 40
                },
                if startup_geometry { CW_USEDEFAULT } else { 420 },
                if startup_geometry { CW_USEDEFAULT } else { 240 },
                None,
                None,
                None,
                None,
            )?;
            if i < 2 {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            handles.push(hwnd);
        }
        let start = Instant::now();
        let stop: PathBuf = exe.with_extension("stop");
        let mut message = MSG::default();
        loop {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if stop.exists() || start.elapsed() > Duration::from_secs(600) {
                break;
            }
            let recreate = exe.with_extension("recreate");
            if recreate.exists() {
                for hwnd in handles.drain(..) {
                    DestroyWindow(hwnd)?;
                }
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class.lpszClassName,
                    w!("ContextSleep fixture recreated"),
                    WS_OVERLAPPEDWINDOW,
                    100,
                    100,
                    420,
                    240,
                    None,
                    None,
                    None,
                    None,
                )?;
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                handles.push(hwnd);
                let _ = std::fs::remove_file(recreate);
                std::fs::write(exe.with_extension("recreated"), "window replaced").unwrap();
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        for hwnd in handles {
            DestroyWindow(hwnd)?;
        }
    }
    Ok(())
}
