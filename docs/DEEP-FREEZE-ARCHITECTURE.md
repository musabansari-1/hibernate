# Deep Freeze architecture

## Current supported beta — 30 September 2026

The desktop now enables a deliberately narrow catalog: the exact official
SumatraPDF 3.6.1 x64 portable image with SHA-256
`719f689b34f47be8ca105ce8484948474dafde0e106bab599e4a89326070c3d0`, on Windows
x64 build 26200. Discovery requires no Sumatra integration or user configuration.
Every selected process includes all its windows. Other app versions and families
remain unavailable for Deep Freeze. Chrome/VS Code retain their existing verified
workspace workflow; native window names never grant an adapter identity.

Authority requires retained exact process and executable handles, a live earlier
parent, no observed children, matching interactive logon/session, non-elevated,
non-critical, non-protected and unpackaged status, a known app/window contract,
and no active TCP connection/listener. Changed reviews fail closed. Opaque reviews
expire in 20 seconds and cancellation drops their handles. The independent expiry
worker continues when the WebView is hidden.

The job policy has changed from the initial experiment below. Observed membership
is permitted **only with this explicit independent-process contract**; unknown
membership still fails. Job membership never defines the selected group. The
engine neither opens nor modifies the containing job. Live private shared-job
testing establishes that the peer continues during freeze and controller exit
releases only the target pause. It does not validate every external supervisor.
The local-process preservation promise is conditional on the app remaining alive:
external job termination, watchdogs and remote timeouts remain possible. Review
explicitly warns that other software can close the app while paused.

This is a deliberate best-effort boundary, not a claim that all existing jobs are
safe. Microsoft documents job inheritance, limits and external termination in
[Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).
The previous blanket membership exclusion conflated the job's lifetime policy
with the scope of this process-handle operation. Exact app identity, independent
ownership and target-only handle operations remain mandatory.

Mechanism: exclusive, non-inherited `NtCreateProcessStateChange` handles and
`NtChangeProcessState`; both are undocumented native APIs. `EmptyWorkingSet`
evicts resident pages after confirmed suspension. Commit remains allocated and
system-wide RAM savings are not guaranteed. Windows remain visible. Wake and
controller teardown release owned state handles; restart reconciles the DPAPI
journal without replay, generic resume counts, relaunch or kill fallback.

The supported beta is not support for arbitrary apps, process trees, unsaved
state after process exit, Windows reboot, network/device sessions, or other OS
builds. See [current validation](DEEP-FREEZE-VALIDATION.md) for actual release gates.

## Historical design and investigation (26–28 September)

The remainder records earlier decisions and failures. In particular, its closed
catalog and blanket job exclusion are superseded by the current policy above.

Phase 1 decision, 26 September 2026. Rollback tag:
`contextsleep-before-deep-freeze-20260926` at `d2e23b744737e3c10a6fa83dee7850b1c9ed126d`.
Generic reconstruction, semantic adapters and their close contracts are retained.

### Implementation status and production gate

Research, native fixtures and draft desktop integration are implemented. They are
**not a released Deep Freeze feature**. The desktop now has a memory-first view,
process-deduplicated measurements, native eligibility assessments and review/Wake
commands. `deep_freeze::CATALOG_RELEASED` is a compile-time **false** release gate:
no discovered app can obtain freeze authority. There is no environment, import or
JavaScript override. The native messaging host exposes no freeze command.

The shared `freeze_engine` implements owned state handles and the journal. Only
the `deep-freeze-lab` feature exposes fixture access and fault injection; packages
do not enable it. Product SQLite remains schema 6. The separate `deep-freeze.db`
uses explicit schema 1, FULL-sync intent and DPAPI payloads and rejects unknown
versions. Its identities and reviews never become authority after restart/import.

The mechanism passes ordinary fixture freeze/Wake, thread churn, same-process
volatile-state checks, partial-error release, overlapping-owner isolation and
controller-exit tests, including exit before confirmation commit. Testing is on
Windows build **10.0.26200.0** only. See [measured evidence](DEEP-FREEZE-VALIDATION.md).
This does not establish arbitrary-application eligibility or cross-version support.

