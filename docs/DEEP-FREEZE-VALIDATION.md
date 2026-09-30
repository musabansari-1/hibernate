# Deep Freeze validation

## Supported beta — 30 September 2026

The pinned SumatraPDF catalog is now enabled in the working tree. The revised
job boundary is documented in [the architecture](DEEP-FREEZE-ARCHITECTURE.md):
membership is observed, never used to infer a process group, and is allowed only
under the exact independent app contract. Unknown apps remain blocked.

Native production discovery/review/Sleep/Wake passed with a disposable pinned app:
`.validation/deep-freeze-catalog/4eb4b8e5-cd4a-4774-a70c-92c3c05ed689/results.json`.
The extended repeat also rejected duplicate selections, canceled, unacknowledged,
expired and changed-window reviews, then passed a ten-second freeze and same-process
Wake: `.validation/deep-freeze-catalog/558ca5ca-7704-4da5-9b94-120a15869bf9/results.json`.
These native cases used restricted-mode empty windows; the real-document desktop
and package gates are recorded separately below.

The enabled desktop lifecycle passed with an ordinary-mode private app and two
generated PDF documents in separate windows:
`.validation/deep-freeze-ui/run-1790688905521/results.json`.
Actual UI selection, cancellation, acknowledgement, Sleep and Wake were exercised.
Both frame windows responded before Sleep, stopped responding while frozen, and
responded after Wake. The original PID/creation identity survived. A second Sleep
followed by test-controller termination released the pause before restart; restart
recorded `owner-exited` and did not relaunch the app. Screenshots were inspected.
The harness supports `DEEP_FREEZE_DESKTOP_EXE` to run the same checks on a package.

Real-document app measurements: working set **34,852,864 → 0 bytes**, private
commit **14,585,856 bytes unchanged**, system available physical memory
**3,013,812,224 → 3,022,503,936 bytes**. The 8,691,712-byte system change is an
observation, not exclusive savings attributed to the app.

Current checks: 81 Rust unit tests plus service integration, 28 TypeScript tests,
Clippy with warnings denied, typecheck, frontend and release desktop build passed.
All six explicit native live tests passed in 17.62 seconds. Logs are
`.validation/deep-freeze-release-{rust,ts,clippy,live}.log`,
`.validation/deep-freeze-enabled-build.log`, and
`.validation/deep-freeze-enabled-ui.log`.

### Portable package

`dist/ContextSleep-0.1.0-2026-09-29T13-42-28-755Z/ContextSleep.exe`

The packaged Deep Freeze test passed in 31 seconds with the real desktop binary,
ordinary-mode SumatraPDF, and two generated PDFs. Evidence:
`.validation/deep-freeze-ui/run-1790689386145/results.json`. It checked native
window responsiveness before/frozen/after Wake, same process identity, whole-process
selection, cancellation/acknowledgement, forged IPC refusal, controller termination,
and recovery on restart. Wide and narrow screenshots were inspected.

The combined packaged Chrome + VS Code + approved-service checkpoint lifecycle
passed across desktop restart and verified cold restore in 77 seconds. Packaged
generic reconstruction also passed, including refusal of an unrelated image using
the Sumatra window class:
`.validation/generic-apps/run-1790689478184/results.json`.
The packaged native host passed the isolated Terminal cwd regression:
`.validation/deep-freeze-release-terminal.log`.

The first suite is `.validation/suite-1790689383671/results.json`; the corrected
packaged-component repeats are `.validation/suite-1790711237583/results.json`.
Failures are retained: the new geometry fixture initially changed its own class
between capture and restore, and several older test drivers ignored the package
host/extension environment overrides. The latter produced a VS Code schema-version
failure and an interrupted browser fixture operation. Drivers now honor those
overrides; original close, restore, geometry and verification assertions remain.
The corrected browser integration and VS Code integration passed in 31 and 43
seconds. No product safety check was bypassed to obtain these results.

Final result: all **19 lifecycle scenarios** have passing results across those
two suites and the generic repeat. All nine packaged-component repeats passed.
This includes the 16 existing browser/editor/workspace/service scenarios plus
packaged Deep Freeze, packaged generic reconstruction and packaged combined V1
restore. The separate native generic and Terminal regressions also passed.
The package includes `RELEASE-VALIDATION.json` and refreshed `SHA256SUMS.json`;
its desktop and host binaries are unchanged from the versions exercised above.

