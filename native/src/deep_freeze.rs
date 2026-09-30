//! Desktop-only authority for reviewed, whole-process sleep. No caller may supply
//! PIDs, paths, a safety assessment, or journal data as authority.
use crate::{
    app_windows::{self, ProcessIdentity, Window},
    freeze_assessment::{self as policy, Evidence},
    freeze_engine::{
        self as engine,
        journal::{self, Faults, Operation, Target},
        Lease,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
type R<T> = Result<T, String>;
const TTL: u64 = 20_000;
// Pinned release candidate. Never a runtime, imported-data or frontend override.
// Distribution is gated on the native, desktop and packaged lifecycle evidence.
const CATALOG_RELEASED: bool = true;
// Exact official portable image, independently downloaded and Authenticode checked
// during validation. Updating this catalog requires fresh family validation.
const SUMATRA_SHA: &str = "719f689b34f47be8ca105ce8484948474dafde0e106bab599e4a89326070c3d0";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopWindow {
    handle: usize,
    thread: u32,
    class: String,
    visible: bool,
    title: String,
}
fn topology(pid: u32) -> R<Vec<TopWindow>> {
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::*,
        },
    };
    unsafe extern "system" fn collect(hwnd: HWND, param: LPARAM) -> BOOL {
        let (pid, rows) = &mut *(param.0 as *mut (u32, Vec<TopWindow>));
        let mut owner = 0;
        let tid = GetWindowThreadProcessId(hwnd, Some(&mut owner));
        if owner == *pid && tid != 0 {
            let mut class = [0u16; 256];
            let n = GetClassNameW(hwnd, &mut class).max(0) as usize;
            let mut title = [0u16; 2048];
            let len = GetWindowTextW(hwnd, &mut title).max(0) as usize;
            rows.push(TopWindow {
                handle: hwnd.0 as usize,
                thread: tid,
                class: String::from_utf16_lossy(&class[..n]),
                visible: IsWindowVisible(hwnd).as_bool(),
                title: String::from_utf16_lossy(&title[..len]),
            });
        }
        BOOL(1)
    }
    let mut data = (pid, Vec::<TopWindow>::new());
    unsafe {
        EnumWindows(Some(collect), LPARAM(&mut data as *mut _ as isize))
            .map_err(|e| e.to_string())?;
    }
    if data.1.len() > 256 {
        return Err("Too many shared windows".into());
    }
    data.1.sort_by_key(|w| w.handle);
    Ok(data.1)
}
#[derive(Clone)]
struct Selection {
    identity: ProcessIdentity,
    window: Window,
    at: Instant,
}
struct Prepared {
    identity: ProcessIdentity,
    parent: ProcessIdentity,
    windows: Vec<TopWindow>,
    lease: Lease,
    _file: File,
    label: String,
    in_job: bool,
}
struct Review {
    at: Instant,
    targets: Vec<Prepared>,
    view: Value,
}
#[derive(Default)]
struct Controller {
    initialized: bool,
    selections: HashMap<String, Selection>,
    reviews: HashMap<String, Review>,
    active: HashMap<String, Operation>,
}
impl Controller {
    fn expire_reviews(&mut self) {
        self.reviews
            .retain(|_, r| r.at.elapsed() < Duration::from_millis(TTL));
    }
}
static CONTROLLER: OnceLock<Mutex<Controller>> = OnceLock::new();
fn controller() -> R<std::sync::MutexGuard<'static, Controller>> {
    CONTROLLER
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| {
            "Sleep controller unavailable; exit ContextSleep to release its owned freezes".into()
        })
}
fn database() -> R<rusqlite::Connection> {
    let path = crate::store::path()?.with_file_name("deep-freeze.db");
    std::fs::create_dir_all(path.parent().ok_or("Missing data directory")?)
        .map_err(|e| e.to_string())?;
    journal::open(&path)
}
/// Called once by the desktop after it acquires the existing database instance lease.
pub fn initialize() -> R<()> {
    let mut ctl = controller()?;
    if !ctl.initialized {
        journal::recover(&database()?)?;
        // Cleanup must not depend on a visible WebView or dashboard polling.
        // This worker only drops unused reviews; it never touches active freezes.
        std::thread::Builder::new()
            .name("sleep-review-expiry".into())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_secs(1));
                if let Some(lock) = CONTROLLER.get() {
                    if let Ok(mut ctl) = lock.try_lock() {
                        ctl.expire_reviews();
                    }
                }
            })
            .map_err(|e| format!("Cannot start sleep review cleanup: {e}"))?;
        ctl.initialized = true;
    }
    Ok(())
}