**Production gate:** implement positive non-security application classification
and a defensible process-group contract before enabling discovered apps. The
conservative first release should support positively identified and validated app
families automatically, with unfamiliar apps retained as restore-only. An absent
denylist match, a signature, a parent PID, or one visible window is insufficient.
The live orphan-worker test demonstrates why checking only current descendants
cannot prove the original app group. It does not mean that every single-process
app is unsafe; it means that the proposed automatic proof is incomplete.

A candidate contract pins the official SumatraPDF 3.6.1 x64 portable image to SHA-256
`719f689b34f47be8ca105ce8484948474dafde0e106bab599e4a89326070c3d0`.
Its Authenticode signature was valid during acquisition. This proves image identity,
not full application safety. A private restricted-mode empty window passed the
primitive freeze/Wake experiment. **The production contract has not passed.**
Test-runner and Windows-management launches remained job-managed; another shell
launch lacked a surviving observable parent. They were refused without freezing.
Job breakaway was also either denied or left job membership present. The job policy
was not weakened. These failed tests do not prove that ordinary installations are
eligible or that SumatraPDF itself creates the job.

Draft eligibility requires exact held process/file identity, same interactive
logon/session, non-elevated/non-critical/non-protected/unpackaged status, no existing
job, live earlier-created parent, no current children, no TCP connection/listener,
the pinned image and document-window class. All of a process's visible and helper
windows are selected together. Topology is checked before and after suspension;
changes release every owned pause. Other external work remains unknown and would
require an explicit warning and acknowledgement. Only tested Windows x64 build
26200 can pass the draft OS gate. Historical/brokered workers and a real document
lifecycle still need family-level validation before the release gate may change.

Reviews retain process/file handles, expire after 20 seconds of monotonic time and
are single-use. Wall-clock values shown in the UI do not grant extra lifetime.
An independent native worker discards expired reviews even when the WebView is
hidden. Canceling a review releases its unused handles immediately. Neither path
releases or changes an active sleep operation.
Frontend commands accept opaque native selection/review IDs, never caller PIDs or
assessments. Wake removes the owned operation before opening the journal, so a
database error drops the state handles. Startup records owner exit without replay.
The existing desktop single-instance lease and checkpoint operation mutex remain
the ownership/serialization boundaries. Normal quit also releases held handles.

Phases 5–6 are draft integration with a passing refusal path, not a passing enabled
app lifecycle. Phase 7 remains gated: no new Deep Freeze portable package.

### Mechanism decision and release gates