For a repeat, set `DEEP_FREEZE_DESKTOP_EXE` to the packaged executable and run
`node tests/deep-freeze-desktop.mjs`. The harness needs the pinned official portable
candidate in `.validation/deep-freeze-catalog/sumatra-3.6.1/`. It copies it into a
fresh private directory, generates its own documents, uses isolated desktop data,
and closes only that private copy. Lab fixtures/fault injectors are not packaged.

### Scope still excluded

This is a working narrow beta, not an "almost any app" compatibility claim. Other
families/versions and Windows builds, ambiguous or child-process groups, inaccessible,
elevated, critical, protected, packaged/system/security processes and active TCP work
remain unavailable. No state survives target exit or Windows reboot. External job
supervisors, device/network sessions and remote leases remain outside preservation.
Chrome/VS Code keep their existing reviewed workspace path; binding those semantic
instances into the same native memory selection requires further ownership work.
Broader app-family, long-duration and OS-version validation remains necessary.

Latest 512 MiB touched-memory evidence:
`.validation/deep-freeze-memory/875882f2-a99c-46b7-81b6-e3a983c7bad7/results.json`.
Working set was 544,948,224 bytes before, 2,547,712 frozen, and 539,938,816 after
Wake/activity. Private commit stayed 539,340,800 bytes. System available RAM rose
by 1,413,120 bytes immediately and continued changing during the frozen interval.
The same PID/creation identity, volatile secret and every touched-page pattern
survived; page faults increased on Wake. Concurrent machine activity prevents
attributing all system-memory changes to this fixture.

## Historical experiment record (26–28 September)

The closed-gate status and blanket job exclusion in the historical record below
describe earlier builds and are superseded by the current status above.

26 September 2026. This records an **unreleased experiment**, not completion of
the requested seven-phase product feature. Rollback tag:
`contextsleep-before-deep-freeze-20260926`, commit
`d2e23b744737e3c10a6fa83dee7850b1c9ed126d`.

### Boundary

28 September continuation: completed native review expiry independent of dashboard
polling, plus explicit review cancellation from the desktop. Added and passed a
sixth live test for two disposable processes in one private, no-limit job: only
the selected process paused; its peer kept running; normal Wake and controller
exit preserved the original process/state. Evidence:
`.validation/deep-freeze-job/46223b6f-f10d-40c4-95ba-210b0c7e184d/results.json`.
This does **not** validate unknown external job limits or supervisors.

An additional Explorer launch outside the sandbox completed read-only launch
diagnostics and cooperative cleanup. Both harness and target still reported job
membership. Evidence:
`.validation/deep-freeze-catalog/f734f183-1280-410a-ae54-6d4c8209f6a9/launch-evidence.json`.
The candidate harness now records this evidence before its gated eligibility
check. The production job exclusion and closed catalog gate remain unchanged.

Continuation verification: 80 Rust unit tests plus service integration passed
(`.validation/deep-freeze-resume-rust.log`); all six explicitly enabled live tests
passed in 15.91 seconds (`.validation/deep-freeze-resume-live.log`); 28 TypeScript
tests, Clippy with warnings denied, typecheck, frontend and release desktop builds
passed (`deep-freeze-resume-ts.log`, `deep-freeze-resume-clippy.log`, and
`deep-freeze-resume-desktop.log` under `.validation/`). The rebuilt desktop's real
UI/native refusal checks, including idempotent cancellation of an unknown review,
passed at `.validation/deep-freeze-ui/run-1790591763428/results.json`.
The first sandboxed UI attempt timed out connecting to WebView2; its failure is
retained in `deep-freeze-resume-ui.log`. The outside-sandbox repeat passed in
`deep-freeze-resume-ui-unsandboxed.log`. No enabled production lifecycle or new
portable package is claimed by these results.

Implemented: research, native fixtures, memory measurements, encrypted durable
journal/crash recovery, typed policy, draft native security/topology/TCP probes,
desktop review/Wake IPC and a memory-first UI. Product schema remains 6; the separate
freeze journal has explicit schema 1. No imported/serialized data grants authority.

