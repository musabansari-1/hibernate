# Generic Windows reconstruction architecture

## Current extension — 22 September 2026

The pre-implementation plan is [RECONSTRUCTION-PLAN.md](RECONSTRUCTION-PLAN.md).
Rollback tag `contextsleep-before-reconstruction-20260922` includes the existing
uncommitted source at `cff461d648265d76c701f2b09f2c27b4d5bb2937` without moving HEAD.

`reconstruction.rs` now assesses each generic observation/capture with separate
process identity, exact window identity, recipe availability, geometry request,
document, unsaved-state, close, and scoped verification capabilities. The chooser,
saved snapshot and reviewed-close explanation expose the resulting disposition.
These reports are descriptive: imported or forged capability flags grant nothing.
The registry and native close coordinator still independently refuse generic close.
Adapters can consume these observations and supply their own missing semantic and
conditional-close contracts; existing adapters retain their execution paths.

The minimum generic restore level is **application launch**. The minimum Sleep
level is **proven reconstruction of the working state at risk, bound to the exact
live instance, with a conditional close contract**. No generic Windows observation
currently satisfies the latter. A simple visibly empty app is not proven stateless.
Consequently this extension adds **no new class of apps safe for generic Sleep**.

Native evidence now includes package identity, restart-registration presence/size/
flags, owner/parent HWND, show state and normal window placement. The restart probe
uses the documented `GetApplicationRestartSettings` size-only query. It requests
`PROCESS_VM_READ` for non-elevated processes but collects no argument string and
does not call ReadProcessMemory, inject code, invoke recovery callbacks or run a
Restart Manager shutdown. Registered restart is an activation candidate with
unproven working-state/cwd coverage; its presence or an unavailable query blocks
the bare-executable fallback. Packaged and unknown package identities also block it.

Executable recipes now bind volume serial/file index as well as path, locked
SHA-256 and size. Byte-identical file replacement is rejected. Legacy generic
snapshots remain readable but require fresh capture for relaunch because their OS
activation evidence is absent. Database schema remains 6; Chrome/VS Code schemas
are unchanged. Existing same-executable processes are reported separately when
the captured PID/creation identity is absent; they never verify the saved instance.

`app_launch.rs` uses explicit-path CreateProcessW, no shell, no arguments, no
inherited handles, executable-directory cwd, and the current environment with
ELECTRON_RUN_AS_NODE removed. The held process handle anchors launch verification.
For a normal, unowned, single top-level window whose nonnegative rectangle fits
the primary display, STARTUPINFO requests its captured screen position/size.
It never passes WINDOWPLACEMENT workspace coordinates as screen coordinates and
never sends SetWindowPlacement/SetWindowPos to a discovered HWND. The application
may ignore the hint. A matching class/rectangle/show state in the sole observed
window of the launched process is reported as `geometry-observed`, with overall
`verified=false`. Shared/hidden windows, maximized/minimized captures and off-primary
placements receive no geometry request. This does not reconstruct logical windows,
multi-window layout, documents, navigation, authentication, jobs or unsaved memory.

