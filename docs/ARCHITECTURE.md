# Architecture

The automatic Windows app layer is specified in [Generic app architecture](GENERIC-APP-ARCHITECTURE.md). `adapters.rs` supplies capability descriptors; `app_windows.rs` supplies read-only OS observations (including a size-only restart query requiring VM_READ); `reconstruction.rs` separates identity, launch, geometry, document and close evidence; `app_context.rs` supplies expiring native selections, protected metadata capture and locally authorized, single-attempt executable relaunch. `app_launch.rs` binds new-process observations to a held process handle and supplies optional creation-time geometry hints. Checkpoints route the reserved native discovery client directly, without impersonating an extension. Generic results never grant reviewed close or semantic readiness. Chrome and VS Code version-1 snapshot bodies and their existing native close contracts are retained.

Schema 6 adds `generic_origins` and `generic_launch_attempts`. These local records are excluded from exports/imports; origins are deleted with their snapshot. Older schema-5 builds reject a schema-6 database. Roll back binaries using the Git checkpoint, and restore a pre-upgrade database backup rather than downgrading this journal in place. Isolated data directories have independent desktop leases and WebView storage, and suppress personal integration enrollment.

## Boundaries

```
React dashboard ── focused Tauri commands ── Rust store / Windows APIs
                                              │
                                            SQLite
                                              │
Chrome MV3 ── Chrome native messaging ── hibernate-host.exe
VS Code ───── framed local child stdio ── hibernate-host.exe
```

There is no HTTP command server, shell-execution endpoint or required network service. Browser web pages cannot call the desktop bridge. Native messaging messages use a bounded, little-endian length prefix and JSON; responses preserve a request ID. The host does not log snapshots to stdout.

The TypeScript adapter owns semantic capture, safety, reconstruction and verification. `packages/core/lifecycle.ts` owns the ordered close protocol. Rust owns Windows observations, durable database operations, fixed application launch functions and framed IPC. Native Windows details stay out of React components.

## Project context

Projects have persistent UUIDs, names and local paths. Snapshots embed capture-time project metadata, bound when a capture job is queued so a later project switch cannot misattribute the result. Explicit per-window memberships are durable. Browser memberships include a session UUID; HWNDs, PIDs and numeric Chrome window IDs are observations, not durable project identities. Chrome profile UUIDs live in profile-local extension storage; VS Code integration IDs live in workspace-scoped storage.

Existing VS Code integration IDs are retained. New IDs use a deterministic hash of the profile storage URI and workspace identity, so they can be reproduced before a workspace-state flush. Reviewed close separately binds to the captured VS Code window session. A competing window cannot claim the first window's pending work. Native host disconnect marks ordinary unfinished work interrupted and releases that session's live report. A committed native close observation remains pending for its bounded evidence check; disconnect alone never proves closure.

`packages/core/projects.ts` models local signals and association evidence. Paths are normalized conservatively and matched at directory boundaries. A matching cwd/folder is association evidence; localhost alone is not proof of repository ownership. Read-only Git inspection handles ordinary repositories and worktrees without executing Git hooks or processes. Browser suggestions omit query strings. Explicit assignment/capture selection remains authoritative. Port-to-process-to-repository ownership and activity-based RAM recommendations are not implemented.

`native/src/discovery.rs` groups up to 40 reported local folders by Git ancestor, using fresh integration reports, Shell folders and process-identity-checked instrumented terminal prompt observations. It performs no recursive scan or automatic assignment. Direct remote paths and remote Git pointers are refused. The desktop requests suggestions automatically every 30 seconds while visible. `project_activity` records foreground observations only when a reporting item matches an explicit client/window membership, including the Chrome session key. The timestamp is historical evidence, not proof of continuous monitoring or inactivity. Activity metadata (project ID, timestamp and reporting client) is not encrypted; semantic folder reports remain DPAPI protected.

## Snapshot and adapter schemas

SQLite envelope:

- `id`, `client`, `adapter`, capture time and operation state.
- Versioned semantic `body`, including captured project identity.
- Native `before`/`after` memory observations and latest operation result.
- Separate job journal for capture/hibernate/resume requests and outcomes.