The compile-time `CATALOG_RELEASED=false` gate refuses all discovered apps. The
native messaging host has no freeze command. Only feature-enabled lab entry points
can exercise the primitive with harness-owned targets. Positive app-family
eligibility and an enabled desktop lifecycle remain incomplete, so **no phase-7
package or packaged freeze pass is claimed**. The memory UI and refusal path are
draft progress, not completion of the user's requested feature.

The official SumatraPDF 3.6.1 x64 portable candidate was hash-pinned and signature
checked. An empty, restricted-mode private copy passed primitive freeze/Wake.
Production-policy attempts did not pass: direct launches inherited a job; Windows
management/explicit-breakaway launches still reported a job; another shell launch
had no observable surviving parent. Failed logs are retained under
`.validation/deep-freeze-production-candidate-*.log`. No unsafe policy exception
was added and no personal process was suspended. The final direct-spawn refusal
regression runs with `--expect-refusal` and records `refusal.json`; it must not be
counted as a successful production freeze lifecycle.

### Mechanism and safety observations

`NtCreateProcessStateChange` + `NtChangeProcessState` are dynamically resolved,
undocumented native APIs. Exclusive non-inherited state objects are held alongside
exact PID/creation-time/executable process handles. Normal Wake operates on that
object; release of an object cannot release another object's freeze in the tested
case. No SuspendThread, NtSuspendProcess, kill, debug privilege, injection, window
hide/minimize or arbitrary close fallback is implemented. Windows remain visible.

On controller exit, Windows released its handles and the exact fixture resumed.
The controller uses `process::exit`, which skips Rust destructors, after either
confirmation or the suspension/confirmation gap. This tests process teardown,
not every possible crash, machine reboot or kernel failure. Recovery records
owner exit and exact target observations. It never suspends, decrements a generic
thread count or relaunches an app. A live/unreadable owner is not recovered.

FULL-sync intent is committed before suspension; confirmation follows suspension.
Partial freeze/confirmation failures release every owned object before recording
failure. Partial Wake failures attempt every target and release remaining objects;
errors remain visible. Same-target duplicate intents serialize through SQLite.
The test-only fault injector is confined to the disabled experiment feature.

### Live results

OS: Windows **10.0.26200.0**, x64. Live suite log:
`.validation/deep-freeze-live-final.log`. Five live tests passed in the initial
complete run (9.33 seconds):

| Test | Evidence directory under `.validation/` |
| --- | --- |
| Same-process state, repeat operations, confirmed-owner exit | `deep-freeze/a7a4d0a4-75cd-42a5-8fae-3d735ce3d129/results.json` |
| 512 MiB touched memory | `deep-freeze-memory/4ab1ef11-e1a4-46bf-8480-7f43e1ad1686/results.json` |
| Partial failures and journal reopen | `deep-freeze-journal/87d0cea2-56fd-40b7-9ce6-0e362b495db6/results.json` |
| Crash before confirmation | `deep-freeze-gap/d8dd7753-9df8-44c5-bd16-aa63031d0e07/results.json` |
| Topology, separate instances, overlapping owners, loopback activity | `deep-freeze-boundaries/0566e968-bf63-4774-a27d-ccd1786bc666/results.json` |

All target processes are private fixture copies launched by the harness with
per-run directories and bounded lifetimes. Tests write cooperative stop markers;
there is no target-kill fallback. Native discovery is scoped to the fixture image.
Parent/child enumeration is read-only. The controller itself never receives
arbitrary frontend process IDs.

### Measured memory (bytes; MiB = 1,048,576 bytes)

Latest repeat, 27 September, `.validation/deep-freeze-live-integrated.log`: all five
live tests passed in 20.83 seconds. Memory evidence is
`.validation/deep-freeze-memory/ca189aa6-cd07-4aaf-9d48-b92711d33d69/results.json`.
Working set: **544,874,496 → 0 → 537,780,224 bytes** (before, immediately after trim,
after Wake). Commit stayed **539,336,704 bytes**. Same PID/creation identity and
volatile state survived. At about 3.2 seconds frozen, working set was 12,288 bytes.
System available RAM initially **fell by 231,858,176 bytes**, then rose above the
initial reading during the frozen interval; a desktop build was running concurrently.
This is why working-set reduction is not reported as guaranteed system-wide savings.
Earlier runs below had positive system deltas and are retained rather than cherry-picked.

