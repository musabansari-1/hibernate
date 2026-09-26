# Generic Windows app architecture — validation

## Reconstruction extension — 22 September 2026

Validation is complete for this extension's stated boundaries. The section headed
Earlier release validation describes the earlier build separately.

Rollback: `contextsleep-before-reconstruction-20260922` at
`cff461d648265d76c701f2b09f2c27b4d5bb2937`.

The first new native run passed 73 unit tests and the existing service integration;
28 TypeScript tests/type checking and native Clippy also passed. The live native
reconstruction lifecycle passed in
`.validation/reconstruction/fe93d722-17fc-44ba-b42f-76f7cc48aa9b/results.json`.
The expanded native run passed at
`.validation/reconstruction/ee6cf9cf-960a-4672-a6e6-ceb4987d745d/results.json`,
including destruction/recreation of the selected HWND while its process remained
alive. The final DPI regression passed at
`.validation/reconstruction/7444639d-4734-4690-8176-22a67b782152/results.json`, with
a per-monitor-aware test host and a DPI-unaware fixture. Final validation passed
**74 native unit tests**, the existing real service integration, **28 TypeScript
tests**, TypeScript checking, and native Clippy with `--all-targets -- -D warnings`.
Native logs are `.validation/reconstruction-rust-tests.log` and
`.validation/reconstruction-clippy.log`.
Early fixture runs exposed mixed path separators in the test scope and extra
OS-created IME helper windows; the fixture now uses native paths and disables IME
only inside its own disposable process. Those failed attempts are not passes.

| Required fixture | Evidence and generic decision |
| --- | --- |
| Simple Win32, no document state | Live: app launch and startup geometry observation; restore only; Sleep refused because statelessness is not OS-proven. |
| Command-line arguments | Live arguments fixture: args exist, but no validated restart semantics; bare app restore only, no argument recovery. |
| Working directory | Live non-default cwd fixture: original cwd not reconstructed; restore only. |
| Multiple windows | Live two visible plus one hidden: independent observations; no layout or close authority. |
| Two windows in one process | Same live fixture: grouped identity is not per-window close authority. |
| Same executable, separate processes | Live: distinct creation identities; another instance blocks duplicate launch and is reported separately. |
| File/document argument | Live local-file argument fixture: document recovery remains unsupported; requires adapter. |
| Unsaved state | Live in-memory unsaved fixture: no WM_CLOSE received; requires adapter and remains open. |
| Packaged/MSIX/UWP | Synthetic package identity policy test; actual MSIX installation/activation is not tested or supported. |
| Restart between review and close | Live replacement process after retained review; remains ineligible; restore reports different instance. |
| Stale HWND | Live destroyed/recreated-window selection rejected while the owner process remains alive, and again after exit; identical-metadata HWND recycling covered synthetically and never treated as lifetime proof. |
| PID reuse | Synthetic same PID with changed creation time revokes selection; OS PID reuse is not forced. |
| Elevated/inaccessible | Synthetic unavailable/elevated evidence refuses launch/close; no elevation attempted. |
| Executable moved/changed | Live moved and byte-identical replacement refused; changed-content/file-lock unit test. |
| Ambiguous app | Live hidden/shared ownership and registered-restart fixture stay open. |

Registered restart presence/flags were obtained through the real Windows API;
arguments were neither captured nor executed. The native lifecycle asserts review
and forged-selection refusal, no close marker, existing-instance observation,
new-process geometry observation, false semantic verification and one-attempt
refusal. Fixtures stop themselves using adjacent marker files.

Safety contract changes: no relaxation of close, origin, journal or readiness
gates. The read-only access envelope now includes the documented VM_READ right for
the size-only restart query. Bare launch is stricter for restart/package/unknown
metadata and byte-identical file replacement. Startup geometry hints are a new
creation-time side effect limited to newly launched processes. Old generic captures
must be refreshed before relaunch. No personal window/data mutation or reboot.

