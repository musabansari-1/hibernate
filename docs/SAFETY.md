# Safety and compatibility

No adapter in this preview meets **full process hibernation** for general user work. Product readiness requires reliable real-project capture, safe closure, reconstruction and explicit verification, not successful executable launch.

| Adapter | Current depth | Automatic close | Known limits |
| --- | --- | --- | --- |
| Chrome HTTP(S) | Context Restore | Explicit fresh user review required | Forms, uploads, DOM/application state and downloads cannot be reconstructed. Internal/file/incognito URLs are unsupported. |
| Owned browser validation pages | Experimental lifecycle proof | Allowed after fresh safety checks | Synthetic workload, not evidence of arbitrary website safety or predictable real-project savings. |
| VS Code | Context Restore, incomplete | Reviewed clean local text contexts only; native verification requires one workbench window and GUI-process exit | Dirty work, terminals, tasks, debugging and custom editors block close. Exact layout, commands and unsaved-buffer recovery are not reconstructed. |
| Explorer | Folder context | Blocked | Shell-exposed folders only; Windows 11 tab coverage, selection and file operations are unknown. |
| Windows Terminal | Experimental instrumented context | Blocked | Exact cwd handshake available for newly launched no-profile shells. Existing shells, panes and running commands are not reconstructed. |
| Other apps | Unsupported | Never | No arbitrary launch, close, suspend or terminate interface. |

## Close policy

Only the extension's input-free sandboxed validation pages may close without user review. Ordinary pages require a fresh captured snapshot, explicit saved-work confirmation and single-use approval. Session/profile identity and tab/group state are checked again; active downloads, audio and navigation block reviewed closure. Chrome owns unload confirmation. The adapter polls disappearance rather than assuming the windows.remove acknowledgement proves closure; canceling reports failure. An absent unload dialog is never considered proof of saved page state. Quick Sleep discards only eligible background tabs, skipping active, pinned and audible tabs; it is not deep sleep.

VS Code dirty buffers and dirty tabs are safety evidence, never copied or discarded. Reviewed close refuses dirty work, terminals, tasks, debugging and unsupported editor types. Local saved files are checked and live state is revalidated after asynchronous checks. A close command can terminate its extension host and child bridge before acknowledgement. The native observer durably records its binding before close and checks from the desktop/controller independently. Success requires an unambiguous workbench window, its disappearance and GUI-process exit within 30 seconds. Multiple candidate windows and unreadable ownership block close. Expired observations stay unverified. No shutdown command is issued by the observer. Do not assume a global hot-exit setting guarantees every buffer backup.

ContextSleep review does not weaken these adapter rules. All selected windows must pass preflight before any closes are dispatched, and each is checked again immediately before closure. A changed window blocks closing; a failed or unverified close stops later windows. Earlier closed windows remain recoverable from the checkpoint. Unselected and unsupported contexts remain outside automatic close and may still consume resources. `closed-selected` is evidence of selected window disappearance only. Startup cancels queued project closes and never automatically retries an interrupted run; inspect applications before a new capture and review.

Explorer and Windows Terminal are not closed. A folder is not proof that an Explorer file operation or terminal child process is safe to stop. No shell history is collected, and no captured command is executed. Terminal definitions use an explicit Windows PowerShell `-NoProfile` launch. Future trusted-command support needs per-command authorization, an argument-vector model, project/path binding and service-health verification.

## Native authority

Windows discovery opens processes with query-only access. Only named developer applications are displayed. There is no process-suspend, working-set-trim, terminate, arbitrary `WM_CLOSE`, credential access or elevation feature. System/security processes cannot be selected for manipulation through the exposed API.

The browser bridge is an explicit native messaging allowlist, not a localhost listener. The VS Code executable-path setting is machine scoped, unavailable to workspace configuration, and remote/untrusted workspaces are excluded. Native folder launches reject non-local, missing and ambiguous inputs. Reopening a VS Code project follows VS Code's existing trust and native restore behavior; the product does not replay tasks.

The local same-user environment remains trusted. DPAPI encrypts semantic payloads for the current Windows account but does not protect against a malicious process running as that account. IDs, states and resource metrics are not encrypted; Chrome's lazy mappings remain in profile storage. Migration does not erase historical plaintext copies. Framing and profile checks are not a compromised-account security boundary. Snapshot archive, protected export/import and deliberate deletion are implemented; imported snapshots never inherit close authority. Before release, complete schema validation at every boundary, integration enrollment, installer/signing and a threat-model review.

## Claims deliberately withheld

- “Ready to continue working” while any required capability is missing, unsupported or deferred.
- “RAM reclaimed” based only on closing a window or trimming a working set.
- Recovery after an actual reboot until the reproducible reboot scenario has been performed.
- Full browser-session state, exact profile launch, exact multi-monitor geometry, service process ownership or terminal cwd without evidence.
