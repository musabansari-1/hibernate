#![cfg(feature = "deep-freeze-lab")]
use hibernate_native::{
    app_windows::{self, ProcessIdentity},
    freeze_lab::Lease,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread::sleep,
    time::{Duration, Instant},
};
fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < end, "fixture timed out");
        sleep(Duration::from_millis(50));
    }
}
fn read(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
struct Fixture {
    root: PathBuf,
    child: Child,
}
impl Fixture {
    fn start(root: &Path, scenario: Value) -> Self {
        Self::start_with(root, scenario, None)
    }
    fn start_with(root: &Path, scenario: Value, shared_executable: Option<&Path>) -> Self {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("fixture-authorized"),
            "harness-created disposable target",
        )
        .unwrap();
        fs::write(root.join("scenario.json"), scenario.to_string()).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/examples");
        fs::copy(
            source.join("freeze_fixture.exe"),
            root.join("freeze_fixture.exe"),
        )
        .unwrap();
        let executable = shared_executable
            .map(Path::to_owned)
            .unwrap_or_else(|| root.join("freeze_fixture.exe"));
        let child = Command::new(executable)
            .arg(root)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        wait(|| read(&root.join("heartbeat.json")));
        Self {
            root: root.to_owned(),
            child,
        }
    }
    fn identity(&self) -> ProcessIdentity {
        serde_json::from_value(wait(|| read(&self.root.join("identity.json")))).unwrap()
    }
    fn heartbeat(&self) -> Value {
        wait(|| read(&self.root.join("heartbeat.json")))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::write(self.root.join("stop"), "cooperative cleanup");
        if self.root.join("child").is_dir() {
            let _ = fs::write(self.root.join("child/stop"), "cooperative child cleanup");
        }
        let end = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < end {
            sleep(Duration::from_millis(50));
        }
        // Never kill a target; its own timeout is a secondary cleanup bound.
    }
}

#[test]
#[ignore = "Disposable shared-job isolation and controller-exit validation"]
fn shared_job_does_not_expand_owned_freeze() {
    use windows::{
        core::{BOOL, PCWSTR},
        Win32::{
            Foundation::*,
            System::{JobObjects::*, Threading::*},
        },
    };
    struct Owned(HANDLE);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".validation/deep-freeze-job")
        .join(uuid::Uuid::new_v4().to_string());
    // No limits or kill-on-close. Only these private targets are assigned.
    let job = Owned(unsafe { CreateJobObjectW(None, PCWSTR::null()).unwrap() });
    let target = Fixture::start(&root.join("target"), json!({"threadChurn":true}));
    let peer = Fixture::start(&root.join("peer"), json!({}));
    for fixture in [&target, &peer] {
        let process = Owned(unsafe {
            OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                fixture.child.id(),
            )
            .unwrap()
        });
        unsafe {
            AssignProcessToJobObject(job.0, process.0).unwrap();
            let mut member = BOOL(0);
            IsProcessInJob(process.0, Some(job.0), &mut member).unwrap();
            assert!(member.as_bool());
        }
    }
    let identity = target.identity();
    let original = target.heartbeat();
    let mut lease = Lease::open_fixture(&identity).unwrap();
    lease.freeze().unwrap();
    lease.trim().unwrap();
    let frozen = target.heartbeat();
    let peer_before = peer.heartbeat();
    sleep(Duration::from_millis(400));
    assert_eq!(target.heartbeat(), frozen);
    assert!(peer.heartbeat()["count"].as_u64() > peer_before["count"].as_u64());
    lease.wake().unwrap();
    wait(|| (target.heartbeat()["count"].as_u64() > frozen["count"].as_u64()).then_some(()));
    assert_eq!(target.heartbeat()["secret"], original["secret"]);
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/examples/freeze_controller.exe");
    let mut owner = Command::new(source)
        .arg(&target.root)
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    wait(|| target.root.join("controller-frozen").exists().then_some(()));
    let frozen = target.heartbeat();
    let peer_before = peer.heartbeat();
    sleep(Duration::from_millis(300));
    assert_eq!(target.heartbeat(), frozen);
    assert!(peer.heartbeat()["count"].as_u64() > peer_before["count"].as_u64());
    fs::write(
        target.root.join("controller-exit"),
        "private controller teardown",
    )
    .unwrap();
    assert_eq!(owner.wait().unwrap().code(), Some(73));
    wait(|| (target.heartbeat()["count"].as_u64() > frozen["count"].as_u64()).then_some(()));
    assert_eq!(target.heartbeat()["secret"], original["secret"]);
    assert_eq!(app_windows::identity(identity.pid), Some(identity.clone()));
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({
        "identity":identity,"samePrivateJob":true,"peerContinuedDuringBothFreezes":true,
        "normalWakePreservedState":true,"controllerExitReleasedFreeze":true,
        "scope":"No-limit private nested job only; unknown external job limits and supervisors are not validated"
    })).unwrap()).unwrap();
    println!("Evidence: {}", root.join("results.json").display());
}

