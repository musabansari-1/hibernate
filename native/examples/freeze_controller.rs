//! Disposable crash-owner for native state-handle validation. Never packaged.
#[cfg(feature = "deep-freeze-lab")]
fn main() {
    use hibernate_native::{
        app_windows::ProcessIdentity,
        freeze_lab::journal::{self, Faults, Operation, Target},
    };
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    let root = PathBuf::from(std::env::args_os().nth(1).expect("fixture root"));
    assert!(root.join("fixture-authorized").is_file());
    let identity: ProcessIdentity =
        serde_json::from_slice(&std::fs::read(root.join("identity.json")).unwrap()).unwrap();
    // The only acceptable executable is the harness's adjacent private fixture copy.
    assert_eq!(
        std::fs::canonicalize(&identity.executable).unwrap(),
        std::fs::canonicalize(root.join("freeze_fixture.exe")).unwrap()
    );
    let c = journal::open(&root.join("freeze-journal.db")).unwrap();
    let parent = hibernate_native::app_windows::process_table()
        .unwrap()
        .get(&identity.pid)
        .and_then(|r| hibernate_native::app_windows::identity(r.0));
    let faults = Faults {
        exit_before_confirmation_marker: (std::env::args().nth(2).as_deref() == Some("gap"))
            .then(|| root.join("controller-gap")),
        ..Default::default()
    };
    let _operation = Operation::begin(&c, &[Target { identity, parent }], false, &faults).unwrap();
    std::fs::write(root.join("controller-frozen"), "owned state handle").unwrap();
    let start = Instant::now();
    while !root.join("controller-exit").exists() && start.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(20));
    }
    // Skips all Rust destructors. Windows closes this process's handles.
    std::process::exit(73);
}
#[cfg(not(feature = "deep-freeze-lab"))]
fn main() {
    panic!("Build this disposable experiment with --features deep-freeze-lab");
}