Reproduce the new native fixture with
`cargo build --manifest-path native/Cargo.toml --offline --example desktop_fixture`
and `cargo test --manifest-path native/Cargo.toml --offline --test reconstruction_windows -- --ignored --nocapture`.
Run `npm.cmd test`, `npm.cmd run typecheck`, the normal Rust suite and native
Clippy with `--all-targets -- -D warnings`. Build with `npm.cmd run desktop:package`;
the packaged generic script additionally tests startup geometry through actual
Tauri commands using the copied fixture executable.

### Full regression sweep and final package

`.validation/suite-1790068510296/results.json` records these exact scripts:
`browser-integration`, `browser-close-safety`, `reviewed-browser`, `quick-sleep`,
`vscode-integration`, `vscode-close-reopen`, `desktop-smoke`, `project-discovery`,
`project-checkpoint`, `project-hibernate`, `project-vscode`, `chrome-cold-restore`,
`vscode-cold-restore`, `service-checkpoint-lifecycle`, `vscode-multiwindow`,
`browser-adapter`, `generic-apps-packaged`, and `packaged-v1`.
Seventeen passed on that build; the packaged generic geometry assertion failed
closed because a per-monitor-aware Tauri host observed a DPI-unaware fixture in
different coordinates. It reported `observedMatch=false` and `verified=false`.
The fix scopes geometry observation and launch to the target DPI context and
compares recorded coordinate-space evidence. The strict assertion was retained.

Final package: `dist/ContextSleep-0.1.0-2026-09-22T09-29-26-210Z`.
All three final packaged cases passed in
`.validation/suite-1790086330309/results.json`: `generic-apps-packaged` (15 seconds),
`packaged-v1` (62 seconds), and `service-checkpoint-lifecycle` (16 seconds).
The generic evidence is
`.validation/generic-apps/run-1790086334280/results.json`. It includes native
discovery, encrypted save, native/UI/forged-selection close refusal, unrelated
fixture survival, import-origin refusal, application-presence-only observation,
durable single-attempt restart behavior, and actual packaged startup-geometry
reconstruction with semantic readiness still false. The screenshot was inspected:
Restore only, Kept open, unsupported saved state and incomplete readiness are visible.
The V1 test used the packaged host and extensions and verified the preserved deep
Chrome/VS Code/service lifecycle. All 18 named cases therefore passed across the
recorded runs; the initial geometry failure remains in the evidence.

Both explicit terminal regressions also passed using newly created fixture shells:
`powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/terminal-integration.ps1`
with the packaged host (`.validation/terminal/results.json`), and
`cargo test --manifest-path native/Cargo.toml --offline --test terminal_context -- --ignored --nocapture`
(`native/.validation/terminal-context/54277e08-abae-402c-b6c9-ee1b56e926ec/results.json`).
Their normal ignored entries were not counted as passes before these explicit runs.

The final package documentation and SHA256SUMS manifest were refreshed after
validation without changing its runtime payloads. No personal windows were closed,
personal data edited, personal host registration replaced, or Windows rebooted.

## Earlier release validation — 21 September 2026

Implementation started 21 September 2026. See [architecture and OS evidence](GENERIC-APP-ARCHITECTURE.md) and [safety boundaries](SAFETY.md).

## Scope of the delivered support

Ordinary visible, titled top-level Windows desktop windows are discovered without application registration. Readable process identity enables encrypted metadata capture. This includes shared-process and multi-window Electron applications; process grouping is an observation, never a semantic workspace binding. Hidden windows count toward process-sharing evidence but are not offered as user workspace entries. Inaccessible identities remain discovery-only. Other desktops, sessions, background-only applications and brokered child surfaces are not universally discoverable.

Generic capture records window title/class/rectangle, HWND/thread/PID/process creation identity, executable path, parent PID, shared-window count, elevation, optional window AppUserModelID and relaunch-metadata availability. It does not capture documents, unsaved work, process memory, command history, authentication or application internals. Window layout is recorded, not restored.