Chrome body version 1: profile UUID, capture-time window ID, ordered tab URLs/titles/index/active/pinned/group fields, group metadata, window geometry/state and capture timestamp. Credentials, cookies, form contents and DOM are not collected. Zod rejects unsupported versions and shapes before browser restoration. Credential-bearing URL authorities and unsupported schemes are refused.

VS Code body version 1: workspace URI/folder URIs, editor groups and local text-editor URIs, dirty flags, active editor, available terminal cwd/shell metadata and explicit limitations. No document content, command history, environment or hot-exit backup files.

Context body version 1: project-associated Shell folder URLs and manually defined terminal context, with provenance. This is not a claim to have captured a running Windows Terminal session.

The SQLite schema is version 6, with explicit migrations for projects, account-protected payloads, terminal sessions, snapshot indexing, checkpoints, reconstruction runs, project-close journals, native close observations and generic capture/launch authority. Desktop and native host must be upgraded together. Older builds reject this database because they cannot interpret the current journals safely. Newer database/adapter versions are rejected. The connection uses WAL, `synchronous=FULL`, `secure_delete`, a busy timeout and parameterized SQL. An acknowledged snapshot commit is required before close. SQLite durability still depends on Windows/storage hardware honoring flushes. DPAPI protects payloads, not all database columns or historical backup copies.

## Operation protocol and recovery

1. Capture fresh semantic state.
2. Commit the snapshot and native baseline sample.
3. Check safety; retain blocked snapshots.
4. Commit close intent.
5. Revalidate the live target immediately before adapter close.
6. Request close and verify target-window disappearance.
7. Settle, observe actual native resource metrics and record the outcome.
8. On resume, reconstruct semantic structure and query the application again.

Jobs are transactionally claimed at most once. Queued requests expire after 30 seconds; running requests older than five minutes are marked interrupted, not replayed. Partial failures retain snapshots and visible diagnostics. The preview does not offer automatic retry of uncertain side effects or a multi-adapter atomic transaction. Mixed project operations can partially succeed, and no overall “ready” status is inferred.

`native/src/checkpoints.rs` groups app snapshots, captured repository context and saved service definitions into a project checkpoint. Capture intent and each result are persisted, including partial failures. Selective reconstruction creates a separate durable run with pending, running, observed, failed and skipped parts. The native coordinator waits for adapter jobs; React does not assert verification outcomes. Services are checked from the checkpoint's definitions, not mutable project settings. A per-session desktop lease prevents another desktop from marking live work interrupted; startup recovery records unfinished journals without replaying them. A finished run means processing completed, not that all work is ready.

Browser restoration records a receipt before creating the remaining tabs. An interrupted live restore is inspected rather than duplicated. Receipts are scoped to the browser session because numeric window IDs can be reused after restart. Lazy URLs refer to local snapshot/index tokens; the original URLs are held in extension storage. Those mappings survive worker/browser restart.

`native/src/hibernation.rs` coordinates a single-use project review, read-only adapter preflight of every selected window, and sequential reviewed closes. Every individual close revalidates its target again. A failed preflight dispatches no close; a failed or unverified close stops later windows without automatically reopening earlier ones. Unsupported contexts and capture failures remain visible. Review selection is project-bound, capture-age-limited and rechecked when claimed. Live capture selections also expire when a window disappears, its report goes stale or its browser session changes.

Project-close jobs are linked to their run in the same transaction that grants snapshot close authority and enqueues the job. Startup marks unfinished runs interrupted and cancels queued project closes, preventing delayed destructive replay. Already-dispatched work remains uncertain and requires inspection. A shared operation lock serializes project capture, reconstruction and closing. The journal keeps snapshot references rather than duplicate URL/editor payloads, and records per-window disappearance and native resource observations; `closed-selected` describes only verified selected windows, not the whole project or exclusive RAM savings. An unverified VS Code close still stops the sequence; verified native closure allows the next window to proceed.

Native VS Code observation is armed only for the exact running, reviewed close job, captured integration session and approved snapshot. The bridge finds the nearest Code.exe ancestor owning a supported Electron workbench window; launcher processes are not mistaken for GUI owners. Hidden workbench windows are included; multiple candidates block close before intent is committed. The current adapter recognizes the observed `Chrome_WidgetWin_1` class; changed classes fail closed.