28 September job follow-up: a new disposable test assigns two harness-owned
processes to a private job with no limits or kill-on-close. Freezing one does not
stop its peer; normal Wake and controller teardown preserve the target's state.
This establishes handle isolation in that configuration, not safety under an
unknown supervisor. The production job exclusion remains. Microsoft's
[IsProcessInJob documentation](https://learn.microsoft.com/en-us/windows/win32/api/jobapi/nf-jobapi-isprocessinjob)
only supplies membership; it does not return the target's job handle. Unknown
limits and nested jobs cannot be certified by that boolean. A further private
Explorer-launched candidate still reported both controller and target in jobs.
No personal app was assigned to a job or suspended.

Investigate **Windows 11 process-state change objects**, dynamically resolving
`NtCreateProcessStateChange` and `NtChangeProcessState` from the loaded ntdll.
These are **undocumented native APIs**, not a supported Win32 contract. The first
implementation is a disposable experiment. Missing exports, failed rights, failed
state transitions or failed crash-release validation must block deployment.
There is no fallback to thread suspension, NtSuspendProcess, process termination,
debugger attachment or a job-object kill policy.

| Candidate | Decision |
| --- | --- |
| Documented SuspendThread / ResumeThread | Debugger-oriented; a thread may hold locks. Enumerating threads is not atomic with thread creation. Suspend counts have no durable caller ownership. A crash between increment and journal commit cannot safely be repaired by guessing counts. Rejected. |
| DebugActiveProcess | Changes debugging behavior; requires event handling and changes failure semantics. Unsuitable for ordinary apps. Rejected. |
| Documented job objects | Good for processes launched into an owned job, with important nesting/breakaway limits. No documented general freeze contract. Retrofitting an existing app changes its lifetime restrictions and cannot recover already escaped children. No assignment of user apps to jobs. |
| Native job freeze information | Undocumented, app may already belong to jobs, and tree ownership is still unproven. Rejected for V1. |
| NtSuspendProcess / NtResumeProcess | Undocumented, no owned undo handle; does not solve crash-gap ownership or group boundaries. Rejected. |
| Native process-state object | Handle-scoped freeze; release is expected to undo this object's state. Validate normal release and abrupt controller exit on real disposable processes before further phases. No inherited/duplicated state handles. |

Primary implementation references: [PHNT declarations](https://github.com/winsiderss/systeminformer/blob/master/phnt/include/ntpsapi.h)
and [System Informer freeze/thaw implementation](https://github.com/winsiderss/systeminformer/blob/master/phlib/native.c).
The latter uses an exclusive state object and process SET_INFORMATION plus
SUSPEND_RESUME rights. These sources are evidence of use, not Microsoft support.
Crash cleanup and new-thread behavior are release tests, not assumed guarantees.

Additional primary research: [state-object ownership and cleanup](https://windows-internals.com/thread-and-process-state-change/)
and [the author's suspension experiments](https://github.com/diversenok/Suspending-Techniques).
The latter distinguishes thread-count suspension from process freezing and
investigates future-thread behavior in specific Windows builds. Neither is a
stable Microsoft API guarantee. Current tests exercise ordinary thread churn;
externally created threads, unusual bypass flags and kernel/device behavior are
not claimed as validated. No injection or bypass tooling is included.

Microsoft references: [SuspendThread](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-suspendthread),
[ResumeThread](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-resumethread),
[job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects),
[DebugActiveProcess](https://learn.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-debugactiveprocess).

### Ownership and threat model

All authority originates in a fresh native review, never a frontend PID, imported
snapshot, saved capability flag, executable name or parent PID alone. Bind a held
process handle to PID, creation time and executable path; capture parent observations,
all top-level window counts, package, job, session and security evidence. Revalidate
before changing state. A changed or unreadable identity blocks the operation.

The initial production candidate is a **whole, single-process interactive app**.
No automatic recursive suspension. Any observed child, missing/reused parent,
job membership, unreadable topology, unselected same-owner window (including hidden
windows), app broker or known semantic app blocks generic freeze. Recheck children
after freezing; a late child causes release of the owned freeze and a failed result.
Two independent instances of one executable remain independent identities.
Parentage is not proof of semantic ownership; arbitrary process trees remain a
future contract. New threads, externally created threads, bypass-freeze threads,
broker-created workers and escaped descendants require explicit validation and
may force a narrower release gate.

Security policy must combine OS evidence with exclusions: self and controller
ancestors, critical/protected processes, service/system accounts, different sessions
or users, elevated/unknown tokens, Windows components, Explorer, shells, runtimes,
brokers and known security tools. Unknown evidence fails closed. An executable-name
denylist alone cannot prove that an app is not antivirus or service infrastructure;
if that role cannot be established, it remains outside the release boundary.
No elevation, debug privilege, injection or process-memory capture is permitted.

The trusted boundary remains the local Windows account. Freezing is not isolation
from a malicious same-user process. A running app can share locks, devices, memory,
IPC or remote leases with others. OS inventory cannot prove absence of those links.
Network, audio, device sessions, uploads/downloads and remote timers may fail during
sleep. Best-effort local memory preservation never establishes semantic correctness.

`FreezeAssessment` must expose independent identity, group, elevation, critical,
shared-window, external-work risk, suspension, trim and safe-to-freeze evidence,
with explicit blockers. Missing data is not false. Safest strategy ordering is
VerifiedReconstruction, GenericDeepFreeze, RestoreOnly, Blocked; a supported Chrome
or VS Code instance keeps its existing semantic path. Discovery must not fabricate
an adapter-to-window binding.

### Durability and recovery design

Before suspension, FULL-sync commit an operation ID, exact process identities,
group relationships, controller identity, prior window observations and intended
state. Then change state using a non-inherited exclusive state handle. Persist
confirmation and measurements. Only the live controller's held handle authorizes
Wake. Repeated Sleep cannot add counts; repeated Wake cannot decrement another
owner's suspension. A partial operation releases every handle acquired by it.
Resume errors remain visible; handle release is the safety backstop and must itself
be validated. A journal write failure after freezing triggers immediate release.

On controller death Windows is expected to release the state objects. Prefer that
automatic safety release to retaining frozen apps across ContextSleep crashes.
Startup never suspends or calls a count-based resume by PID. Reconcile journal
identities as still present, ended, replaced or unknown; report interrupted sleep
and expected automatic release without inventing proof of application health.
While the owner is alive, sleeping apps have an explicit Wake action. An owner
that cannot be proven dead must not have its journal recovered by another instance.
Do not relaunch externally terminated apps automatically. Existing explicit generic
Restore retains its separate authority and cannot promise unsaved-state recovery.

Database work, if reached, is an explicit forward-only schema migration. Freeze
authority is local, excluded from export/import. Existing desktop lease and operation
lock serialize lifecycle work. An app-wide exit releases freezes; hiding to tray
does not. No watchdog process may inherit a state handle accidentally.

### Physical memory semantics

After confirmed freeze, request [EmptyWorkingSet](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-emptyworkingset).
This requests eviction of eligible resident pages; it does not free committed
virtual allocations. [SetProcessWorkingSetSize](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setprocessworkingsetsize)
with both limits -1 can also trim; persistent limits and Ex hard-limit flags are
unnecessary. Never tune global pagefile or working-set policies.

Measure independently: working set, private commit, system available physical RAM,
timestamps, API outcome and same identity before/after/wake. Use
[GetProcessMemoryInfo / PROCESS_MEMORY_COUNTERS_EX](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex)
and [GlobalMemoryStatusEx](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex).
Shared pages can be double-counted; system deltas include unrelated activity and
can be negative. Show selected resident memory as an upper bound, not guaranteed
reclaim. Backing store and commit remain; waking incurs page faults and latency.

### Windows and UX

Do not destroy, hide, recreate or overlay arbitrary windows. V1 leaves windows
visible and unchanged, with a clear warning that they will not respond until Wake.
This avoids an HWND-reuse crash-recovery visibility hazard. Minimizing before
freeze remains a later feature requiring reliable per-window restoration; saved
WINDOWPLACEMENT alone is not an HWND lifetime identity. [ShowWindowAsync](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindowasync)
only queues work and does not prove that minimizing happened.

The main UI should show total RAM/pressure, running apps with measured resident
memory, safe available method, reviewed Sleep, and a separate Sleeping/Wake list.
Technical evidence belongs in expandable diagnostics. External-work limitations
must appear before a generic freeze. Existing checkpoint-based semantic Sleep and
Restore retain their reviews and verification. Never label an unavailable adapter
as verified, or a relaunch as recovery of unsaved work.

### Sequential validation gates

1. This research/architecture and rollback tag.
2. Disposable native GUI fixture: exact identity, frozen heartbeat, preserved
   in-memory state, same-process Wake, abrupt controller exit, thread churn.
3. 512 MiB touched allocation; actual physical and commit measurements.
4. Durable intent/confirmation, injected failures and crash/restart journal checks.
5. Native assessments/strategy and every ownership/security refusal.
6. Memory UI with no new authority in JavaScript.
7. Packaged regression only after prior gates pass.

Tests must cover GUI+worker, late child, same executable independent processes,
shared windows, orphan worker, exits/PID reuse, access/elevation simulations,
network activity, repeat operations and partial freeze/resume errors. Unsupported
cases must prove refusal, not be relabeled as passes for supported freeze. Use
only harness-launched fixtures, isolated profiles/data and cooperative cleanup.
Existing Rust/TypeScript, Clippy, frontend, Chrome/VS Code/service/terminal/generic
and packaged regressions remain mandatory. Record failed gates honestly; do not
enable downstream production behavior on the strength of a happy-path experiment.
