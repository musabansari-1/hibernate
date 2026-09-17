//! Isolated direct-runtime lifecycle fixture; never touches a user's service or database.
use hibernate_native::{health, projects, services, store};
use serde_json::json;
use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    time::Duration,
};

#[test]
fn approved_service_launch_verifies_ownership_and_survives_database_reopen() {
    let base =
        std::env::temp_dir().join(format!("hibernate-service-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    std::env::set_var("HIBERNATE_DATA_DIR", base.join("data"));
    let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("node.exe"))
        .find(|p| p.is_file())
        .expect("Node runtime required for service fixture");
    let script = base.join("fixture.cjs");
    std::fs::write(&script, "const http=require('node:http');const s=http.createServer((q,r)=>{r.end('fixture');if(q.url==='/stop')s.close(()=>process.exit(0));});setTimeout(()=>s.listen(Number(process.argv[2]),'127.0.0.1'),300);setTimeout(()=>process.exit(0),15000);").unwrap();
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = occupied.local_addr().unwrap().port();
    let c = store::open().unwrap();
    let project = projects::active(&c).unwrap();
    let service = health::save(
        project["id"].as_str().unwrap(),
        "Fixture",
        &format!("http://127.0.0.1:{port}/"),
    )
    .unwrap();
    let id = service["id"].as_str().unwrap();
    let definition = services::Launch {
        executable: node.to_string_lossy().into_owned(),
        arguments: vec![script.to_string_lossy().into_owned(), port.to_string()],
        cwd: base.to_string_lossy().into_owned(),
    };
    services::configure(id, definition.clone()).unwrap();
    assert!(
        services::start_headless_validation(id, definition.clone(), false)
            .unwrap_err()
            .contains("approve")
    );
    let mut changed = definition.clone();
    changed.arguments.push("unexpected".into());
    assert!(services::start_headless_validation(id, changed, true)
        .unwrap_err()
        .contains("changed"));
    assert!(
        services::start_headless_validation(id, definition.clone(), true)
            .unwrap_err()
            .contains("occupied")
    );
    drop(occupied);
    let run = services::start_headless_validation(id, definition.clone(), true).unwrap();
    assert_eq!(run["verified"], true, "{run}");
    assert_eq!(
        run["observation"]["listenerOwnership"]["listeners"][0]["identity"],
        run["identity"]
    );
    assert!(
        services::start_headless_validation(id, definition.clone(), true)
            .unwrap_err()
            .contains("still running")
    );
    drop(c);
    let reopened = store::open().unwrap();
    assert_eq!(
        health::list(&reopened, project["id"].as_str().unwrap()).unwrap()[0]["launchRun"],
        run
    );
    let mut stop = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stop.write_all(b"GET /stop HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    for _ in 0..50 {
        if hibernate_native::platform::process_ended(
            run["identity"]["pid"].as_u64().unwrap() as u32,
            run["identity"]["processStarted"].as_str().unwrap(),
        )
        .unwrap()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let restarted = services::start_headless_validation(id, definition, true).unwrap();
    assert_eq!(restarted["verified"], true, "{restarted}");
    assert_ne!(restarted["id"], run["id"]);
    let mut stop = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stop.write_all(b"GET /stop HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    std::fs::write(base.join("results.json"), json!({"initial":run,"restart":restarted,"checks":["explicit approval","stale definition refusal","occupied port refusal","owned HTTP listener","duplicate prevention","database reopen","approved restart"]}).to_string()).unwrap();
    println!(
        "Service lifecycle evidence: {}",
        base.join("results.json").display()
    );
}