| Counter | Before | After freeze + trim | After Wake + activity |
| --- | ---: | ---: | ---: |
| Process working set | 544,841,728 | 3,268,608 | 540,499,968 |
| Private commit | 539,348,992 | 539,348,992 | 539,348,992 |
| System available physical | 3,959,013,376 | 4,105,912,320 | 4,015,529,984 |
| Page faults | 133,374 | 133,983 | 265,143 |

Working set fell by **516.48 MiB**. System available physical memory increased
by **140.09 MiB** over the sampling interval; that is a whole-machine observation,
not exclusive fixture savings. Private commit did not fall. Wake faulted pages
back in; every touched page's pattern and a randomly generated volatile string
survived in the same PID/creation identity. The fixture remained functional.

Earlier runs observed system-available changes of 6.73 MiB and 33.51 MiB despite
roughly 519 MiB working-set drops. This variation is retained rather than treating
working-set eviction as guaranteed usable RAM. Only a measurable fixture working-set
drop (>1 MiB), preserved allocation/state and increased faults are asserted; no
exact reclaim percentage or positive system-wide delta is required.

### Requested cases and honest coverage

| Case | Coverage |
| --- | --- |
| Single GUI process | Live freeze, stopped heartbeat, same-process Wake and state validation. |
| GUI with child worker | Live parent/child evidence; pure policy refuses unproven group. Not generic tree suspension. |
| Late child | Live creation after the earlier topology observation; no inference of continued exclusive ownership. |
| Same executable, independent processes | Live: selected process freezes; other continues. |
| Two windows, one process | Live shared-window observations; pure policy refusal. |
| Parent exits, child survives | Live orphan remains exact/alive; no automatic orphan freeze. |
| Exit between review and freeze | Live retained handle and reopen both refuse. |
| PID identity changes | Synthetic changed creation time; no forced OS PID recycling. |
| Inaccessible/elevated | Independent policy simulations; no elevation or protected-process testing. |
| High memory | Live 512 MiB committed/touched fixture. |
| Active network | Live loopback exchange; pure active-work policy blocks it. No production network detector claimed. |
| Controller crash/restart | Live teardown after confirmation and before confirmation; journal reopened and reconciled. |
| Repeated Sleep | Native lease refuses; durable operation rejects duplicate target. |
| Repeated Wake | Idempotent; does not clear another state object's freeze. |
| Partial freeze failure | Injected second-target failure; first target resumes with preserved state. |
| Partial resume failure | Injected second-target error; object release resumes it and records errors. |

Additional: confirmation-write failure, journal reopen, protected metadata,
unknown schema refusal, semantic-strategy priority and separate critical/user/
package/job/ownership/trim-rights blockers. Ordinary thread churn runs in the
fixture. No claim of remote-thread, driver, GPU, audio or all-version validation.

### Regression checks and reproduction

Baseline: 74 Rust unit tests + service integration, 28 TypeScript tests. Rust and
TypeScript suites were repeated after the early mechanism/memory phases. Final
logs and additional lifecycle results are listed below as they finish.

27 September integration checks:

| Check | Result / evidence |
| --- | --- |
| Rust, all features | 79 unit tests + service integration passed; `.validation/deep-freeze-rust-final.log` |
| Rust, default features | 79 unit tests + service integration passed; `.validation/deep-freeze-final-default-rust.log` |
| Clippy, all targets/features, warnings denied | Passed; `.validation/deep-freeze-clippy-final.log` |
| TypeScript | 28 passed; `.validation/deep-freeze-integrated-ts.log` |
| Typecheck + production frontend + release desktop | Passed; `.validation/deep-freeze-desktop-final.log` |
| Native live freeze suite | Five passed, explicitly run; `.validation/deep-freeze-live-integrated.log` |
| Generic reconstruction | Passed; `.validation/deep-freeze-generic-regression.log` |
| Isolated Terminal context | Passed outside sandbox; `.validation/deep-freeze-terminal-regression-unsandboxed.log`; first sandbox run could not launch Windows Terminal |
| Real Tauri memory/refusal UI | Passed, with wide/narrow screenshots inspected; `.validation/deep-freeze-ui/run-1790515342591/results.json` |

