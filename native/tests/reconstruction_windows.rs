//! Opt-in live Windows test. Every discovered executable is a private fixture copy.
use hibernate_native::{app_context, app_windows, checkpoints, hibernation, projects, store};
use serde_json::{json, Value};
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread::sleep,
    time::{Duration, Instant},
};

struct Fixture {
    path: PathBuf,
    child: Child,
}
impl Fixture {
    fn start(root: &Path, name: &str, scenario: Value, args: &[&str], cwd: &Path) -> Self {
        let path = root.join(format!("{name}.exe"));
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/examples/desktop_fixture.exe"),
            &path,
        )
        .unwrap();
        fs::write(path.with_extension("scenario.json"), scenario.to_string()).unwrap();
        let child = Command::new(&path)
            .args(args)
            .current_dir(cwd)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        Self { path, child }
    }
    fn stop(&mut self) {
        fs::write(self.path.with_extension("stop"), "fixture cleanup").unwrap();
        let end = Instant::now() + Duration::from_secs(15);
        while self.child.try_wait().unwrap().is_none() && Instant::now() < end {
            sleep(Duration::from_millis(50));
        }
        assert!(
            self.child.try_wait().unwrap().is_some(),
            "Fixture must stop cooperatively"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::write(self.path.with_extension("stop"), "fixture cleanup");
        let _ = self.child.wait();
    }
}
fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = f() {
            return value;
        }
        assert!(Instant::now() < end, "fixture condition timed out");
        sleep(Duration::from_millis(100));
    }
}
fn rows(path: &Path) -> Vec<app_windows::Window> {
    app_windows::inventory()
        .unwrap()
        .into_iter()
        .filter(|w| {
            w.owner
                .as_ref()
                .is_some_and(|o| Path::new(&o.executable) == path)
        })
        .collect()
}
fn capture(project: &str, path: &Path) -> (String, String, Value) {
    let discovery = app_context::discover().unwrap();
    let id = discovery["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["window"]["owner"]["executable"].as_str() == path.to_str())
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let point = checkpoints::capture(
        project,
        vec![("windows-discovery".into(), id.into())],
        false,
    )
    .unwrap();
    let review = hibernation::prepare(&point).unwrap();
    assert_eq!(review["body"]["steps"][0]["eligible"], false);
    let sid = review["body"]["steps"][0]["snapshotId"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(hibernation::close(review["id"].as_str().unwrap(), vec![sid.clone()], true).is_err());
    let saved = store::snapshot(&store::open().unwrap(), &sid).unwrap();
    assert_eq!(saved["body"]["reconstruction"]["safeCloseAllowed"], false);
    assert_eq!(
        saved["body"]["reconstruction"]["documentContextRestorable"],
        false
    );
    (point, sid, saved)
}

#[test]
#[ignore = "Starts disposable Win32 GUI fixtures; build --example desktop_fixture first"]
fn isolated_reconstruction_evidence_and_restore_lifecycle() {
    // Match Tauri's per-monitor-aware host while the fixture remains DPI-unaware.
    // This changes only this disposable test process, never the user's display settings.
    unsafe {
        windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )
        .unwrap();
    }
    let manifest = env!("CARGO_MANIFEST_DIR").replace('/', "\\");
    let root = Path::new(&manifest)
        .parent()
        .unwrap()
        .join(".validation")
        .join("reconstruction")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("HIBERNATE_DATA_DIR", root.join("data"));
    std::env::set_var("HIBERNATE_TEST_DISCOVERY_EXECUTABLES", "[]");
    let project = projects::save(
        &store::open().unwrap(),
        None,
        "Reconstruction fixtures",
        root.to_str().unwrap(),
    )
    .unwrap();
    let project = project["id"].as_str().unwrap();
    let cwd = root.join("working directory");
    fs::create_dir(&cwd).unwrap();
    let document = cwd.join("saved document.txt");
    fs::write(&document, "fixture saved document").unwrap();
    let mut fixtures = vec![
        Fixture::start(&root, "Simple", json!({"single":true}), &[], &root),
        Fixture::start(
            &root,
            "Arguments",
            json!({"single":true}),
            &["--mode", "fixture value"],
            &root,
        ),
        Fixture::start(&root, "WorkingDirectory", json!({"single":true}), &[], &cwd),
        Fixture::start(&root, "MultipleWindows", json!({}), &[], &root),
        Fixture::start(
            &root,
            "SavedDocument",
            json!({"single":true}),
            &[document.to_str().unwrap()],
            &root,
        ),
        Fixture::start(
            &root,
            "UnsavedDocument",
            json!({"single":true,"unsaved":true}),
            &[],
            &root,
        ),
        Fixture::start(
            &root,
            "RestartRegistered",
            json!({"single":true,"restart":true}),
            &[],
            &root,
        ),
    ];
    std::env::set_var(
        "HIBERNATE_TEST_DISCOVERY_EXECUTABLES",
        serde_json::to_string(&fixtures.iter().map(|f| &f.path).collect::<Vec<_>>()).unwrap(),
    );
    let mut evidence = vec![];
    for fixture in &fixtures {
        println!(
            "Discovering fixture {} (PID {}, identity {:?})",
            fixture.path.display(),
            fixture.child.id(),
            app_windows::identity(fixture.child.id())
        );
        wait(|| (!rows(&fixture.path).is_empty()).then_some(()));
        let (point, sid, saved) = capture(project, &fixture.path);
        assert!(!fixture.path.with_extension("close-requested").exists());
        let result = app_context::restore(&sid).unwrap();
        assert_eq!(result["status"], "app-present-state-unverified");
        assert_eq!(result["verified"], false);
        evidence.push(json!({"fixture":fixture.path.file_stem().unwrap().to_string_lossy(),"checkpoint":point,"snapshot":sid,"assessment":saved["body"]["reconstruction"],"native":saved["body"]["window"]["evidence"]}));
    }
    assert!(rows(&fixtures[3].path)
        .iter()
        .all(|w| w.same_owner_windows >= 3));
    assert_eq!(evidence[6]["native"]["restart"]["status"], "registered");
    assert_eq!(evidence[6]["assessment"]["disposition"], "keep-open");

    // Same executable, independent process: distinguish it and do not launch a duplicate.
    let (point, sid, _) = capture(project, &fixtures[0].path);
    let mut second = Command::new(&fixtures[0].path)
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    wait(|| (rows(&fixtures[0].path).len() == 2).then_some(()));
    let identities: Vec<_> = rows(&fixtures[0].path)
        .into_iter()
        .map(|w| w.owner.unwrap())
        .collect();
    assert_ne!(identities[0], identities[1]);
    let review = hibernation::prepare(&point).unwrap();
    fixtures[0].stop();
    second.wait().unwrap();
    fs::remove_file(fixtures[0].path.with_extension("stop")).unwrap();
    let mut replacement = Command::new(&fixtures[0].path)
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    wait(|| (!rows(&fixtures[0].path).is_empty()).then_some(()));
    assert_eq!(
        app_context::restore(&sid).unwrap()["status"],
        "different-app-instance-present"
    );
    assert_eq!(review["body"]["steps"][0]["eligible"], false);
    fs::write(fixtures[0].path.with_extension("stop"), "stop replacement").unwrap();
    replacement.wait().unwrap();
    fs::remove_file(fixtures[0].path.with_extension("stop")).unwrap();

    // Make only the disposable app accept STARTUPINFO on its next launch.
    fs::write(
        fixtures[0].path.with_extension("scenario.json"),
        json!({"single":true,"startupGeometry":true}).to_string(),
    )
    .unwrap();
    let result = app_context::restore(&sid).unwrap();
    assert_eq!(
        result["status"], "launched-app-observed-state-unverified",
        "{result}"
    );
    assert_eq!(result["placement"]["observedMatch"], true, "{result}");
    assert_eq!(result["verified"], false);
    evidence.push(json!({"fixture":"geometry-and-replacement-process","result":result}));
    fs::write(
        fixtures[0].path.with_extension("stop"),
        "stop restored fixture",
    )
    .unwrap();
    wait(|| rows(&fixtures[0].path).is_empty().then_some(()));
    fs::remove_file(fixtures[0].path.with_extension("stop")).unwrap();
    assert!(app_context::restore(&sid)
        .unwrap_err()
        .contains("already attempted"));

    // Stale observed HWND and ended PID are not capture authority.
    let selected = app_context::discover().unwrap();
    let old = selected["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["window"]["owner"]["executable"].as_str() == fixtures[1].path.to_str())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    fs::write(
        fixtures[1].path.with_extension("recreate"),
        "recreate only fixture HWND",
    )
    .unwrap();
    wait(|| {
        fixtures[1]
            .path
            .with_extension("recreated")
            .exists()
            .then_some(())
    });
    assert!(fixtures[1].child.try_wait().unwrap().is_none());
    assert!(
        app_context::capture(project, &old).is_err(),
        "Destroyed HWND must not survive in a live process selection"
    );
    fixtures[1].stop();
    assert!(app_context::capture(project, &old).is_err());

    // Changed/moved executable must block restoration before any attempt is recorded.
    let (_, changed_sid, _) = capture(project, &fixtures[2].path);
    fixtures[2].stop();
    let moved = fixtures[2].path.with_extension("moved");
    fs::rename(&fixtures[2].path, &moved).unwrap();
    assert!(app_context::restore(&changed_sid).is_err());
    fs::copy(&moved, &fixtures[2].path).unwrap();
    assert!(app_context::restore(&changed_sid)
        .unwrap_err()
        .contains("Executable changed"));
    evidence.push(
        json!({"fixture":"stale-window-moved-image-identical-image-replacement","passed":true}),
    );
    for fixture in &fixtures {
        assert!(!fixture.path.with_extension("close-requested").exists());
    }
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("Evidence: {}", root.join("results.json").display());
}
