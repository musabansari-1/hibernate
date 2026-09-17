//! Opt-in live test. Uses only fresh, idle instrumented fixture shells in an isolated DB.
use hibernate_native::{checkpoints, context, hibernation, platform, projects, store, terminal};
use serde_json::{json, Value};
use windows::Win32::{Foundation::CloseHandle, System::Threading::*};

struct Fixtures(Vec<Value>);
impl Drop for Fixtures {
    fn drop(&mut self) {
        // Test cleanup only: acquire the exact fixture shell handle, recheck creation identity,
        // then stop that idle test shell. Never target a Terminal host or any window.
        for v in &self.0 {
            let pid = v["pid"].as_u64().unwrap() as u32;
            unsafe {
                if let Ok(handle) = OpenProcess(
                    PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    false,
                    pid,
                ) {
                    if platform::process_identity(pid).as_ref() == Some(v) {
                        let _ = TerminateProcess(handle, 0);
                        let _ = WaitForSingleObject(handle, 5000);
                    }
                    let _ = CloseHandle(handle);
                }
            }
        }
    }
}
fn record(fixtures: &mut Fixtures, result: &Value) -> Value {
    assert_eq!(result["status"], "cwd-verified", "{result}");
    let identity = platform::process_identity(result["pid"].as_u64().unwrap() as u32).unwrap();
    assert!(identity["executable"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .ends_with("\\windowspowershell\\v1.0\\powershell.exe"));
    fixtures.0.push(identity.clone());
    identity
}
#[test]
#[ignore = "Creates isolated Windows Terminal fixture windows; run explicitly in an interactive Windows session"]
fn multiple_live_prompts_remain_context_only_and_restore_fresh() {
    let base = std::env::current_dir()
        .unwrap()
        .join(".validation/terminal-context")
        .join(uuid::Uuid::new_v4().to_string());
    let project_dir = base.join("project");
    let api = project_dir.join("api");
    std::fs::create_dir_all(&api).unwrap();
    std::env::set_var("HIBERNATE_DATA_DIR", base.join("data"));
    let c = store::open().unwrap();
    let project =
        projects::save(&c, None, "Terminal fixture", project_dir.to_str().unwrap()).unwrap();
    let other = projects::save(
        &c,
        None,
        "Separate fixture project",
        project_dir.to_str().unwrap(),
    )
    .unwrap();
    let project_id = project["id"].as_str().unwrap();
    let mut fixtures = Fixtures(vec![]);
    for folder in [&project_dir, &api] {
        record(
            &mut fixtures,
            &terminal::restore_for(folder, Some(project_id)).unwrap(),
        );
    }
    let unrelated = record(
        &mut fixtures,
        &terminal::restore_for(&project_dir, other["id"].as_str()).unwrap(),
    );
    assert_eq!(
        terminal::live_contexts_for_project(&c, project_id)
            .unwrap()
            .len(),
        2
    );
    let checkpoint = checkpoints::capture(project_id, vec![], true).unwrap();
    let points = checkpoints::list(&c, project_id).unwrap();
    let point = points.iter().find(|v| v["id"] == checkpoint).unwrap();
    let entries: Vec<Value> = point["body"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == "terminal")
        .cloned()
        .collect();
    assert_eq!(entries.len(), 2);
    let saved = store::snapshot(&c, entries[0]["snapshotId"].as_str().unwrap()).unwrap();
    for item in saved["body"]["terminal"]["items"].as_array().unwrap() {
        assert_eq!(item["commandReplay"], false);
        for authority in ["pid", "processStarted", "id", "sessionId", "handle"] {
            assert!(item.get(authority).is_none());
        }
    }
    let review = hibernation::prepare(&checkpoint).unwrap();
    for step in review["body"]["steps"].as_array().unwrap() {
        if step["kind"] == "terminal" {
            assert_eq!(step["eligible"], false);
        }
    }
    assert!(hibernation::close(
        review["id"].as_str().unwrap(),
        vec![entries[0]["id"].as_str().unwrap().into()],
        true
    )
    .is_err());
    assert_eq!(
        platform::process_identity(unrelated["pid"].as_u64().unwrap() as u32),
        Some(unrelated)
    );
    let before: i64 = c
        .query_row("SELECT COUNT(*) FROM terminal_sessions", [], |r| r.get(0))
        .unwrap();
    drop(c);
    checkpoints::recover().unwrap();
    let reopened = store::open().unwrap();
    let after: i64 = reopened
        .query_row("SELECT COUNT(*) FROM terminal_sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after, "Recovery must not launch another shell");
    let original = fixtures.0.clone();
    for entry in &entries {
        let result = context::restore_context_item(
            entry["snapshotId"].as_str().unwrap(),
            "terminal",
            entry["index"].as_u64().unwrap() as usize,
        )
        .unwrap();
        let identity = record(&mut fixtures, &result);
        assert!(!original.contains(&identity));
        let index = entry["index"].as_u64().unwrap() as usize;
        assert_eq!(
            result["cwd"],
            saved["body"]["terminal"]["items"][index]["cwd"]
        );
    }
    std::fs::write(base.join("results.json"), json!({"checks":["two distinct live project cwd contexts","other project excluded despite same cwd","runtime authority absent from snapshot","automatic terminal close refused before side effects","other fixture shell retained","reopen/recovery created no shells","two fresh restored shell identities at saved cwds"],"decision":"Context Restore only; cleanup is test-only, not product close verification"}).to_string()).unwrap();
    println!(
        "Terminal context evidence: {}",
        base.join("results.json").display()
    );
}
