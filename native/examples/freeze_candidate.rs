//! Opt-in candidate research, confined to a downloaded private portable copy.
#[cfg(feature = "deep-freeze-lab")]
fn main() -> Result<(), String> {
    use hibernate_native::{app_context, app_windows, deep_freeze};
    use std::{
        fs,
        os::windows::process::CommandExt,
        path::Path,
        time::{Duration, Instant},
    };
    use windows::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
    let manifest = env!("CARGO_MANIFEST_DIR").replace('/', "\\");
    let root = Path::new(&manifest)
        .parent()
        .unwrap()
        .join(".validation")
        .join("deep-freeze-catalog")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(root.join("profile")).map_err(|e| e.to_string())?;
    let source = root
        .parent()
        .unwrap()
        .join("sumatra-3.6.1/SumatraPDF-3.6.1-64.exe");
    let exe = root.join("SumatraPDF.exe");
    let bytes = fs::read(&source).map_err(|e| e.to_string())?;
    use sha2::{Digest, Sha256};
    if format!("{:x}", Sha256::digest(&bytes))
        != "719f689b34f47be8ca105ce8484948474dafde0e106bab599e4a89326070c3d0"
    {
        return Err("Unrecognized candidate image".into());
    }
    fs::write(&exe, bytes).map_err(|e| e.to_string())?;
    fs::write(root.join("profile/SumatraPDF-settings.txt"),"ReuseInstance = false\nRememberOpenedFiles = false\nRememberStatePerDocument = false\nCheckForUpdates = false\n").map_err(|e|e.to_string())?;
    // The original official portable binary and its personal settings are never run.
    let mut child = std::process::Command::new(&exe)
        .args(["-new-window", "-restrict", "-appdata"])
        .arg(root.join("profile"))
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let pid = child.id();
    let expected = app_windows::identity(pid).ok_or("Candidate identity unavailable")?;
    let test = (|| {
        // Read-only launch diagnostics distinguish host restrictions from app
        // behavior. They never grant freeze authority or bypass eligibility.
        let launch_evidence = (|| -> Result<serde_json::Value, String> {
            use windows::Win32::System::{JobObjects::IsProcessInJob, Threading::*};
            let mut controller_job = windows::core::BOOL(0);
            let mut target_job = windows::core::BOOL(0);
            unsafe {
                IsProcessInJob(GetCurrentProcess(), None, &mut controller_job)
                    .map_err(|e| e.to_string())?;
                let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                    .map_err(|e| e.to_string())?;
                let result = IsProcessInJob(process, None, &mut target_job);
                let _ = CloseHandle(process);
                result.map_err(|e| e.to_string())?;
            }
            let table = app_windows::process_table()?;
            Ok(
                serde_json::json!({"controllerInJob":controller_job.as_bool(),"targetInJob":target_job.as_bool(),
                "controller":app_windows::identity(std::process::id()),
                "parent":table.get(&pid).and_then(|r|app_windows::identity(r.0)),"target":expected}),
            )
        })();
        fs::write(
            root.join("launch-evidence.json"),
            serde_json::to_vec_pretty(&launch_evidence).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            root.join("identity.json"),
            serde_json::to_vec_pretty(&expected).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        std::env::set_var("HIBERNATE_DATA_DIR", root.join("data"));
        std::env::set_var(
            "HIBERNATE_TEST_DISCOVERY_EXECUTABLES",
            serde_json::to_string(&vec![exe.clone()]).unwrap(),
        );
        let start = Instant::now();
        let before = loop {
            let rows = app_windows::inventory()?;
            if !rows.is_empty() {
                break rows;
            }
            if start.elapsed() > Duration::from_secs(10) {
                return Err("Candidate window unavailable".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        deep_freeze::initialize()?;
        let discovered = deep_freeze::dashboard(&app_context::discover()?)?;
        let selected = &discovered["apps"][0];
        if std::env::args().any(|a| a == "--expect-refusal") {
            if selected["strategy"] == "GenericDeepFreeze" {
                return Err("Unreleased candidate unexpectedly enabled".into());
            }
            assert!(deep_freeze::review(vec![selected["id"].as_str().unwrap().into()]).is_err());
            assert!(deep_freeze::sleep("forged", true).is_err());
            assert!(deep_freeze::wake("forged").is_err());
            assert_eq!(app_windows::identity(pid).as_ref(), Some(&expected));
            assert!(discovered["operations"].as_array().unwrap().is_empty());
            let evidence = serde_json::json!({"scope":"production refusal only; no suspend invoked","selection":selected,"identityPreserved":true,"journalEmpty":true});
            fs::write(
                root.join("refusal.json"),
                serde_json::to_vec_pretty(&evidence).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            println!("Refusal evidence: {}", root.join("refusal.json").display());
            return Ok(());
        }
        if selected["strategy"] != "GenericDeepFreeze" {
            return Err(format!("Candidate refused: {selected}"));
        }
        assert!(deep_freeze::review(vec!["forged".into()]).is_err());
        let selection_id = selected["id"].as_str().unwrap().to_string();
        assert!(deep_freeze::review(vec![selection_id.clone(), selection_id.clone()]).is_err());
        let canceled = deep_freeze::review(vec![selection_id.clone()])?;
        deep_freeze::cancel_review(canceled["id"].as_str().unwrap())?;
        assert!(deep_freeze::sleep(canceled["id"].as_str().unwrap(), true).is_err());
        let unacknowledged = deep_freeze::review(vec![selection_id.clone()])?;
        assert!(deep_freeze::sleep(unacknowledged["id"].as_str().unwrap(), false).is_err());
        let changed = deep_freeze::review(vec![selection_id.clone()])?;
        let window = before
            .iter()
            .find(|w| w.class == "SUMATRA_PDF_FRAME")
            .ok_or("Missing candidate frame")?;
        let hwnd = HWND(window.handle.parse::<usize>().map_err(|e| e.to_string())? as *mut _);
        unsafe {
            SetWindowTextW(hwnd, windows::core::w!("Changed private review fixture"))
                .map_err(|e| e.to_string())?;
        }
        assert!(deep_freeze::sleep(changed["id"].as_str().unwrap(), true).is_err());
        let title: Vec<u16> = window.title.encode_utf16().chain(Some(0)).collect();
        unsafe {
            SetWindowTextW(hwnd, windows::core::PCWSTR(title.as_ptr()))
                .map_err(|e| e.to_string())?;
        }
        let expired = deep_freeze::review(vec![selection_id])?;
        std::thread::sleep(Duration::from_secs(21));
        assert!(deep_freeze::sleep(expired["id"].as_str().unwrap(), true).is_err());
        let refreshed = deep_freeze::dashboard(&app_context::discover()?)?;
        let selected = &refreshed["apps"][0];
        let review = deep_freeze::review(vec![selected["id"].as_str().unwrap().into()])?;
        let id = deep_freeze::sleep(review["id"].as_str().unwrap(), true)?;
        assert!(deep_freeze::sleep(review["id"].as_str().unwrap(), true).is_err());
        std::thread::sleep(Duration::from_secs(10));
        let frozen = deep_freeze::dashboard(&app_context::discover()?)?;
        deep_freeze::wake(&id)?;
        deep_freeze::wake(&id)?;
        std::thread::sleep(Duration::from_millis(250));
        let after = app_windows::inventory()?;
        if app_windows::identity(pid).as_ref() != Some(&expected) || before.len() != after.len() {
            return Err("Candidate changed identity/windows".into());
        }
        let table = app_windows::process_table()?;
        let descendants: Vec<_> = table.iter().filter(|(_, r)| r.0 == pid).collect();
        let evidence = serde_json::json!({"candidate":"SumatraPDF 3.6.1 x64 portable","scope":"production discovery/review/sleep/wake; private restricted-mode window","identity":expected,"before":before,"after":after,"children":descendants,"review":review,"frozen":frozen,"canceledReviewRejected":true,"unacknowledgedRejected":true,"changedWindowRejected":true,"expiredReviewRejected":true,"duplicateSelectionRejected":true});
        fs::write(
            root.join("results.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{evidence}\nEvidence: {}",
            root.join("results.json").display()
        );
        Ok(())
    })();
    deep_freeze::release_all();
    // Cleanup is confined to the still-exact harness child and its own windows.
    // This is not an application/product close or freeze fallback.
    if app_windows::identity(pid).as_ref() == Some(&expected) {
        unsafe extern "system" fn close(hwnd: HWND, p: LPARAM) -> windows::core::BOOL {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == p.0 as u32 {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            windows::core::BOOL(1)
        }
        unsafe {
            EnumWindows(Some(close), LPARAM(pid as isize)).map_err(|e| e.to_string())?;
        }
    }
    let end = Instant::now() + Duration::from_secs(10);
    while !hibernate_native::platform::process_ended(pid, &expected.process_started)?
        && Instant::now() < end
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    if !hibernate_native::platform::process_ended(pid, &expected.process_started)? {
        return Err("Candidate did not close cooperatively; no kill attempted".into());
    }
    let _ = child.wait();
    test
}
#[cfg(not(feature = "deep-freeze-lab"))]
fn main() {
    panic!("Candidate research requires deep-freeze-lab");
}
