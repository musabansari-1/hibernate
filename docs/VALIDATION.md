# Technical validation — updated 14 September 2026

## Terminal ownership review — 15 September

**55 Rust unit tests, 1 real service integration test, native Clippy, native host build, 23 TypeScript tests and frontend/extensions production build passed.** The terminal-specific production change rejects stale/mismatched launch-session evidence and oversized PIDs before declaring cwd restoration verified.

Automatic Windows Terminal closure was not implemented. The initial isolated named-window probe admitted an additional independent tab and exposed an incorrect console-HWND visibility assumption, so it still does not provide exclusive tab/window close authority. The opt-in multi-context Context Restore regression now **passes live** in a disposable Windows session: exact-session/live-process prompt verification succeeded for multiple saved cwd values, automatic close remained refused, recovery launched nothing, an unrelated fixture shell remained untouched, and fresh restored shells were verified at the saved cwd values with no command/history replay. The earlier failure was a Windows path-separator normalization bug, not missing process identity. No shared Terminal host was reset and no existing user terminal was targeted. Full ownership boundaries remain documented in [TERMINAL-OWNERSHIP.md](TERMINAL-OWNERSHIP.md). The packaged V1 results below remain unchanged historical evidence.

The current source build demonstrates reviewed Chrome/VS Code closure, durable project checkpoints, cold-start app reconstruction, explicit development-service commands, required-capability readiness, encrypted storage, repository suggestions and real terminal-directory verification. The **packaged complete-project lifecycle now passes across a ContextSleep process restart**. **The remaining V1 release gate is an actual Windows reboot using that same prepared scenario.** Broader application compatibility, exact UI/layout restoration and production signing remain outside this V1 proof.

## Latest executed checks — 17 September

The final packaged V1 developer-project lifecycle **passed end to end** with VS Code, Chrome and one approved Node service in a single checkpoint. The packaged desktop preflighted all three, closed VS Code with native GUI-process exit verification, closed the selected Chrome window, gracefully stopped the exact verified service process, then survived a ContextSleep process restart without replaying any app or service launch. Explicit restore approval then cold-launched the enrolled Chrome profile and saved VS Code workspace, restarted the approved service as a new verified process, verified all three recorded capabilities and reported overall project readiness as `ready`; that readiness evidence also persisted across dashboard reload. Evidence: `.validation/packaged-v1/results.json`. Current validated package: `dist/ProjectHibernate-0.1.0-2026-09-17T08-51-37-076Z`.

Chrome cold-start checkpoint restore **passed end to end** in an isolated persistent Chrome profile. The user-facing profile binding stored the explicitly supplied Chrome executable/profile directory/user-data root, the whole profile disconnect was marked offline immediately, restore relaunched that exact profile without inspecting Chrome's private profile data, the original extension identity reconnected, saved browser context reconstructed and verified, and checkpoint readiness became `ready`. Evidence: `.validation/chrome-cold-restore/results.json`.

VS Code cold-start checkpoint restore **passed end to end** in real VS Code after a reviewed native close. The product verified native GUI-process exit, restore launched the saved local workspace without the test manually reopening VS Code, the original integration identity reconnected in a new VS Code session, **2/2 saved text editors** were verified, and checkpoint readiness became `ready`. Evidence: `.validation/vscode-cold-restore/results.json`. The isolated validation hook only supplies test profile/extension paths when `HIBERNATE_DATA_DIR` is set; normal product launch still uses an installed stable VS Code and its installed ContextSleep extension.

Required-capability readiness is now explicit: a reconstruction run can finish while the project remains `incomplete`. Every recorded restorable checkpoint capability must provide its adapter-specific observed verification evidence; skipped parts, failed captures and unverified results prevent a `ready` result. Unit coverage includes skipped capabilities and failed captures.

The owned-service lifecycle now supports explicitly reviewed direct Node/Python definitions, duplicate/occupied-port refusal, durable launch identity, process-owned HTTP verification, graceful console-break shutdown only for the exact previously verified process, and checkpoint-driven restart only after explicit approval of the saved command. No force-kill fallback is used. The real headless startup/ownership integration passes, and packaged graceful stop/restart passed both in isolation and inside the complete packaged V1 lifecycle.