fn parent_and_boundary(identity: &ProcessIdentity) -> R<ProcessIdentity> {
    let table = app_windows::process_table()?;
    let parent_pid = table.get(&identity.pid).ok_or("Process ended")?.0;
    let parent = app_windows::identity(parent_pid)
        .ok_or("Parent identity unavailable; orphaned or ambiguous group")?;
    if parent
        .process_started
        .parse::<u64>()
        .map_err(|e| e.to_string())?
        >= identity
            .process_started
            .parse::<u64>()
            .map_err(|e| e.to_string())?
    {
        return Err("Parent PID has been reused".into());
    }
    if table.values().any(|r| r.0 == identity.pid) {
        return Err("Apps with child processes need a separately validated group".into());
    }
    let mut ancestor = std::process::id();
    let mut seen = HashSet::new();
    while seen.insert(ancestor) {
        if ancestor == identity.pid {
            return Err("ContextSleep and its ancestors are excluded".into());
        }
        let Some(row) = table.get(&ancestor) else {
            break;
        };
        ancestor = row.0;
    }
    Ok(parent)
}
fn prepare(s: &Selection) -> R<Prepared> {
    if !CATALOG_RELEASED {
        return Err(
            "Deep Freeze is awaiting complete app-family validation; this app stays open".into(),
        );
    }
    engine::validated_os()?;
    if s.at.elapsed() >= Duration::from_millis(TTL) {
        return Err("App selection expired; refresh and select it again".into());
    }
    if s.window.class != "SUMATRA_PDF_FRAME" {
        return Err("This app has no validated Deep Freeze contract yet".into());
    }
    let (executable, file) = crate::app_context::executable(&s.identity.executable)?;
    if executable.sha256 != SUMATRA_SHA {
        return Err("This app version has no validated Deep Freeze contract yet".into());
    }
    // Catalog approval covers the ENTIRE process, including all helper windows.
    // It never expands to same-name peers, children, brokers or historic descendants.
    let parent = parent_and_boundary(&s.identity)?;
    if crate::listeners::active_tcp(s.identity.pid)? {
        return Err(
            "This app has active network connections or listeners; finish that work before sleep"
                .into(),
        );
    }
    let windows = topology(s.identity.pid)?;
    if !windows
        .iter()
        .any(|w| w.visible && w.class == "SUMATRA_PDF_FRAME")
    {
        return Err("Document window is no longer present".into());
    }
    let lease = Lease::open(&s.identity)?;
    let in_job = lease.security()?;
    Ok(Prepared {
        identity: s.identity.clone(),
        parent,
        windows,
        lease,
        _file: file,
        label: "SumatraPDF 3.6.1".into(),
        in_job,
    })
}
fn validate(p: &Prepared) -> R<()> {
    if p.lease.security()? != p.in_job {
        return Err("App job membership changed after review".into());
    }
    if crate::listeners::active_tcp(p.identity.pid)? {
        return Err("Network activity began after review; keep this app running".into());
    }
    if parent_and_boundary(&p.identity)? != p.parent || topology(p.identity.pid)? != p.windows {
        return Err(
            "App processes or windows changed after review; nothing will remain sleeping".into(),
        );
    }
    Ok(())
}
fn proven(in_job: bool) -> policy::FreezeAssessment {
    policy::assess(&Evidence {
        process_identity_proven: true,
        process_group_proven: true,
        elevated: Some(false),
        system_critical: Some(false),
        same_interactive_user: Some(true),
        protected_process: Some(false),
        packaged: Some(false),
        in_job: Some(in_job),
        job_independent_contract: true,
        shared_with_unselected_windows: false,
        active_external_work_risk: None,
        suspend_available: true,
        memory_trim_available: true,
        excluded_role: false,
        validated_non_security_app_contract: true,
    })
}