#[test]
#[ignore = "Disposable topology, active loopback, identity and overlapping-owner gate"]
fn topology_activity_and_ownership_boundaries() {
    use hibernate_native::freeze_assessment::{self, Evidence};
    use std::io::{Read, Write};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".validation/deep-freeze-boundaries")
        .join(uuid::Uuid::new_v4().to_string());
    let mut evidence = Vec::new();
    let parent = Fixture::start(&root.join("parent"), json!({"child":true}));
    let worker: ProcessIdentity =
        serde_json::from_value(wait(|| read(&parent.root.join("child/identity.json")))).unwrap();
    let table = app_windows::process_table().unwrap();
    assert_eq!(table[&worker.pid].0, parent.child.id());
    let blocked = freeze_assessment::assess(&Evidence {
        process_identity_proven: true,
        process_group_proven: false,
        ..Default::default()
    });
    assert!(!blocked.safe_to_freeze);
    evidence.push(json!({"case":"GUI with worker","parent":parent.identity(),"worker":worker,"assessment":blocked}));
    fs::write(parent.root.join("stop"), "orphan observation").unwrap();
    wait(|| {
        hibernate_native::platform::process_ended(
            parent.child.id(),
            &parent.identity().process_started,
        )
        .ok()
        .filter(|v| *v)
    });
    assert_eq!(app_windows::identity(worker.pid), Some(worker.clone()));
    evidence.push(json!({"case":"parent exits, worker survives","parentGone":true,"workerStillExact":true,"freezeAllowed":false}));

    let late = Fixture::start(&root.join("late"), json!({}));
    let before = app_windows::process_table().unwrap();
    assert!(!before.values().any(|(p, _)| *p == late.child.id()));
    fs::write(
        late.root.join("spawn-child"),
        "change topology after review",
    )
    .unwrap();
    let new_child: ProcessIdentity =
        serde_json::from_value(wait(|| read(&late.root.join("child/identity.json")))).unwrap();
    assert_eq!(
        app_windows::process_table().unwrap()[&new_child.pid].0,
        late.child.id()
    );
    evidence.push(
        json!({"case":"child created after review","groupChanged":true,"freezeAllowed":false}),
    );

    let first = Fixture::start(&root.join("first"), json!({}));
    let second = Fixture::start_with(
        &root.join("second"),
        json!({}),
        Some(Path::new(&first.identity().executable)),
    );
    assert_eq!(first.identity().executable, second.identity().executable);
    assert_ne!(first.identity().pid, second.identity().pid);
    let mut lease_a = Lease::open_fixture(&first.identity()).unwrap();
    let mut lease_b = Lease::open_fixture(&first.identity()).unwrap();
    lease_a.freeze().unwrap();
    lease_b.freeze().unwrap();
    let frozen = first.heartbeat();
    let untouched = second.heartbeat();
    lease_a.wake().unwrap();
    sleep(Duration::from_millis(250));
    assert_eq!(
        first.heartbeat(),
        frozen,
        "Wake must not release another state object's freeze"
    );
    assert!(second.heartbeat()["count"].as_u64() > untouched["count"].as_u64());
    lease_b.wake().unwrap();
    wait(|| (first.heartbeat()["count"].as_u64() > frozen["count"].as_u64()).then_some(()));
    evidence.push(json!({"case":"same executable independent processes and overlapping freeze owners","unselectedProcessRan":true,"oneWakeDidNotReleaseOtherOwner":true}));

    let shared = Fixture::start(&root.join("shared"), json!({"twoWindows":true}));
    std::env::set_var("HIBERNATE_DATA_DIR", root.join("discovery-data"));
    std::env::set_var(
        "HIBERNATE_TEST_DISCOVERY_EXECUTABLES",
        serde_json::to_string(&vec![shared.identity().executable]).unwrap(),
    );
    let windows = wait(|| app_windows::inventory().ok().filter(|w| w.len() == 2));
    assert!(windows
        .iter()
        .all(|w| w.owner_pid == shared.child.id() && w.same_owner_windows >= 2));
    let shared_assessment = freeze_assessment::assess(&Evidence {
        process_identity_proven: true,
        shared_with_unselected_windows: true,
        ..Default::default()
    });
    assert!(!shared_assessment.safe_to_freeze);
    evidence.push(json!({"case":"two windows one process","assessment":shared_assessment}));

    let network = Fixture::start(&root.join("network"), json!({"network":true}));
    let endpoint = fs::read_to_string(network.root.join("endpoint")).unwrap();
    let mut socket = std::net::TcpStream::connect(endpoint).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    socket.write_all(b"a").unwrap();
    let mut byte = [0];
    socket.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'a']);
    let network_assessment = freeze_assessment::assess(&Evidence {
        active_external_work_risk: Some(true),
        ..Default::default()
    });
    assert!(!network_assessment.safe_to_freeze);
    evidence.push(json!({"case":"active network fixture","loopbackExchange":true,"assessment":network_assessment}));

    let exiting = Fixture::start(&root.join("exiting"), json!({}));
    let old = exiting.identity();
    let mut retained = Lease::open_fixture(&old).unwrap();
    fs::write(exiting.root.join("stop"), "exit after review").unwrap();
    wait(|| {
        hibernate_native::platform::process_ended(old.pid, &old.process_started)
            .ok()
            .filter(|v| *v)
    });
    assert!(retained.freeze().is_err());
    assert!(Lease::open_fixture(&old).is_err());
    evidence.push(json!({"case":"exit between review and freeze","heldHandleRefused":true,"reopenRefused":true}));
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("Evidence: {}", root.join("results.json").display());
}
#[test]
#[ignore = "Disposable native freeze gate; build freeze_fixture and freeze_controller with deep-freeze-lab"]
fn phase_two_same_process_state_and_controller_exit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".validation/deep-freeze")
        .join(uuid::Uuid::new_v4().to_string());
    let fixture = Fixture::start(&root, json!({"threadChurn":true}));
    let identity = fixture.identity();
    assert_eq!(identity.pid, fixture.child.id());
    let mut changed = identity.clone();
    changed.process_started.push('0');
    assert!(
        Lease::open_fixture(&changed).is_err(),
        "PID reuse/stale identity must fail"
    );
    let mut lease = Lease::open_fixture(&identity).unwrap();
    let before = fixture.heartbeat();
    lease.freeze().unwrap();
    assert!(
        lease.freeze().is_err(),
        "Repeat Sleep cannot increment another count"
    );
    sleep(Duration::from_millis(150));
    let frozen = fixture.heartbeat();
    sleep(Duration::from_millis(500));
    assert_eq!(fixture.heartbeat(), frozen, "GUI heartbeat must stop");
    lease.wake().unwrap();
    lease.wake().unwrap();
    wait(|| (fixture.heartbeat()["count"].as_u64() > frozen["count"].as_u64()).then_some(()));
    let awake = fixture.heartbeat();
    assert_eq!(awake["secret"], before["secret"]);
    assert_eq!(awake["valid"], true);
    assert_eq!(app_windows::identity(identity.pid), Some(identity.clone()));
    drop(lease);
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/examples/freeze_controller.exe");
    let mut controller = Command::new(source)
        .arg(&root)
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    wait(|| root.join("controller-frozen").exists().then_some(()));
    let owned = fixture.heartbeat();
    sleep(Duration::from_millis(300));
    assert_eq!(fixture.heartbeat(), owned);
    fs::write(
        root.join("controller-exit"),
        "unexpected owner exit without destructors",
    )
    .unwrap();
    assert_eq!(controller.wait().unwrap().code(), Some(73));
    let journal =
        hibernate_native::freeze_lab::journal::open(&root.join("freeze-journal.db")).unwrap();
    let recovered = hibernate_native::freeze_lab::journal::recover(&journal).unwrap();
    assert_eq!(recovered[0]["state"], "owner-exited");
    assert_eq!(
        recovered[0]["body"]["recovery"]["targets"][0]["observation"],
        "same-process-present"
    );
    wait(|| (fixture.heartbeat()["count"].as_u64() > owned["count"].as_u64()).then_some(()));
    assert_eq!(fixture.heartbeat()["secret"], before["secret"]);
    assert_eq!(app_windows::identity(identity.pid), Some(identity.clone()));
    let evidence = json!({"phase":2,"identity":identity,"before":before,"frozen":frozen,"awake":awake,"controllerExitReleasedFreeze":true,"pidReuseSimulationRejected":true,"repeatedSleepRejected":true,"repeatedWakeIdempotent":true});
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("Evidence: {}", root.join("results.json").display());
}