Current source checks: **57 Rust unit tests + 1 real service integration test, 25 TypeScript tests, TypeScript typecheck, native Clippy, frontend/extension build and Tauri release desktop build passed**. Native coverage verifies project-scoped live instrumented-terminal discovery, multiple saved terminal working-directory entries in one context snapshot, and exact terminal restore identity including mixed Windows path separators. The opt-in disposable `terminal_context` regression also passes live for two project cwd values plus an unrelated fixture context, while automatic Terminal close remains ineligible. Native backup-import coverage verifies remapped snapshot/service identities, restore-only imported app captures, stripped service process/launch-run authority, no imported jobs/reviews/reconstruction runs, and that a different existing Chrome cold-start binding is never overwritten. The current 17 September package passed the combined packaged VS Code + Chrome + service lifecycle across a ContextSleep process restart and returned overall readiness to `ready`.

Post-V1 UX work now includes a first-run setup checklist derived from live dashboard state rather than a dismiss-only tutorial, plus tray actions that reuse the existing safe active-project Explorer and fresh instrumented-terminal launch paths. Local-context capture now uses live process-identity-verified instrumented terminal prompt working directories for the selected project when available (up to 20), stores only reconstructable cwd/shell metadata rather than runtime PID/session authority, and falls back to one project-root terminal definition when no instrumented prompt is live. Restores still open fresh no-profile PowerShell prompts and never replay commands, history, panes or process state. Checkpoint backups now bundle the checkpoint and its referenced snapshots under the current Windows account's DPAPI protection. Import uses new snapshot/checkpoint/service IDs, never imports pending commands, close authority, runtime PIDs or previous readiness evidence, and only preserves a static Node/Python launch definition if the local validator still accepts it. A referenced Chrome cold-start binding is imported only when it validates and does not conflict with an existing binding; otherwise the existing configuration is retained and the imported checkpoint records a warning. The portable package now also includes an opt-in `SETUP.ps1` helper; running it with no switches makes zero machine changes. Private-beta diagnostics can be exported as an aggregate-only JSON support report: native coverage seeds projects, reports, services, snapshots and jobs with sentinel secret strings and proves the report omits project paths, browser URLs/titles, terminal working directories/history, service URLs/commands, snapshot payloads and raw process IDs. The real Tauri/WebView2 desktop smoke also exported this report through the UI from an isolated data directory and verified that neither the saved project name nor its local path appeared in the file.

Native close verification passed in real VS Code 1.137.0: the product observed the uniquely bound native window disappear and its GUI process exit, retained real memory observations, and then reconstructed and verified 2/2 saved editors after reopening. Dirty work, tasks, debuggers and existing terminals remain protected by the adapter's safety checks. The durable schema-5 observer survives integration/bridge exit; tests cover exact review/job/session authority, multiple-window refusal, PID identity, disconnect plus database reopen, expiry without false success, and preservation of failed/interrupted results.

The disposable VS Code multi-window ownership regression also passes live. Two separate extension hosts and Hibernate bridges were observed converging on one GUI Code.exe owner with multiple supported `Chrome_WidgetWin_1` HWND candidates. The reviewed close was refused as ambiguous before a native-close observation was committed, both fixture integrations remained responsive, and dirty work still blocked preflight. After the second disposable window closed itself, the unchanged exclusive-window path successfully verified the remaining exact HWND disappearance plus GUI-process exit. The ordinary installed extension cannot use VS Code's proposal-gated `nativeWindowHandle` API, so same-process multi-window automatic close remains intentionally unsupported; see `VSCODE-MULTIWINDOW-OWNERSHIP.md`.

## Executed checks — 13 September