/// Memory rows are distinct process identities, never sums of duplicate windows.
/// The inventory only publishes expiring selection IDs; assessments are descriptive.
pub fn dashboard(apps: &Value) -> R<Value> {
    let mut ctl = controller()?;
    let now = crate::now();
    ctl.expire_reviews();
    let previous = std::mem::take(&mut ctl.selections);
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for item in apps["items"].as_array().into_iter().flatten() {
        let window: Window =
            serde_json::from_value(item["window"].clone()).map_err(|e| e.to_string())?;
        let Some(identity) = window.owner.clone() else {
            continue;
        };
        if !seen.insert((identity.pid, identity.process_started.clone())) {
            continue;
        }
        let id = previous
            .iter()
            .find(|(_, s)| s.identity == identity)
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let selection = Selection {
            identity: identity.clone(),
            window,
            at: Instant::now(),
        };
        let memory = engine::observe_memory(&identity).ok();
        let mut assessment = policy::assess(&Evidence {
            process_identity_proven: true,
            elevated: selection.window.elevated,
            ..Default::default()
        });
        if ctl.initialized && selection.window.class == "SUMATRA_PDF_FRAME" {
            match prepare(&selection) {
                Ok(prepared) => assessment = proven(prepared.in_job),
                Err(e) => assessment.blockers = vec![e],
            }
        }
        let strategy = policy::choose(
            false,
            &assessment,
            item["capabilities"]["contextCapture"] == true,
            false,
        );
        let label = identity
            .executable
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or("Windows app");
        rows.push(json!({"id":id,"label":label,"title":selection.window.title,"identity":identity,"memory":memory,"strategy":strategy,"assessment":assessment,"allWindows":selection.window.same_owner_windows,"enhancement":item["enhancement"]}));
        ctl.selections.insert(id, selection);
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r["memory"]["workingSet"].as_u64().unwrap_or(0)));
    let mut sleeping = if ctl.initialized {
        journal::rows(&database()?)?
    } else {
        vec![]
    };
    for row in &mut sleeping {
        row["canWake"] = json!(ctl.active.contains_key(row["id"].as_str().unwrap_or("")));
    }
    Ok(json!({"at":now,"apps":rows,"operations":sleeping,"error":null}))
}
pub fn review(ids: Vec<String>) -> R<Value> {
    let _operation = crate::checkpoints::OPERATION
        .lock()
        .map_err(|_| "Another operation failed")?;
    let mut ctl = controller()?;
    if !ctl.initialized {
        return Err("Only the desktop can own Deep Freeze".into());
    }
    if ids.is_empty() || ids.len() > 16 || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err("Choose 1 to 16 distinct apps".into());
    }
    ctl.reviews.clear();
    let targets = ids
        .iter()
        .map(|id| prepare(ctl.selections.get(id).ok_or("Unknown app selection")?))
        .collect::<R<Vec<_>>>()?;
    let id = uuid::Uuid::new_v4().to_string();
    let at = crate::now();
    let items=targets.iter().map(|p|Ok(json!({"label":p.label,"identity":p.identity,"windows":p.windows,"memory":p.lease.measure()?,"assessment":proven(p.in_job)}))).collect::<R<Vec<_>>>()?;
    let view = json!({"id":id,"expiresAt":at+TTL,"apps":items,"warning":"All windows of each selected app will pause and remain visible. Finish printing, file changes and external work first. Network, audio and device sessions can time out. Other software can still close the app or time out while it is paused. Keep Windows running: this does not survive an app exit or reboot. Committed memory stays allocated; physical RAM reclaimed varies."});
    ctl.reviews.insert(
        id,
        Review {
            at: Instant::now(),
            targets,
            view: view.clone(),
        },
    );
    Ok(view)
}
/// Discarding an opaque review drops only its unused native handles.
pub fn cancel_review(review_id: &str) -> R<()> {
    let mut ctl = controller()?;
    ctl.reviews.remove(review_id);
    Ok(())
}
pub fn sleep(review_id: &str, acknowledged: bool) -> R<String> {
    let _operation = crate::checkpoints::OPERATION
        .lock()
        .map_err(|_| "Another operation failed")?;
    let mut ctl = controller()?;
    let review = ctl
        .reviews
        .remove(review_id)
        .ok_or("Unknown or already used sleep review")?;
    if !acknowledged || review.at.elapsed() >= Duration::from_millis(TTL) {
        return Err("Sleep review was not acknowledged or has expired".into());
    }
    for p in &review.targets {
        validate(p)?;
    }
    let mut leases = Vec::new();
    let mut targets = Vec::new();
    let mut observations = Vec::new();
    let mut files = Vec::new();
    for p in review.targets {
        targets.push(Target {
            identity: p.identity.clone(),
            parent: Some(p.parent.clone()),
        });
        observations.push((p.identity, p.parent, p.windows));
        leases.push(p.lease);
        files.push(p._file);
    }
    let operation = Operation::begin_owned(
        &database()?,
        &targets,
        leases,
        true,
        &Faults::default(),
        || {
            for (identity, parent, windows) in &observations {
                if crate::listeners::active_tcp(identity.pid)? {
                    return Err(
                        "Network activity changed during review; releasing owned freezes".into(),
                    );
                }
                if parent_and_boundary(identity)? != *parent || topology(identity.pid)? != *windows
                {
                    return Err(
                        "App group changed during freeze; releasing every owned freeze".into(),
                    );
                }
            }
            Ok(())
        },
        review.view,
    )?;
    let id = operation.id.clone();
    ctl.active.insert(id.clone(), operation);
    drop(files);
    Ok(id)
}
pub fn wake(id: &str) -> R<()> {
    // Wake is urgent and only touches retained freeze handles. Do not queue it
    // behind an unrelated long-running checkpoint capture/restore.
    let mut ctl = controller()?;
    if !ctl.initialized {
        return Err("Only the desktop can release its owned freeze".into());
    }
    // Remove first: database open/write failures cannot retain owned suspension.
    if let Some(mut operation) = ctl.active.remove(id) {
        let c = database()?;
        operation.wake(&c, &Faults::default())?;
        return Ok(());
    }
    if journal::rows(&database()?)?.iter().any(|r| r["id"] == id) {
        return Ok(());
    }
    Err("Unknown sleep operation".into())
}
pub fn release_all() {
    if let Some(lock) = CONTROLLER.get() {
        if let Ok(mut ctl) = lock.try_lock() {
            ctl.reviews.clear();
            ctl.active.clear();
        }
    }
    // Quit must not wait on a stuck controller. Process exit closes its handles.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_reviews_are_removed_without_dashboard_polling() {
        let mut ctl = Controller::default();
        for (id, age) in [("expired", TTL + 1), ("fresh", 0)] {
            ctl.reviews.insert(
                id.into(),
                Review {
                    at: Instant::now() - Duration::from_millis(age),
                    targets: vec![],
                    view: json!({}),
                },
            );
        }
        ctl.expire_reviews();
        assert!(!ctl.reviews.contains_key("expired"));
        assert!(ctl.reviews.contains_key("fresh"));
    }
    #[test]
    fn forged_reviews_and_uninitialized_authority_are_rejected() {
        assert!(sleep("forged", true).is_err());
        assert!(review(vec!["forged".into()]).is_err());
        assert!(wake("forged").is_err());
    }
}