Geometry capture and process creation use a temporary thread-local scope matching
the target window's DPI awareness. Captures record DPI context, window DPI and
primary display size; changed/unknown coordinate spaces cannot verify geometry.
The scope always restores the worker thread's previous context and never changes
global DPI settings. This prevents Tauri's per-monitor-aware observation from
feeding physical coordinates to a DPI-unaware application as logical coordinates.
See [GetWindowRect's DPI virtualization](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect)
and [SetThreadDpiAwarenessContext](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddpiawarenesscontext).

Command line, cwd, file associations, App Paths, Start Menu identities, protocol
handlers, accessibility data and AUMIDs remain insufficient proof of working-state
coverage. An absolute document argument alone does not establish an app's opening
semantics or prove no unsaved edits. See the plan's linked Windows documentation.

## Earlier implementation record — 21 September 2026

Status: plan written before implementation, 21 September 2026; implemented incrementally. Validation evidence is recorded in `GENERIC-APP-VALIDATION.md`.

## Existing invariants and rollback

The inspected implementation uses schema 5, DPAPI semantic payloads, FULL-sync SQLite commits, a desktop lease and a shared operation lock. Checkpoints commit capture and close intent before dispatch. Chrome and VS Code own semantic safety; native VS Code observation requires exactly one supported workbench HWND (including hidden candidates), its disappearance and GUI-process exit. All selected parts preflight before any close; later failure stops the sequence. Imports never acquire close authority. Recovery never replays uncertain side effects. Explorer and terminals are context-only. Approved Node/Python service ownership remains separate.

Create a Git tag at the unchanged tracked HEAD before implementation. Existing untracked demo files and distribution archives remain outside the checkpoint. Code rollback does not downgrade a database; use a separate fixture database for validation and preserve a database backup when deploying an upgrade.

Created tag: `contextsleep-before-generic-apps-20260921`, commit `6aecacefdea22edc9b3e7e1c7059dcc841326da3`.

## Implemented boundaries

`native/src/adapters.rs` is the capability registry. `app_windows.rs` performs query-only inventory; `app_context.rs` owns expiring selections, strict version-1 native context, validated executable continuity and one-attempt launch journals. `checkpoints.rs`, `history.rs` and schema 6 integrate these with project selection, DPAPI, imports and existing readiness. `packages/core/app-discovery.ts` adapts observations for the normal chooser without inventing a semantic integration-to-HWND binding. Each shared-process window remains a distinct selectable observation; selecting an app group is an explicit user action.

Capture records window title/class/rectangle, HWND/thread/PID/creation identity, executable, diagnostic parent PID, total observed same-owner windows including hidden ones, optional window AppUserModelID and presence (never contents) of taskbar relaunch metadata. It records no documents, command line, environment, application memory, shell history or unsaved content. HWND observations are intentionally not lifetime identities.

Relaunch is available only when the captured window has a readable non-elevated owner and no app-specific relaunch metadata, and the executable passes the local fixed-drive, canonical path, GUI PE and exclusion policy. An unchanged SHA-256 digest is checked under a file handle denying write/delete sharing. Only native capture mints a protected local origin; neither an extension message nor an imported snapshot can do so. A durable origin-keyed launch attempt covers sibling snapshots of the same process and survives interruption. Observing an existing app does not activate it or launch another copy. All generic results keep semantic verification false.

This is generic metadata support for ordinary visible named desktop windows, including Electron windows; it is not generic safe hibernation. Brokered/packaged apps, inaccessible/elevated processes, app-specific command routes and system hosts remain discovery/context only. No generic close primitive, forced exit, working-set trim, new network server, elevation or process-memory inspection was added.

## Architecture

1. Add a native adapter registry with independent discovery, context capture, reviewed close, relaunch, semantic restore and verification capabilities. Capabilities describe available evidence, not a single misleading compatibility score. Existing semantic adapters keep their schemas, clients and exact close contracts.
2. Enumerate Windows top-level windows independently of the existing four-app name filter. Read HWND, owning thread/PID, process creation identity, executable path, class, visibility, placement and optional AppUserModelID. Count hidden same-owner windows as ambiguity evidence. Process ancestry and app grouping are observations, never per-window project ownership. Inaccessible windows remain discovery-only. No injection, debugger, UI scraping, process-memory capture or elevation.
3. Issue bounded, short-lived native discovery selections. Capture re-observes the selected window and rejects changed/stale identity. Store versioned `windows-generic` snapshots with explicit absent semantics and project identity using existing protected history/checkpoint machinery. Native-only local origin records cannot be minted by extensions or imported backups.
4. Unknown applications always retain automatic-close=false and semantic-restore=false. User confirmation cannot override missing saved-work or exact-window authority. Generic restore can observe an already-running application or launch a narrowly validated, locally captured desktop executable with no arguments. Launch requires a local origin, unchanged executable identity, no running matching application and a durable one-attempt journal. Packaged/brokered apps, known semantic adapters, shells, hosts, system/security surfaces and ambiguous launch routes retain context-only support. Never execute taskbar/restart command strings, recovered CLI arguments, scripts, URLs or document titles.
5. Generic verification reports only an observed application window/process. It never marks documents, unsaved buffers, tabs, authentication, jobs, layout or workspace readiness verified. Shared-process Electron windows remain separate discovery observations grouped by process identity; they do not gain close authority. Existing Chrome/VS Code reports progressively add semantic entries without guessing a report-to-HWND mapping.
6. Add automatically refreshed open-app cards to the normal workspace selection and capture flow. Users choose discovered windows directly; no generic per-app connection workflow. Native selections and semantic selections share checkpoint Save → Review → Sleep → Restore reporting. Retained generic contexts visibly prevent an overall verified-workspace claim. Existing Chrome permission prompts remain only for deeper browser access; VS Code enrollment remains its existing integration behavior.

## OS evidence and deliberate limits

- [IsWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iswindow): HWNDs are recyclable. PID + creation time protects against PID reuse, not HWND reuse within one long-lived process. Capture is a bounded observation, never a close lease.
- [AppUserModelID](https://learn.microsoft.com/en-us/windows/win32/shell/appids): taskbar grouping can differ per window in one process. It is identity metadata, not saved-state or workspace ownership evidence.
- [RelaunchCommand](https://learn.microsoft.com/en-us/windows/win32/properties/props-system-appusermodel-relaunchcommand): app-provided taskbar relaunch metadata can contain arbitrary commands. Record availability only; do not execute it.
- [GetApplicationRestartSettings](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getapplicationrestartsettings): registered restart information requires PROCESS_VM_READ and does not promise document recovery. Do not expand query-only access merely to collect it.
- [SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook): out-of-context events require a pumping observer thread; event loss/restart gaps cannot establish continuous generation identity. No shared-owner close upgrade until an exact integration binding and conditional safe-close contract exist.
- [ActivateApplication](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication): activation identifies an application, not a saved window/document. Defer generic packaged-app activation until launch identity and duplicate routing have a validated contract.

## Incremental delivery and validation

First implement read-only inventory and registry; then native snapshots and bounded relaunch journals; then checkpoint/import/readiness integration; then UI. Add unit regressions for PID reuse, stale selection, hidden/shared windows, denied identity, forged/imported authority, changed executables, launch retry and interrupted runs. Preserve original Chrome/VS Code snapshots and run existing TypeScript/Rust suites, lint and builds. Audit existing live harnesses for fixture isolation and stale UI selectors, run the complete runnable integration suite, then build a portable package and validate its actual Tauri commands with disposable Win32 multi-window fixtures plus the existing Chrome/VS Code/service packaged scenario. Do not reboot, terminate personal processes, install into personal profiles during tests, or operate existing user windows.