| Scenario | Observed result | Boundary |
| --- | --- | --- |
| Source checks | 23 TypeScript tests, 39 Rust tests, TypeScript build and native Clippy passed | Includes project-close review scope/single-use, minimal journal payloads, queued-close recovery, schema 3→4 migration, stale selections and reused Chrome window IDs, plus earlier discovery/identity checks |
| Project-wide reviewed Hibernate | Changed second window blocked all closes at preflight; a change after preflight stopped later closes after one verified closure; a fresh review closed both remaining selected windows; reconstruction verified 4/4 tabs and 1/1 group; observations survived UI reload | Isolated Chrome HTTP fixtures, including a grouped tab; unsupported terminal context retained; system memory samples are not exclusive project RAM; no Windows reboot |
| Native listener observations | Real IPv4 and IPv6 sockets matched their test process PID, executable and creation identity; malformed table bounds and unrelated interface/port matches refused; HTTP checks retained listener observations | Listening socket ownership is a time-bound observation, not repository ownership or authority to terminate a process |
| Desktop discovery and activity | Real Tauri UI grouped two fixture folders under one repository; acceptance created one project; explicit assignment enabled activity; reload retained membership and timestamp; background reports did not advance it; disconnected reports stopped producing suggestions | Folder/focus inputs are isolated protocol fixtures, not new OS foreground evidence; no RAM estimate or guaranteed inactivity claim |
| Real VS Code regression | Read-only preflight accepted clean saved editors and refused new dirty work without changing snapshot authority; saved editor reconstruction and 2/2 instrumented terminal cwd checks passed; clean close/reopen regression passed | Existing arbitrary shells, prior commands and exact layout remain outside verified scope; product native-window disappearance remains unverified |
| Chrome Quick Sleep regression | Updated reporting path retained 2/2 eligible background tab discard verification and wake on selection | Isolated browser profile; no new project RAM claim |
| Packaged project reconstruction | Bundled desktop/host passed selective 3/3 Chrome reconstruction, captured-endpoint health and exact fixture listener PID/executable checks; listener evidence survived desktop restart unchanged; second-instance protection passed | Executable comparison resolves NVM junction aliases; listener ownership remains separate from project ownership; no OS reboot |

Discovery evidence: `.validation/project-discovery/results.json`. Its screenshot is private. The following September 12 checks remain earlier evidence for the existing close/restore paths, not claims of a full Windows reboot test.

Project-close evidence: `.validation/project-hibernate/results.json` records per-window reconstruction facts and before/after system memory. The scenario exposed a stale UI selection after a window closed; selections now prune missing/offline windows and changed browser-session identities. Review rendering also distinguishes Chrome tab groups from VS Code editor groups. The grouped-tab test fixture initially moved its tab to another window; explicitly supplying the fixture destination corrected setup without weakening product assertions.

The final project-close regression also passed with the packaged desktop and native host in `ProjectHibernate-0.1.0-2026-09-13T14-01-41-357Z`. The workspace-owned test manifest temporarily routed Chrome to that packaged host and was restored afterward; tested extension files matched the package. All 18 distribution hashes were verified. This package includes the architecture notes and schema 4 upgrade instructions.

The latest packaged checkpoint evidence is `.validation/project-checkpoint/results.json`. Its first run exposed a test comparison error: Windows returned the physical Node executable path while `process.execPath` used the NVM junction. Comparing both resolved paths preserved the exact PID and creation-identity requirements and the regression then passed. All 17 packaged file hashes were checked.

## Executed checks — 12 September

| Scenario | Observed result | Boundary |
| --- | --- | --- |
| TypeScript tests | 21 passed | Schema, safety, lifecycle, association, multiset verification and JSON-order-independent editor comparison |
| Native Rust tests | 28 passed | Includes DPAPI, project binding, history/import safety, journal recovery, strict HTTP status parsing, process-identity-scoped terminal observations and competing VS Code window protection |
| Real reviewed Chrome closure | Ordinary local HTTP project window closed; restored URLs verified; canceled beforeunload preserved unsaved input; changed tab state and consumed approval refused | Explicit user-review path in isolated Chrome; no arbitrary website-state guarantee |
| Quick Sleep | 2/2 eligible background tabs discarded without closing their window; selecting a tab woke it | Worker-only test attachment; no reboot or exclusive resource-reclamation claim |
| Full Chrome regression | 50 tabs captured, safety checked, closed, measured; browser/native-host restart; 50/50 organization verified; 49 lazy placeholders | Synthetic owned pages; OS reboot not performed |
| Real desktop project checkpoint | Chrome and local context saved together; project switching preserved scoped history/counts; selective restore verified 3/3 tabs; terminal stayed explicitly skipped; only capture-time service endpoint checked | Current service settings intentionally differed from checkpoint expectations |
| Desktop recovery | Completed checkpoint results survived desktop process restart; no browser launches replayed; second desktop instance exited without taking over the live journal | Native interrupted-journal behavior also covered by Rust tests |
| Real VS Code 1.137.0 | Missing saved editor restored; dirty work preserved; two same-name/same-cwd terminals reconstructed as distinct shells with 2/2 cwd observations | Instrumented no-profile shells; prior commands/history and full layout not restored; reviewed close completion not verified |
| Reviewed VS Code close/reopen | Clean saved-file window closed through the product; test harness observed isolated process exit; native disconnect retained the snapshot; reopening reproduced integration identity and verified 2/2 editors | Test harness observes process exit; the product still does not equate a disconnected extension with general native-window closure or full process hibernation |
| Real Windows Terminal | Expected local cwd observed from a new PowerShell prompt in the correct isolated database | New shell only; existing terminal work untouched |
| Runnable Windows distribution | Release desktop/native host, Chrome extension and VSIX packaged with instructions and file hashes; packaged desktop passed real WebView2/native IPC/save/reload smoke test | Unsigned folder distribution; no installer, automatic update or Windows reboot proof |

