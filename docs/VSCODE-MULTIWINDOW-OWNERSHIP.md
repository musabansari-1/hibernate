# VS Code shared-process ownership decision — 15 September 2026

**Keep multiple same-process workbench candidates blocked for reviewed automatic close.** No new shared-owner close mode was added. Existing V1 sequencing and the exclusive-window verification contract are unchanged.

## Observed topology

Disposable VS Code **1.137.0** profiles used two separate local folders, a private user-data directory, private extensions and snapshot database. Both fixture extensions reported distinct VS Code session IDs and extension-host PIDs. Each extension host spawned its own Hibernate native bridge. Both chains converged on the same GUI Code.exe PID and process creation identity.

The GUI process owned the `Chrome_WidgetWin_1` candidates. Renderer and extension-host processes in its subtree did not own those HWNDs. In one recorded run the fixture had GUI PID 17160, renderer PIDs 43136 and 32444, extension-host PIDs 46148 and 20360, and bridge PIDs 43244 and 40440 respectively. Three supported-class HWNDs were observed under that GUI PID: two visible and one hidden. Other observations included only one visible candidate plus a hidden candidate. These are time-bound observations, not permanent workbench IDs; visibility is not an ownership filter.

The evidence proves separate integration process trees and shared native window ownership. It does not prove which candidate belongs to either integration. Child process ordering, workspace paths, titles, foreground state and launch-time differences were not used to grant close authority.

## Available handle mechanism and missing contract

The installed build contains `vscode.window.nativeHandle`, but its getter checks the **proposed** `nativeWindowHandle` API. The ordinary installed fixture extension received an explicit proposal-permission error. The shipped Hibernate extension declares no proposed API. The local stable API declarations do not expose this property.

The [upstream proposal](https://raw.githubusercontent.com/microsoft/vscode/main/src/vscode-dts/vscode.proposed.nativeWindowHandle.d.ts) describes a handle that follows the active window. [VS Code's extension-host implementation](https://raw.githubusercontent.com/microsoft/vscode/main/src/vs/workbench/api/common/extHostWindow.ts) initializes and updates that handle through internal messages. That is a promising primitive to investigate when a usable contract exists, but it is not an immutable reviewed-workbench binding. Main/auxiliary window semantics and any changes between capture and close would need to be established. We did not enable the proposal, patch VS Code, attach a debugger, or use internal Electron IPC to bypass the current API contract. [Proposed API guidance](https://code.visualstudio.com/api/advanced-topics/using-proposed-api) explains its development-only stability and distribution constraints.

Required missing primitive: an integration-scoped, supported native-window identity tied to the exact workbench instance, with well-defined lifetime/generation and a fresh recheck before close. Public Electron main-process HWND access does not expose that authority to an ordinary VS Code extension host. Ancestry supplies the shared owner but does not supply the per-window association.

## Selected safety model

The existing binder counts **all** supported-class HWNDs belonging to the discovered GUI ancestor, including hidden ones. Exactly one candidate is required before durable close intent. Multiple candidates remain ambiguous, even if only one is visible or its path/title appears to match. Exact client, instance, snapshot, fresh approval and running job checks are retained. Dirty buffers, terminals, tasks, debugging and custom editor checks are unchanged.

For the established single-candidate case, verification still requires both exact HWND disappearance and GUI-owner process exit. PID plus creation identity prevents a reused PID satisfying the old process identity. HWND reuse within the still-running owner cannot weaken this into success because GUI-process exit is still required.

A future shared-owner contract would have to verify the exact target window's destruction/generation and the exact integration's shutdown while retaining unrelated siblings; shared GUI-process survival would be expected. HWND plus owner PID/creation time alone does not handle HWND reuse within a long-lived shared process. Integration disconnect alone could be reload or crash. A durable destroy-event observation or another trustworthy generation-aware primitive is needed, with unknown/crash gaps left unverified. Do not adopt that contract until its identity and lifetime guarantees are proven.

Existing durable observation/recovery remains read-only: no destructive action is replayed after restart. No new mode, schema or observer was introduced.

## Changes and reproducibility

Only investigation tools were added:

- `native/examples/vscode_topology.rs`: read-only fixture process ancestry, supported HWND candidates and native observation count, built outside the shipped host/desktop API.
- `tests/fixtures/vscode-ownership/`: a fixture-only extension and process-role sampler. The extension can probe its own integration, create/revert one known fixture edit and gracefully close its own clean fixture window. The sampler exports recognized roles and identities, never raw command lines.
- `tests/vscode-multiwindow.mjs`: disposable profile setup, ambiguous-close regression and graceful fixture cleanup. No Code.exe termination fallback.

Build with `cargo build --manifest-path native/Cargo.toml --offline --bin hibernate-host --example vscode_topology`, then explicitly run `node tests/vscode-multiwindow.mjs` in an interactive Windows session. Evidence stays under `.validation/vscode-multiwindow`; it contains fixture process IDs/paths and is not part of the aggregate compatibility diagnostics export.

The live regression was rerun on 17 September 2026 and passed after correcting two fixture-only assumptions: a remaining supported HWND may be hidden, so cleanup waits for exactly one supported HWND rather than one visible HWND; and cleanup-only commands use short reply deadlines so a just-closed extension host cannot keep the Node harness alive. The passing run again proved ambiguous same-process windows are refused before native observation intent, dirty work still blocks preflight, both integrations remain alive after the refusal, and once the second disposable window exits the unchanged exclusive-window path verifies exact HWND disappearance plus GUI-process exit.

No production Rust/extension/UI behavior changed. In particular, `native_close.rs`, `platform.rs`, project sequencing, Chrome, services, terminal support, checkpoint import, readiness and privacy diagnostics were not modified. Existing packaged V1 evidence was not replaced by these source-only probes.

## Next task

Continue the private-beta validation of the currently supported packaged lifecycle in an isolated Windows session. Include two simultaneous VS Code projects as an explicit retained/unsupported close case. Revisit shared-process close only when an exact workbench native identity contract is available. No Explorer, terminal-close, other adapter or reboot work is part of this milestone.