#[test]
#[ignore = "Disposable crash between suspension and journal confirmation"]
fn phase_four_crash_in_journal_gap() {
    use hibernate_native::freeze_lab::journal;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".validation/deep-freeze-gap")
        .join(uuid::Uuid::new_v4().to_string());
    let fixture = Fixture::start(&root, json!({}));
    let original = fixture.heartbeat();
    let controller =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/examples/freeze_controller.exe");
    let result = Command::new(controller)
        .arg(&root)
        .arg("gap")
        .creation_flags(0x08000000)
        .status()
        .unwrap();
    assert_eq!(result.code(), Some(74));
    assert!(root.join("controller-gap").is_file());
    let c = journal::open(&root.join("freeze-journal.db")).unwrap();
    let before = journal::rows(&c).unwrap();
    assert_eq!(before[0]["state"], "intent");
    let recovered = journal::recover(&c).unwrap();
    assert_eq!(recovered[0]["state"], "owner-exited");
    let resumed = fixture.heartbeat();
    wait(|| (fixture.heartbeat()["count"].as_u64() > resumed["count"].as_u64()).then_some(()));
    assert_eq!(fixture.heartbeat()["secret"], original["secret"]);
    assert_eq!(
        app_windows::identity(fixture.child.id()),
        Some(fixture.identity())
    );
    fs::write(root.join("results.json"),serde_json::to_vec_pretty(&json!({"beforeRecovery":before,"afterRecovery":recovered,"volatileStatePreserved":true,"sameProcess":true})).unwrap()).unwrap();
    println!("Evidence: {}", root.join("results.json").display());
}