Latest local evidence: `.validation/project-checkpoint/results.json`, `.validation/reviewed-browser/results.json`, `.validation/quick-sleep/results.json`, `.validation/terminal/results.json`, `.validation/browser/results.json`, `.validation/vscode/results.json`, `.validation/vscode-close/results.json`, `.validation/desktop/results.json`. UI screenshots are private and may contain local paths/window titles.

The 50-tab test initially timed out while creating fixture tabs. Its synchronous native polling blocked the automation event loop; changing that polling to asynchronous child execution allowed the full regression to pass. This was a test-harness change, not a relaxed adapter success condition. VS Code testing exposed that the installed Windows PowerShell shell supplied no built-in shell-integration cwd; the product now uses its own process-bound prompt handshake for created terminals instead of assuming their directories.

The reviewed VS Code test exposed two further defects: native JSON key reordering made unchanged editor groups fail a serialization comparison, and test-mode workspace storage did not preserve a random integration ID across restart. Editor groups now compare explicit semantic fields, and new integration IDs can be reproduced from profile/workspace identity while retaining previously enrolled IDs. The test records caught assertion failures separately because intentionally closing a window can end a pending VS Code test runner with exit code 1; native close intent, integration disconnect, snapshot retention and subsequent reconstruction are required assertions.

## Earlier baseline — 10 September

## Executed scenarios

| Scenario | Observed result | Boundary |
| --- | --- | --- |
| Core TypeScript tests | 17 tests across schema, URL policy, project association, semantic comparison and lifecycle ordering | Deterministic tests; not evidence of native application behavior |
| Native Rust tests | 10 tests covering framing, persistence, schema compatibility, client ownership, job recovery and measurement preservation | Uses isolated databases; no user applications closed |
| Real Chrome adapter | 50 tabs reconstructed through Chrome extension APIs, including order, active/pinned state and groups | Isolated Chrome for Testing profile |
| Real browser + native bridge | Durable capture → safety checks → close → window disappearance → resource measurements → native-host/browser restart → restore → verification | Automatic closure restricted to owned input-free fixture pages |
| Lazy Chrome restore | 50/50 tab organization verified; 49 background destinations stayed as placeholders, one active destination loaded; selecting a placeholder loaded its original URL | Organization verified, not loaded page contents or authentication |
| Browser unsafe-page refusal | An ordinary HTTP page blocked automatic close and its window remained open | Does not attempt to infer form/upload safety |
| Real VS Code 1.137.0 | Bridge connected; local workspace/editor metadata persisted; a missing saved editor reopened and was verified | Automatic close remains blocked; exact layout and terminals incomplete |
| VS Code dirty-work preservation | Dirty editor blocked hibernation; file on disk unchanged; unrelated dirty untitled buffer preserved during reconstruction | No unsaved contents captured, discarded or claimed recoverable |
| Native Tauri/WebView2 smoke | Real memory/window observations reached React; project name/path saved; context snapshot survived reload; no JS or native-command errors observed | Does not close user applications or verify restored Explorer/terminal sessions |

Machine-local evidence is in `.validation/browser/results.json`, `.validation/vscode/results.json` and `.validation/desktop/results.json`. That directory is ignored by Git. It contains local paths, snapshots and screenshots of actual window titles; do not distribute it.

## Historical resource experiment — 10 September

The browser test opened **50 sandboxed extension fixture pages**, each explicitly allocating and touching 8 MiB: **419,430,400 bytes total**. This is a synthetic workload, not representative project data. The measurements below came from the successful end-to-end native-bridge run, before and approximately 3.3 seconds after closure.