`native_close_observations` stores a protected binding and a 30-second deadline in the same transaction as close intent. A background native worker, dashboard reads and the project coordinator reconcile this evidence from outside VS Code's process tree, because VS Code can terminate its child bridge on shutdown. Both native window disappearance and GUI-process exit must be observed before success. Process-list enumeration errors, unreadable identities, live processes and expired observation windows never count as success. Failed/interrupted jobs are not promoted; reconciliation performs no application launches or closes.

Windows [IsWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iswindow) warns that handles can be recycled. The observer therefore also checks owner PID and creation identity using [GetWindowThreadProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid), and independently confirms process exit from a complete Toolhelp snapshot. These are bounded observations of a supported local window, not a general VS Code window-identity API or proof of exclusive project RAM reclamation.

## Verification and accounting

Chrome reports exact tab/group counts using observed ordered URLs, active/pinned state and normalized group membership. It does not claim page contents loaded, login validity or exact geometry. VS Code checks workspace identity, saved text-editor presence and terminal name/cwd multisets; duplicate expectations cannot share one observation. Explorer checks folder presence. A new instrumented Windows Terminal shell reports cwd through a child host process, bound to its launch session and snapshot project. Only an exact cwd handshake produces `cwd-verified`; missing evidence remains `launched-unverified`. The helper does not capture or replay commands.

Loopback health checks accept only explicit HTTP localhost/127.0.0.1/[::1] URLs, without credentials, query strings or fragments. Connections, response size and time are bounded; redirects are not followed. A 2xx response is an endpoint observation, never a process-ownership claim. Results render as capability facts, without synthesized percentages.

`native/src/listeners.rs` samples the connected endpoint's IPv4 or IPv6 listener table using Microsoft's [GetExtendedTcpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable). Buffer growth is bounded, row layout/counts are checked, and matching includes address family, loopback/wildcard binding and network-order port. Process executable queries require matching creation times before and after the query. Listener evidence is timestamped separately from HTTP response: a listener can disappear or change and does not establish which repository owns the service, nor authorise termination. Lookup failure leaves ownership unavailable without inventing a PID.

VS Code can use the same instrumented PowerShell helper as Windows Terminal. Launch definitions are derived natively from a snapshot belonging to the requesting integration; unknown or non-local directories are refused. Prompt observations are integration-scoped and require the original process creation time as well as PID. The extension matches each distinct live terminal to its session and process ID before accepting cwd evidence. This avoids relying on unavailable shell integration in Windows PowerShell and prevents a reused PID from validating an older observation.

Global physical used = total physical − available physical. Process working sets describe resident pages, including shared pages that can be counted more than once across processes. Private commit is distinct from physical RAM. Chrome totals cover all readable Chrome processes; source-browser tree totals are available when the native host can identify its Chrome ancestor. That tree may include other windows/profiles and is not exclusive workspace memory. PIDs in samples are diagnostic only. Unreadable processes and negative deltas are retained.

No working-set trimming or suspension is used to produce favorable readings. Window closure, resource observation and semantic verification are separate facts.

## Explicit development-service startup

`native/src/services.rs` stores a user-entered direct Node/Python executable, literal argument array and local working directory inside the existing encrypted service record. Saving never executes the command. Starting requires explicit acknowledgement and an exact match to the reviewed saved definition. The project operation lock prevents overlapping desktop operations; occupied or unreadable expected listeners block startup. Package-manager wrappers and child-process ownership are outside this initial support boundary.

The launch intent is persisted before spawning. A crash with an unknown process identity blocks replay, and a still-live prior PID/creation identity blocks duplicate startup. Startup waits up to 20 seconds for HTTP health and a single matching listener identity, rechecking the launched runtime remains alive. Timeout records unverified state and never kills or retries the process. Definitions and observations survive database reopen; startup is never automatic after application restart. The process runs with the user's inherited environment, which is not captured into the snapshot. Graceful shutdown and checkpoint-driven service startup remain outstanding.
