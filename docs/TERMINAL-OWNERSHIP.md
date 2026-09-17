# Windows Terminal ownership decision — 15 September 2026

**Decision: retain Context Restore. Do not implement automatic Terminal closure.**

This is a bounded rejection for the current Windows Terminal integration, not a claim that safe terminal ownership is impossible with another host or a future protocol. Installed stable Terminal: **1.24.11911.0**. Existing V1 app/service behavior is preserved; no reboot was attempted.

## Evidence and ownership unit

The provable unit today is an instrumented PowerShell process identity, associated with a Hibernate launch/session ID and project in the local database. Its child bridge checks the reporting parent executable/PID, and subsequent observations check process creation identity. This establishes the source of a past prompt cwd observation. It does **not** establish present idleness, exclusive console membership, or a native Terminal tab/window close target.

The existing `restore_for` already uses `wt -w new new-tab`. Keep that separation. A named window would improve routing but would not establish ownership: the [documented CLI](https://learn.microsoft.com/en-us/windows/terminal/command-line-arguments) targets existing named windows and creates new ones when absent. Additional tabs/panes can join; indices and names are not exclusive leases. The initial isolated probe successfully added an independently launched fixture tab to its named window. Do not consolidate a project's shells into a supposedly owned named window on that evidence.

Microsoft documents that [GetConsoleWindow under a pseudoconsole](https://learn.microsoft.com/en-us/windows/console/getconsolewindow) supplies a message-queue handle rather than the presented terminal window. The first live probe additionally showed `IsWindowVisible` true for its shell console handles. The test's initial assumption that this flag must be false was wrong. Consequently visibility is not proof of a close target either. Subsequent handle/class probes did not complete because fresh fixture launches stopped reporting; exact native mapping is **unproven**, not guessed from window differences or titles.

The [Terminal session-management design](https://raw.githubusercontent.com/microsoft/terminal/main/doc/specs/%235000%20-%20Process%20Model%202.0/%234472%20-%20Windows%20Terminal%20Session%20Management.md) discusses window/session routing; design proposals are not a deployed ownership contract. There is also a newer [Microsoft Intelligent Terminal COM protocol](https://raw.githubusercontent.com/microsoft/intelligent-terminal/main/tools/wta/README.md) with window/pane operations. The three initial stable-Terminal fixture shells did not receive its `WT_COM_CLSID`. We did not install another Terminal build, query terminal contents, or assume this separate project's API is supported by the installed app. Even an inventory API would need a conditional close contract to prevent membership changing between inspection and closure.

## Conditions required before enabling close

For a dedicated window, require an authenticated launch-bound association among project ID, fresh session nonce, shell PID/creation identities, host PID/creation identity, HWND and a window-generation identity. HWND plus host creation time alone cannot exclude HWND reuse inside the same long-lived host. A title, cwd, environment string or launch-time window difference is insufficient.

The host must expose complete current tab/pane membership, including any user-added work, and reject a close if that membership/generation changes. A read-only enumeration followed by WM_CLOSE has a race. Named-window routing is not a membership lock. Shell cooperation must independently establish safe idleness and decline while commands, child work, jobs or unresolved interactive state are active. A prompt report may predate a long-running command; even no child processes would not prove an in-process PowerShell command is idle.

If that contract becomes available, persist the exact review/identity set before dispatch, expire single-use authority, and fail closed on unknown membership/identity. Verify **every bound shell exited and the exact owned window disappeared**. Do not require WindowsTerminal.exe to exit: a shared host may legitimately retain unrelated windows. Do not interpret host survival as failed closure or host exit alone as success. Unrelated windows must remain untouched. Timeout/ambiguous identity stays unverified; no force-kill fallback.

A narrower alternative is a cooperative protocol that lets only an explicitly owned idle shell exit itself, without sending any window close. That could reclaim the shell while leaving the Terminal window alive, and must be described that way. It still requires a fresh, atomic shell-side safe-exit decision; the current prompt reporter cannot provide one. A Hibernate-controlled terminal host could supply stronger boundaries but is a separate implementation decision, outside this task.

## Implementation and interruption

Only terminal verification behavior changed: `restored_prompt` requires the exact session, expected cwd and currently matching PID/creation identity. PID values outside `u32` are refused instead of truncating. The same checked identity predicate serves live observations. No new runtime authority is placed in checkpoint data, and no terminal launch/close replay mechanism was added. Existing recovery still does not replay interrupted checkpoint work. Ambiguous terminal contexts remain ineligible for automatic project close.

No behavior changes were made to shared native close, service, browser, readiness, import or project sequencing. `cargo fmt` also formatted existing code in `checkpoints.rs`, `context.rs`, `hibernation.rs` and `store.rs`; these are formatting-only changes. `platform.rs` and `native_close.rs` were not changed. No new schema, adapter, UI or packaging change was introduced.

## Validation and limits

Passed this turn:

- **55 Rust unit tests**, including exact terminal session/cwd identity, reused and oversized PID rejection; existing project isolation, multiple cwd entries, import authority stripping, readiness and native-close recovery tests.
- **1 real service integration test** covering the existing approved service lifecycle.
- Native Clippy, all targets, warnings denied; native host build.
- **23 TypeScript tests** and frontend/extensions production build.

Live **Context Restore** validation now passes, while automatic Terminal closure remains rejected:

- Initial `tests/terminal-window-probe.ps1` reached two distinct fixture windows and added a third shell to the first named window. It also exposed that the earlier console-HWND visibility assumption was invalid. Those observations still support the decision that a named window/title/console handle is not exclusive close authority.
- Opt-in `native/tests/terminal_context.rs` now passes in an interactive Windows session. The earlier `launched-unverified` failure was traced to Windows path-separator normalization: the exact session and live PID/creation identity were valid, but equivalent `D:\\...` and `D:/...` cwd strings compared unequal. The verifier and project scoping now normalize Windows separators before comparison.
- The passing live regression proves two distinct live project cwd contexts, exclusion of an unrelated project even when it uses the same folder tree, removal of PID/session/runtime authority from checkpoint data, refusal of automatic Terminal close before side effects, survival of the unrelated fixture shell, no shell launch during recovery/reopen, and two fresh restored PowerShell identities at the saved cwds with no command/history replay.
- Product-owned shell exit as a Hibernate close action, exclusive native-window disappearance, unrelated-window survival during an automatic close operation, and native Terminal tab/window reclamation have **not** been demonstrated. No Terminal shared-host reset or interaction with existing user terminals was used to obtain the passing Context Restore result. Existing packaged V1 evidence remains unchanged by this terminal-only validation.

Next recommended coverage task: retain Terminal as Context Restore only and move to another developer-workspace capability such as Explorer multi-folder reconstruction. Before reconsidering native terminal hibernation, choose and prove the cooperative safe-exit or host ownership contract above. Explorer native close remains out of scope until its own ownership/safety model is proven.