| Observation | Before (bytes) | After (bytes) | Change |
| --- | ---: | ---: | ---: |
| System available physical memory | 110,456,832 | 1,472,143,360 | +1,361,686,528 (+1.27 GiB) |
| Source browser process-tree private commit | 2,216,787,968 | 552,218,624 | −1,664,569,344 (−1.55 GiB) |
| Source browser process-tree working set | 4,392,890,368 | 792,293,376 | −3,600,596,992 (−3.35 GiB) |
| All readable Chrome working sets | 4,525,322,240 | 924,725,248 | −3,600,596,992 (−3.35 GiB) |

Total physical memory reported was 34,088,693,760 bytes. No sampled Chrome process was unreadable. No process suspension, forced termination or working-set trimming was used by the hibernation operation.

These are separate observations. System available physical memory changes with unrelated activity, particularly under the high initial memory pressure in this run. Working sets can double-count shared pages; private commit is not resident physical RAM. Browser ancestry gives a process cohort, not exclusive per-project or per-tab attribution. A single synthetic run cannot establish typical savings, a guaranteed reclaimable amount, or a project-level RAM recommendation.

The first fixture design shared a privileged extension rendering context and retained memory after closing its window. It did not establish reclamation. The successful experiment used sandboxed pages without extension privileges and retained actual measurements rather than treating window closure as proof.

## Failures that improved the implementation

- A real restore initially verified only 35/50 tabs: group creation moved tabs into a different focused window. Setting the destination window explicitly fixed the loss; the repeated test verified 50/50.
- Restore receipts now include a browser-session identity so reused numeric window IDs cannot masquerade as a previous restore after restart.
- The desktop smoke test exposed initial data loading overwriting newly entered project fields. Inputs now wait for initial data; the repeated UI test saved the entered project successfully.
- Interrupted or expired jobs are recorded without automatically replaying uncertain side effects. Tests cover client ownership and durable measurement preservation.
- Unit-test discovery is limited to `tests/**/*.test.ts` so downloaded VS Code packages cannot introduce unrelated suites.

## Reproduce

Use the setup and native-host registration in [README](../README.md), then:

```powershell
npm.cmd run build
npm.cmd run native:build
npm.cmd test
cargo test --manifest-path native/Cargo.toml --offline
cargo clippy --manifest-path native/Cargo.toml --offline --all-targets -- -D warnings
npm.cmd run test:adapter
npm.cmd run test:browser
node scripts/prepare-vscode-test.mjs
npm.cmd run test:vscode
npm.cmd run test:vscode-close
npm.cmd run test:chrome-cold
npm.cmd run test:vscode-cold
npm.cmd run desktop:build
npm.cmd run test:desktop
npm.cmd run test:checkpoint
npm.cmd run test:project-hibernate
npm.cmd run test:project-vscode
$env:HIBERNATE_TEST_PACKAGE='D:\path\to\ProjectHibernate-0.1.0-...'; npm.cmd run test:packaged-v1
npm.cmd run test:reviewed-browser
npm.cmd run test:quick-sleep
npm.cmd run test:terminal
```

The download step requires network access. Browser registration must authorize the extension ID for this checkout. Integration tests use dedicated profiles/databases and must never be pointed at a personal browser profile. The desktop test's loopback debugging port exists only for the test-launched WebView2 instance.

## Remaining validation gates

### V1 release gate

- Run the already-passing packaged VS Code + Chrome + approved-service scenario across an actual Windows reboot and verify that the durable checkpoint reconstructs only explicitly approved capabilities without replaying interrupted work.

### Post-V1 validation backlog

- Extend reviewed Chrome coverage to more real application behaviors and exact multi-monitor geometry.
- Extend VS Code validation to multiple workbench windows sharing a process, third-party extensions, custom editors and richer layout state.
- Exercise Explorer folder restoration end to end, including Windows 11 tabs/selection and file-operation safety.
- Add package-manager/child-process service launch support beyond direct Node/Python commands.
- Extend deterministic discovery to richer port/project ownership and context-level memory recommendations supported by observations.
- Exercise additional multi-adapter partial-failure combinations; required-capability readiness itself is implemented and covered by unit and packaged combined evidence.

General process suspension, production hardening, installer signing and broad application compatibility remain outside this tested slice. Quick Sleep, payload encryption, retention controls and an unsigned portable distribution are implemented. See [safety](SAFETY.md) and [architecture](ARCHITECTURE.md) for the boundaries.