The same UI/IPC refusal checks also passed against the final monotonic-review build:
`.validation/deep-freeze-ui/run-1790515928985/results.json`. The compiled desktop is
`apps/desktop/src-tauri/target/release/project-hibernate.exe`; it is a development
artifact with the catalog gate closed, **not a new portable Deep Freeze release**.

The first memory UI harness incorrectly launched its disposable GUI with `SW_HIDE`,
so discovery could not see it; its first cleanup also chose a helper window. The
harness was corrected to show the explicitly requested fixture and close only its
own enumerated windows. The successful repeat preserved the original app identity,
left the freeze journal empty, rejected forged IPC, and had no UI runtime errors.
The failed attempt remains in `.validation/deep-freeze-ui-validation.log`.

All 16 existing adapter/workspace scenarios have passing final results across
`.validation/suite-1790515038457/results.json`,
`.validation/suite-1790515259785/results.json` and
`.validation/suite-1790515670042/results.json`: browser integration/close safety,
reviewed browser sleep, Quick Sleep, VS Code integration/close, desktop smoke,
project discovery/checkpoint/hibernate/VS Code, Chrome/VS Code cold restore,
service lifecycle, VS Code multiwindow and browser adapter.

The first two 50-tab browser attempts failed when Playwright retained a worker
that Chrome restarted during fixture setup. `tests/browser-integration.mjs` now
reacquires the worker during setup and observes each numbered fixture URL before
creating it, making setup repeatable without duplicating tabs. Retry is restricted
to that setup and its discovery read. Capture/close/restore assertions and their
failure behavior were not relaxed. The final repeat passed in 24 seconds; failed
logs are retained in the earlier two suite directories.

The enabled production-app freeze lifecycle and packaged freeze regression are
still **not passed**. Do not combine the passing refusal regression with the
passing low-level fixture mechanism and call that an end-to-end app feature.

```powershell
cargo build --manifest-path native/Cargo.toml --offline --features deep-freeze-lab --example freeze_fixture --example freeze_controller
cargo test --manifest-path native/Cargo.toml --offline --features deep-freeze-lab --test deep_freeze_windows -- --ignored --nocapture --test-threads=1
cargo test --manifest-path native/Cargo.toml --offline --all-features
cargo clippy --manifest-path native/Cargo.toml --offline --all-targets --all-features -- -D warnings
npm.cmd test
npm.cmd run build
```

Candidate refusal reproduction (separate from the primitive suite): download the
[official portable 3.6.1 x64 archive](https://files.sumatrapdfreader.org/software/sumatrapdf/rel/3.6.1/SumatraPDF-3.6.1-64.zip)
into `.validation/deep-freeze-catalog/sumatra-3.6.1/`, preserving the archive's
`SumatraPDF-3.6.1-64.exe` filename. The harness verifies the pinned SHA-256 before
copying it into a new private directory and launches only that copy with a private,
restricted profile. `cargo run --manifest-path native/Cargo.toml --offline
--features deep-freeze-lab --example freeze_candidate -- --expect-refusal` checks
the closed release gate. Omitting `--expect-refusal` requests a full candidate
lifecycle and currently fails; it is not an enable switch.

`node tests/deep-freeze-desktop.mjs` tests the built desktop's actual memory UI and
IPC refusal with the same pinned private image. It is deliberately labelled a
refusal regression, not a packaged or successful Deep Freeze lifecycle.

Live cases are intentionally ignored in an ordinary `cargo test`; they are counted
only after the explicit command. The experiment controller and GUI fixture are
not copied into portable packages. No new Deep Freeze package is published or
claimed until the preceding product gates pass.

### Work required before a broad product claim

Complete a real app-family lifecycle under the production policy, including real
documents, all windows, historical/brokered workers, stable parent/job observations
and every changed-review refusal. Exercise the draft desktop-owned leases and Wake
UI with a legitimately eligible app before opening the compile-time catalog gate.
Broaden supported Windows builds, network/device and lengthy-sleep validation;
finish verified-adapter strategy mapping without guessing identity from filenames.
Then run the new portable package and packaged freeze lifecycle. Until those pass,
there is no new package path to report. Existing reconstruction remains available.
These experiments cannot honestly support “put almost any app to sleep”.