Generic restore can observe an existing app or, after explicit selection, attempt a no-argument launch of an unchanged eligible local GUI executable. The original process must have ended, matching background processes block launch, and a native protected capture origin must authorize it. One durable attempt covers all snapshots of the same captured process. Imported snapshots never gain launch authority. Brokered/packaged routes, shells/runtimes/system hosts, elevated apps, special launch metadata and known specialized apps are excluded from generic launch. Application updates invalidate the captured executable digest.

Every generic result remains semantically unverified. **Unknown apps stay open during Sleep and therefore do not gain generic RAM reclamation.** Their presence cannot make an entire workspace Verified. Chrome and VS Code retain specialized capture, exact reviewed-close and semantic verification contracts; services retain their separate explicit-command and process/listener ownership rules. Explorer and terminal cwd support remain context-only. Other apps need specialized cooperation before document recovery or safe automatic closure can be promised.

## Preserved invariants

- Query-only OS discovery, no injection, process-memory reads, privilege elevation or replay of taskbar/restart command strings.
- PID plus creation identity; HWNDs are recyclable observations, never close leases. Parent PID, executable names, AUMID and shared Electron ownership cannot grant exact-window authority.
- Short-lived native selections are re-observed at capture. Missing or changed evidence fails closed.
- DPAPI-protected payloads and FULL-sync SQLite journals. Capture/origin are committed together; launch intent is committed before spawn. Imports strip authority; interrupted operations do not replay.
- All selected specialized parts preflight before any close; an uncertain or failed close stops the sequence. Generic entries cannot enter that path, even through forged selections.
- No forced generic termination, working-set trimming, generic WM_CLOSE, or claim to capture arbitrary app state.
- Existing Chrome/VS Code snapshot versions remain unchanged. Database schema migrates from 5 to 6; code rollback does not downgrade data.
- Tests use isolated databases, WebView storage, Chrome profiles, VS Code user data and disposable apps. Canonical database-directory identity binds the isolated desktop lease; path aliases share one owner, and an override cannot target the normal data directory. Fixture-only native messaging registration never replaces the user's installed host.

## Evidence

Rollback tag: `contextsleep-before-generic-apps-20260921` at `6aecacefdea22edc9b3e7e1c7059dcc841326da3`.

Native validation passed 68 unit tests and one real approved-service integration test, plus Clippy with warnings denied. TypeScript validation passed 28 tests and type checking. Production frontend/extensions, native host and portable desktop packaging succeeded. Regression coverage includes expired selection, PID reuse, missing identity, hidden/shared windows, forged/imported authority, changed/locked executables, unsafe launch paths, reserved native-client spoofing, database-path aliases, repeat launch and false semantic readiness.

Adapter suite `.validation/suite-1789998697813/results.json` passed browser integration, browser close safety, reviewed-browser, quick-sleep, VS Code integration, VS Code close/reopen and VS Code multi-window ownership. The separate 50-tab browser adapter regression also passed.

Desktop suite `.validation/suite-1790015168766/results.json` passed desktop smoke, project discovery, project checkpoint, project hibernation and project VS Code. Its cold-restore/service cases exposed outdated harness selectors, subsequently corrected. Both cold-restore cases passed in `.validation/suite-1790015380541/results.json`. The service-only selective restore passed against the package in `.validation/suite-1790015710125/results.json`. One development-host service signal attempt failed closed; the development-host rerun passed in `.validation/suite-1790015788959/results.json`. The shipped helper passed both the standalone service lifecycle and combined V1 lifecycle. The initial signal failure is retained in the logs and should be monitored as an intermittent console-helper limitation; it never triggered forced termination. Earlier parallel build/live-test resource timeouts are not counted as passes; heavyweight validation now runs sequentially.