#[test]
#[ignore = "Disposable two-process partial-failure and durable journal gate"]
fn phase_four_journal_and_partial_failures() {
    use hibernate_native::freeze_lab::journal::{self, Faults, Operation, Target};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".validation/deep-freeze-journal")
        .join(uuid::Uuid::new_v4().to_string());
    let fixtures = [
        Fixture::start(&root.join("a"), json!({})),
        Fixture::start(&root.join("b"), json!({})),
    ];
    let targets: Vec<_> = fixtures
        .iter()
        .map(|f| Target {
            identity: f.identity(),
            parent: app_windows::identity(std::process::id()),
        })
        .collect();
    let c = journal::open(&root.join("journal.db")).unwrap();
    let assert_running = || {
        for f in &fixtures {
            let before = f.heartbeat();
            wait(|| (f.heartbeat()["count"].as_u64() > before["count"].as_u64()).then_some(()));
            assert_eq!(f.heartbeat()["secret"], before["secret"]);
            assert_eq!(f.heartbeat()["valid"], true);
        }
    };
    assert!(Operation::begin(
        &c,
        &targets,
        false,
        &Faults {
            freeze_at: Some(1),
            ..Default::default()
        }
    )
    .is_err());
    assert_running();
    assert!(Operation::begin(
        &c,
        &targets,
        false,
        &Faults {
            confirmation_write: true,
            ..Default::default()
        }
    )
    .is_err());
    assert_running();
    let mut operation = Operation::begin(&c, &targets, false, &Faults::default()).unwrap();
    assert_eq!(
        journal::rows(&c).unwrap().last().unwrap()["state"],
        "frozen"
    );
    assert!(Operation::begin(&c, &targets, false, &Faults::default()).is_err());
    assert_eq!(
        journal::recover(&c).unwrap().last().unwrap()["state"],
        "frozen",
        "Never recover a live owner's handles"
    );
    assert!(operation
        .wake(
            &c,
            &Faults {
                resume_at: Some(1),
                ..Default::default()
            }
        )
        .is_err());
    assert_running();
    operation.wake(&c, &Faults::default()).unwrap();
    assert_eq!(
        journal::rows(&c).unwrap().last().unwrap()["state"],
        "released-with-errors"
    );
    let evidence = journal::rows(&c).unwrap();
    drop(c);
    let c = journal::open(&root.join("journal.db")).unwrap();
    assert_eq!(journal::rows(&c).unwrap(), evidence);
    let raw: String = c
        .query_row("SELECT body FROM freezes LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert!(
        !raw.contains("executable"),
        "Protected process metadata must not be plaintext"
    );
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("Evidence: {}", root.join("results.json").display());
}

