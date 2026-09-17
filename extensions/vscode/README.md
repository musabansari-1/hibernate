# ContextSleep (private beta)

Captures local project/editor metadata for ContextSleep. No source buffers, terminal history or environment variables are captured. Reviewed clean closure is experimental: dirty work, terminals, tasks, debugging and unsupported editors block closure, and native verification requires an unambiguous workbench window plus observed GUI-process exit. Multiple windows sharing a process block reviewed closure.

Set `hibernate.nativeHostPath` in **User Settings** to the absolute built `hibernate-host.exe` path, then run **ContextSleep: Connect Local Bridge** in a trusted local workspace. The desktop can capture metadata and reopen/check saved local text editors. Remote and untrusted workspaces are unsupported.

Use **ContextSleep: Open Instrumented Project Terminal** to create a no-profile PowerShell shell with process-bound cwd reporting. Saved known terminal directories can be reconstructed, and distinct terminals require distinct observed handshakes. Unknown external-terminal cwd is never guessed. Previous commands, history, exact layout, custom editors and unsaved-buffer recovery are not restored. VS Code owns hot-exit backups.

Use one connected window per workspace. Reviewed closure is bound to the captured window session; reconnecting or opening another window requires a new capture before closure. An interrupted operation is retained and never automatically replayed. This is a local development integration, not a published Marketplace extension.