The first packaged generic lifecycle passed in `.validation/generic-apps/run-1789998637481/results.json`: actual packaged Tauri discovery of two visible plus one hidden shared-process fixture windows, encrypted Save, UI and forged-close refusal, unchanged unrelated fixture, existing-app observation without duplication, imported-origin launch refusal, process restart without replay, explicit one-shot relaunch and repeat refusal. All generic results stayed unverified. The runtime package `dist/ContextSleep-0.1.0-2026-09-21T13-58-22-735Z` passed generic revalidation in `.validation/generic-apps/run-1790015675105/results.json` and combined V1 validation in `.validation/suite-1790015613754/packaged-v1.log`. V1 used the shipped native host, a fixture copy of the shipped Chrome extension (only its native-host registration name changed), and the extension extracted from the shipped VSIX. It verified reviewed Chrome/VS Code/service shutdown, dashboard restart without replay, cold reconstruction of all three capabilities and persisted Verified readiness. No generic entry was included in that semantically Verified workspace.

The packaged terminal-directory test passed (`.validation/terminal/results.json`), and the opt-in multi-context terminal regression passed (`native/.validation/terminal-context/956d632c-92b9-4bb1-bde3-7c143b2df936/results.json`). Both used newly created instrumented shells, left unrelated fixture shells alone during the lifecycle, refused automatic terminal close and replayed no commands. The generic checkpoint screenshot was inspected: it clearly shows Kept open, application presence observed, saved state unsupported and workspace readiness incomplete.

## Final delivered package

`dist/ContextSleep-0.1.0-2026-09-21T18-45-47-359Z` passed all four final packaged cases in `.validation/suite-1790016371119/results.json`: generic apps (10 seconds), combined V1 (46 seconds), service-only lifecycle (16 seconds), and Chrome cold restore (17 seconds). This run used automatic extraction of the shipped VSIX and the shipped native host. Generic evidence is `.validation/generic-apps/run-1790016372811/results.json`, including the same-database path-alias duplicate-owner refusal. All 26 packaged payload hashes were verified; documentation was refreshed after validation and its hashes regenerated without changing executables or extension payloads.

The final native rerun passed 68 unit tests plus one live service integration test and Clippy; all 28 TypeScript tests and type checking passed. The opt-in terminal tests had already passed and were not silently counted as part of the normally ignored test run. The full existing integration suite passed across the recorded sequential runs. No reboot, personal-window close, personal integration registration replacement or personal-data edit was performed.

## Reproduction

Run `npm test`, `npm run typecheck`, `cargo test --manifest-path native/Cargo.toml --offline`, and `cargo clippy --manifest-path native/Cargo.toml --offline --all-targets -- -D warnings`. Build with `npm run desktop:package`; compile the disposable Win32 fixture with `cargo build --manifest-path native/Cargo.toml --offline --example desktop_fixture`.

`node tests/run-validation.mjs` runs the desktop/adapter suite sequentially with a unique temporary native-messaging registration. Set `HIBERNATE_TEST_PACKAGE` to the built distribution and run `node tests/run-validation.mjs packaged-v1 generic-apps-packaged service-checkpoint-lifecycle` for packaged validation. The runner uses the packaged host, copies the shipped Chrome extension into the fixture, and extracts the shipped VSIX without installing it into a personal profile. The existing VS Code fixture executable location must be configured. Live terminal checks are `npm run test:terminal` and `cargo test --manifest-path native/Cargo.toml --offline --test terminal_context -- --ignored`.

Build first, then run live GUI suites; avoid simultaneous release compilation and heavyweight GUI fixtures. Temporary registration is removed in `finally`. Relaunched browser fixtures are closed using their dedicated CDP connection, rather than merely disconnecting the test client. Never run these harnesses against an existing personal profile or database.

## Remaining limits

No Windows reboot was performed. The build remains unsigned/private beta. A fixture proves the policy and lifecycle, not compatibility with every third-party application. Per-app document semantics, safe-close cooperation, shared-process exact-window integration bindings, UWP/MSIX activation routes, launchers/singleton handoff, elevated/security boundaries and application-specific recovery need additional adapters or validated OS contracts. No generic mechanism can truthfully supply arbitrary application memory or unsaved-state restoration.
