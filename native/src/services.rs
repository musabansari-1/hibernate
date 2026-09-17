//! Explicitly reviewed direct development-runtime launches. Never replay on startup.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    os::windows::process::CommandExt,
    path::PathBuf,
    time::{Duration, Instant},
};
use windows::Win32::System::Console::{
    AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
    ATTACH_PARENT_PROCESS, CTRL_BREAK_EVENT,
};
type R<T> = Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Launch {
    pub executable: String,
    pub arguments: Vec<String>,
    pub cwd: String,
}
pub(crate) fn validate(def: &Launch) -> R<()> {
    crate::context::local_folder(&def.cwd)?;
    let exe = PathBuf::from(&def.executable);
    if !exe.is_absolute()
        || !exe.is_file()
        || def.executable.starts_with("\\\\")
        || !matches!(
            exe.file_name()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("node.exe" | "python.exe" | "python3.exe")
        )
    {
        return Err("Choose an absolute local node.exe or python.exe runtime. Shells, package-manager wrappers and arbitrary applications are not supported yet.".into());
    }
    if def.arguments.is_empty()
        || def.arguments.len() > 64
        || def.arguments.iter().any(|s| s.contains('\0'))
        || def.arguments.iter().map(String::len).sum::<usize>() > 16000
    {
        return Err("Provide between 1 and 64 literal arguments, at most 16000 bytes, without NUL characters.".into());
    }
    Ok(())
}
fn get(c: &Connection, id: &str) -> R<Value> {
    let raw: String = c
        .query_row("SELECT body FROM services WHERE id=?", [id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    crate::protection::decode(&raw)
}
fn write(c: &Connection, id: &str, body: &Value) -> R<()> {
    c.execute(
        "UPDATE services SET body=? WHERE id=?",
        params![crate::protection::encode(body)?, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn may_start(body: &Value) -> R<()> {
    let run = &body["launchRun"];
    if run.is_null() || run["state"] == "failed-before-launch" {
        return Ok(());
    }
    let pid = run["identity"]["pid"]
        .as_u64()
        .and_then(|p| u32::try_from(p).ok());
    let started = run["identity"]["processStarted"].as_str();
    if let (Some(pid), Some(started)) = (pid, started) {
        if crate::platform::process_ended(pid, started)? {
            return Ok(());
        }
        return Err("The previously launched runtime is still running. Inspect its console; another copy will not be started.".into());
    }
    Err("An earlier launch has no confirmed process identity. Inspect the service manually; automatic retry is blocked.".into())
}
pub fn configure(id: &str, def: Launch) -> R<Value> {
    let _guard = crate::checkpoints::OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    validate(&def)?;
    let c = crate::store::open()?;
    let mut body = get(&c, id)?;
    may_start(&body)?;
    body["launch"] = serde_json::to_value(def).map_err(|e| e.to_string())?;
    write(&c, id, &body)?;
    Ok(body)
}
fn owned(observation: &Value, identity: &Value) -> bool {
    let listeners = observation["listenerOwnership"]["listeners"].as_array();
    observation["status"] == "healthy"
        && identity["processStarted"].is_string()
        && listeners.is_some_and(|items| items.len() == 1 && items[0]["identity"] == *identity)
}
fn live_owned(body: &Value) -> R<(Value, String)> {
    let run = &body["launchRun"];
    if run["state"] != "verified" || run["verified"] != true {
        return Err("Only a previously verified Project Hibernate service launch can be stopped automatically.".into());
    }
    let identity = run["identity"].clone();
    let pid = identity["pid"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("Saved service process ID is missing")?;
    let started = identity["processStarted"]
        .as_str()
        .ok_or("Saved service process identity is missing")?;
    if crate::platform::process_ended(pid, started)? {
        return Err("The saved service process has already exited.".into());
    }
    let url = body["url"]
        .as_str()
        .ok_or("Missing service endpoint")?
        .to_owned();
    let observation = crate::health::check(&url)?;
    if !owned(&observation, &identity) {
        return Err("The expected endpoint is not uniquely owned by the saved service process. Automatic shutdown is blocked.".into());
    }
    Ok((identity, url))
}
fn request_console_break_direct(pid: u32) -> R<()> {
    // Console attachment is process-wide on Windows. This function therefore runs only
    // inside the short-lived hibernate-host helper, never in the multithreaded desktop.
    unsafe {
        let _ = FreeConsole();
        AttachConsole(pid).map_err(|e| format!("Could not attach to the service console: {e}"))?;
        SetConsoleCtrlHandler(None, true).map_err(|e| {
            format!("Could not protect the signal helper from the console signal: {e}")
        })?;
        let signal = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, 0)
            .map_err(|e| format!("Could not request graceful service shutdown: {e}"));
        let _ = FreeConsole();
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        let _ = SetConsoleCtrlHandler(None, false);
        signal
    }
}

pub fn signal_pending_shutdown(id: &str, shutdown_id: &str) -> R<()> {
    let c = crate::store::open()?;
    let body = get(&c, id)?;
    let pending = &body["shutdownRun"];
    if pending["id"] != shutdown_id || pending["state"] != "signaling" {
        return Err("This service shutdown signal is stale or was not durably requested.".into());
    }
    let (identity, url) = live_owned(&body)?;
    if pending["identity"] != identity || pending["url"] != url {
        return Err("The pending shutdown no longer matches the verified service process.".into());
    }
    let pid = identity["pid"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("Saved service process ID is missing")?;
    request_console_break_direct(pid)
}

fn request_console_break(id: &str, shutdown_id: &str) -> R<()> {
    let helper = crate::terminal::bridge()?;
    let mut child = std::process::Command::new(helper)
        .args(["--service-break", id, shutdown_id])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("Could not start the graceful service-signal helper: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err("The graceful service-signal helper failed. The service was not force-terminated.".into())
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The graceful service-signal helper did not finish within 5 seconds. The service was not force-terminated.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn stop_unlocked(id: &str, acknowledged: bool) -> R<Value> {
    if !acknowledged {
        return Err(
            "Review and explicitly approve stopping this verified development service.".into(),
        );
    }
    let c = crate::store::open()?;
    let mut body = get(&c, id)?;
    let (identity, url) = live_owned(&body)?;
    let pid = identity["pid"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("Saved service process ID is missing")?;
    let started = identity["processStarted"]
        .as_str()
        .ok_or("Saved service process identity is missing")?
        .to_owned();
    body["shutdownRun"] = json!({"id":uuid::Uuid::new_v4().to_string(),"state":"signaling","requestedAt":crate::now(),"identity":identity,"url":url});
    let shutdown_id = body["shutdownRun"]["id"]
        .as_str()
        .ok_or("Shutdown intent ID is missing")?
        .to_owned();
    write(&c, id, &body)?; // Persist intent before signalling. It is never replayed automatically.
    if let Err(e) = request_console_break(id, &shutdown_id) {
        body["shutdownRun"]["state"] = json!("signal-failed");
        body["shutdownRun"]["detail"] = json!(e.clone());
        write(&c, id, &body)?;
        return Err(e);
    }
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let ended = crate::platform::process_ended(pid, &started)?;
        let observation = crate::health::check(&url)
            .unwrap_or_else(|e| json!({"status":"unverified","detail":e}));
        let listener_gone = observation["listenerOwnership"]["status"] == "unobserved"
            || observation["status"] == "missing";
        body["shutdownRun"]["checkedAt"] = json!(crate::now());
        body["shutdownRun"]["observation"] = observation;
        if ended && listener_gone {
            body["shutdownRun"]["state"] = json!("verified");
            body["shutdownRun"]["verified"] = json!(true);
            body["shutdownRun"]["detail"] = json!("The verified service process exited after the graceful console signal and the expected endpoint is no longer listening.");
            body["launchRun"]["state"] = json!("stopped");
            write(&c, id, &body)?;
            return Ok(body["shutdownRun"].clone());
        }
        if Instant::now() >= end {
            body["shutdownRun"]["state"] = json!("unverified");
            body["shutdownRun"]["verified"] = json!(false);
            body["shutdownRun"]["detail"] = json!("Graceful shutdown could not be verified within 20 seconds. No force termination was attempted; inspect the service before continuing.");
            write(&c, id, &body)?;
            return Ok(body["shutdownRun"].clone());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}
fn start_unlocked(id: &str, reviewed: Launch, acknowledged: bool, creation_flags: u32) -> R<Value> {
    if !acknowledged {
        return Err("Review and explicitly approve this command before starting it.".into());
    }
    validate(&reviewed)?;
    let c = crate::store::open()?;
    let mut body = get(&c, id)?;
    if body["launch"] != serde_json::to_value(&reviewed).map_err(|e| e.to_string())? {
        return Err("The command changed. Review the current saved command again.".into());
    }
    may_start(&body)?;
    let url = body["url"]
        .as_str()
        .ok_or("Missing service endpoint")?
        .to_owned();
    // Query both address families before launching, including wildcard listeners. An unavailable
    // listener table is not permission to start a competing server.
    for address in crate::health::endpoint(&url)?.1 {
        let observation = crate::listeners::observe(address);
        if observation["status"] != "unobserved" {
            return Err("The expected port is occupied or ownership cannot be read. No command was started.".into());
        }
    }
    body["launchRun"] = json!({"id":uuid::Uuid::new_v4().to_string(),"state":"launching","approvedAt":crate::now(),"definition":reviewed});
    write(&c, id, &body)?; // Persist intent before the side effect. An interrupted intent never replays.
    let child = std::process::Command::new(&reviewed.executable)
        .args(&reviewed.arguments)
        .current_dir(&reviewed.cwd)
        .creation_flags(creation_flags)
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            body["launchRun"]["state"] = json!("failed-before-launch");
            body["launchRun"]["detail"] = json!(e.to_string());
            write(&c, id, &body)?;
            return Err(format!("Runtime could not start: {e}"));
        }
    };
    let identity = crate::platform::process_identity(child.id());
    body["launchRun"]["identity"] = json!(identity);
    body["launchRun"]["state"] = json!("started-unverified");
    write(&c, id, &body)?;
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let observation = crate::health::check(&url)
            .unwrap_or_else(|e| json!({"status":"unverified","detail":e}));
        let live = child.try_wait().map_err(|e| e.to_string())?.is_none();
        let verified = live
            && identity.as_ref().is_some_and(|i| {
                owned(&observation, i)
                    && crate::platform::process_identity(child.id()).as_ref() == Some(i)
            });
        body["launchRun"]["observation"] = observation;
        body["launchRun"]["verified"] = json!(verified);
        body["launchRun"]["checkedAt"] = json!(crate::now());
        if verified || !live || Instant::now() >= end {
            body["launchRun"]["state"] = json!(if verified {
                "verified"
            } else if live {
                "started-unverified"
            } else {
                "exited"
            });
            body["launchRun"]["detail"] = json!(if verified {
                "The launched runtime owns the expected listener and the endpoint returned HTTP 2xx."
            } else {
                "Service startup was not verified. Inspect the service console. No retry or termination was performed."
            });
            write(&c, id, &body)?;
            return Ok(body["launchRun"].clone());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}
pub fn start(id: &str, reviewed: Launch, acknowledged: bool) -> R<Value> {
    let _guard = crate::checkpoints::OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    start_unlocked(id, reviewed, acknowledged, 0x00000010)
}
#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn start_headless_validation(id: &str, reviewed: Launch, acknowledged: bool) -> R<Value> {
    let _guard = crate::checkpoints::OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    start_unlocked(id, reviewed, acknowledged, 0)
}
pub fn stop(id: &str, acknowledged: bool) -> R<Value> {
    let _guard = crate::checkpoints::OPERATION
        .try_lock()
        .map_err(|_| "Another project operation is running")?;
    stop_unlocked(id, acknowledged)
}
fn same_checkpoint_service(current: &Value, saved: &Value) -> R<()> {
    if current["id"] != saved["id"]
        || current["projectId"] != saved["projectId"]
        || current["url"] != saved["url"]
        || current["launch"] != saved["launch"]
    {
        return Err("The service definition changed since this checkpoint. Capture a new checkpoint or review/start the service manually.".into());
    }
    Ok(())
}
pub(crate) fn validate_checkpoint_hibernate(services: &[Value]) -> R<()> {
    let c = crate::store::open()?;
    for saved in services {
        if saved["launch"].is_null() {
            return Err(format!("{} has no approved startup definition and cannot be included in automatic service hibernation.", saved["name"].as_str().unwrap_or("A service")));
        }
        let current = get(
            &c,
            saved["id"]
                .as_str()
                .ok_or("Checkpoint service ID missing")?,
        )?;
        same_checkpoint_service(&current, saved)?;
        let (identity, _) = live_owned(&current)?;
        if saved["launchRun"]["identity"] != identity {
            return Err("The running service differs from the process captured by this checkpoint. Capture again before hibernating it.".into());
        }
    }
    Ok(())
}
pub(crate) fn hibernate_checkpoint(services: &[Value]) -> R<Value> {
    validate_checkpoint_hibernate(services)?;
    let mut results = Vec::new();
    for saved in services {
        let id = saved["id"]
            .as_str()
            .ok_or("Checkpoint service ID missing")?;
        let result = stop_unlocked(id, true)?;
        if result["verified"] != true {
            return Err(format!(
                "Service {} did not shut down cleanly; later services were retained.",
                saved["name"].as_str().unwrap_or(id)
            ));
        }
        results.push(json!({"id":id,"name":saved["name"],"result":result}));
    }
    Ok(
        json!({"closed":true,"services":results,"detail":"All selected verified development services exited after graceful console signals. No force termination was used."}),
    )
}
pub(crate) fn restart_checkpoint(services: &[Value]) -> R<Value> {
    let c = crate::store::open()?;
    for saved in services {
        let current = get(
            &c,
            saved["id"]
                .as_str()
                .ok_or("Checkpoint service ID missing")?,
        )?;
        same_checkpoint_service(&current, saved)?;
        if saved["launch"].is_null() {
            return Err("A checkpoint service has no approved startup definition.".into());
        }
    }
    drop(c);
    let mut results = Vec::new();
    for saved in services {
        let id = saved["id"]
            .as_str()
            .ok_or("Checkpoint service ID missing")?;
        let launch: Launch = serde_json::from_value(saved["launch"].clone())
            .map_err(|e| format!("Saved service command is unreadable: {e}"))?;
        let result = start_unlocked(id, launch, true, 0x00000010)?;
        if result["verified"] != true {
            return Err(format!(
                "Service {} started but readiness could not be verified.",
                saved["name"].as_str().unwrap_or(id)
            ));
        }
        results.push(json!({"id":id,"name":saved["name"],"result":result}));
    }
    Ok(
        json!({"verified":true,"services":results,"detail":"Checkpoint service commands were explicitly selected for restore and each restarted runtime uniquely owns its healthy endpoint."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn healthy_foreign_listener_is_not_restoration() {
        let identity = json!({"pid":42,"processStarted":"100","executable":"node.exe"});
        let mut o =
            json!({"status":"healthy","listenerOwnership":{"listeners":[{"identity":identity}]}});
        assert!(owned(&o, &identity));
        o["listenerOwnership"]["listeners"][0]["identity"]["processStarted"] = json!("101");
        assert!(!owned(&o, &identity));
        assert!(!owned(&json!({"status":"healthy"}), &identity));
    }
    #[test]
    fn interrupted_unknown_launch_and_live_process_block_replay() {
        assert!(may_start(&json!({"launchRun":{"state":"launching"}})).is_err());
        let identity = crate::platform::process_identity(std::process::id()).unwrap();
        assert!(may_start(&json!({"launchRun":{"state":"verified","identity":identity}})).is_err());
        assert!(may_start(&json!({})).is_ok());
    }
}
