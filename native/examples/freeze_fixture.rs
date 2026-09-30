//! Disposable GUI with volatile state and observable heartbeat. Never packaged.
#![windows_subsystem = "windows"]
use std::{
    os::windows::process::CommandExt,
    path::PathBuf,
    time::{Duration, Instant},
};
use windows::{
    core::w,
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
};
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CLOSE {
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
fn main() -> windows::core::Result<()> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("disposable fixture directory"),
    );
    assert!(root.join("fixture-authorized").is_file());
    let scenario: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("scenario.json")).unwrap()).unwrap();
    let mb = scenario["megabytes"].as_u64().unwrap_or(1).min(1024) as usize;
    let mut memory = vec![0u8; mb * 1024 * 1024];
    for (i, p) in memory.chunks_mut(4096).enumerate() {
        p[0] = (i % 251) as u8;
    }
    let secret = uuid::Uuid::new_v4().to_string(); // exists only in this process; compared across Wake
    let pid = std::process::id();
    std::fs::write(
        root.join("identity.json"),
        serde_json::to_vec(&hibernate_native::app_windows::identity(pid).unwrap()).unwrap(),
    )
    .unwrap();
    let mut count = 0u64;
    if scenario["network"] == true {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        std::fs::write(root.join("endpoint"), endpoint.to_string()).unwrap();
        let network_root = root.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(180);
            while !network_root.join("stop").exists() && Instant::now() < deadline {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    let mut byte = [0u8];
                    if stream.read_exact(&mut byte).is_ok() {
                        let _ = stream.write_all(&byte);
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
    }
    let spawn_child = || {
        let child_root = root.join("child");
        std::fs::create_dir_all(&child_root).unwrap();
        std::fs::write(child_root.join("fixture-authorized"), "owned child").unwrap();
        std::fs::write(child_root.join("scenario.json"), "{\"worker\":true}").unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg(&child_root)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        // Child has an independent bounded lifetime and cooperative stop file.
        std::fs::write(root.join("child-pid"), child.id().to_string()).unwrap();
        // Reap while the parent lives, without making parent exit wait for this
        // intentionally independent worker. Rust main exit ends the waiter too.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    };
    let mut child_started = false;
    if scenario["child"] == true {
        spawn_child();
        child_started = true;
    }
    unsafe {
        let _ = windows::Win32::UI::Input::Ime::ImmDisableIME(u32::MAX);
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            lpszClassName: w!("ContextSleepFreezeFixture"),
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&class), 0);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("ContextSleep disposable freeze fixture"),
            WS_OVERLAPPEDWINDOW,
            80,
            80,
            440,
            200,
            None,
            None,
            None,
            None,
        )?;
        if scenario["worker"] != true {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        let second = if scenario["twoWindows"] == true {
            let w = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class.lpszClassName,
                w!("ContextSleep second disposable window"),
                WS_OVERLAPPEDWINDOW,
                150,
                150,
                440,
                200,
                None,
                None,
                None,
                None,
            )?;
            let _ = ShowWindow(w, SW_SHOWNOACTIVATE);
            Some(w)
        } else {
            None
        };
        let start = Instant::now();
        let mut message = MSG::default();
        while !root.join("stop").exists() && start.elapsed() < Duration::from_secs(180) {
            if !child_started && root.join("spawn-child").exists() {
                spawn_child();
                child_started = true;
            }
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let valid = memory
                .chunks(4096)
                .enumerate()
                .all(|(i, p)| std::hint::black_box(p[0]) == (i % 251) as u8);
            count += 1;
            let report = serde_json::json!({"pid":pid,"count":count,"secret":secret,"valid":valid,"megabytes":mb});
            // Publish atomically: freezing between truncate/write must not leave
            // the observer waiting on an incomplete JSON file until Wake.
            std::fs::write(root.join("heartbeat.next"), report.to_string()).unwrap();
            std::fs::rename(root.join("heartbeat.next"), root.join("heartbeat.json")).unwrap();
            if scenario["threadChurn"] == true {
                let _ = std::thread::spawn(|| std::hint::black_box(42)).join();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        DestroyWindow(hwnd)?;
        if let Some(w) = second {
            DestroyWindow(w)?;
        }
    }
    Ok(())
}