#[test]
#[ignore = "Allocates and touches 512 MiB in a disposable GUI fixture"]
fn phase_three_physical_memory_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".validation/deep-freeze-memory")
        .join(uuid::Uuid::new_v4().to_string());
    let fixture = Fixture::start(&root, json!({"megabytes":512}));
    let identity = fixture.identity();
    let mut lease = Lease::open_fixture(&identity).unwrap();
    assert!(lease.trim().is_err());
    let original = fixture.heartbeat();
    let before = lease.measure().unwrap();
    assert!(before.private_commit >= 512 * 1024 * 1024);
    lease.freeze().unwrap();
    lease.trim().unwrap();
    sleep(Duration::from_millis(200));
    let after = lease.measure().unwrap();
    let frozen_heartbeat = fixture.heartbeat();
    let mut timeline = vec![after.clone()];
    for delay in [1000, 2000] {
        sleep(Duration::from_millis(delay));
        timeline.push(lease.measure().unwrap());
        assert_eq!(fixture.heartbeat(), frozen_heartbeat);
    }
    // Physical reclaim is nondeterministic. The disposable touched fixture should
    // produce a measurable (>1 MiB) drop, not an exact percentage or system delta.
    assert!(
        before.working_set > after.working_set + 1024 * 1024,
        "No measurable fixture working-set reduction: {before:?} {after:?}"
    );
    assert!(
        after.private_commit >= 512 * 1024 * 1024,
        "Trim must not remove the allocation"
    );
    lease.wake().unwrap();
    wait(|| (fixture.heartbeat()["count"].as_u64() > original["count"].as_u64()).then_some(()));
    sleep(Duration::from_millis(200));
    let wake = lease.measure().unwrap();
    assert_eq!(fixture.heartbeat()["valid"], true);
    assert_eq!(fixture.heartbeat()["secret"], original["secret"]);
    assert_eq!(app_windows::identity(identity.pid), Some(identity.clone()));
    assert!(wake.page_fault_count > after.page_fault_count);
    let evidence = json!({"phase":3,"identity":identity,"before":before,"afterFreezeAndTrim":after,"whileFrozenTimeline":timeline,"afterWakeAndActivity":wake,"workingSetReduction":before.working_set as i64-after.working_set as i64,"systemAvailableChange":after.available_physical as i64-before.available_physical as i64,"sameProcess":true,"volatileStatePreserved":true});
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!(
        "{}\nEvidence: {}",
        evidence,
        root.join("results.json").display()
    );
}
