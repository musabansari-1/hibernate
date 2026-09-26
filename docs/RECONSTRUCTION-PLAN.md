# Generic reconstruction evidence plan — 22 September 2026

Rollback: tag `contextsleep-before-reconstruction-20260922`, commit
`cff461d648265d76c701f2b09f2c27b4d5bb2937`. This detached checkpoint includes
the pre-existing source changes without changing the branch or index. Untracked
demo recordings and distribution archives are excluded.

## Definition before implementation

An application launch, an observed process, an observed window, saved documents,
unsaved state, and workspace readiness are separate reconstruction levels.
Generic Sleep requires a reconstruction contract for all state at risk, an exact
live instance binding, and a conditional close contract. Windows inventory alone
proves none of these three. Even a visibly empty fixture cannot be classified as
stateless by inspecting its title, executable, or window count.

Implement a typed evidence assessment with explicit capabilities and blockers.
Report process identity separately from exact window lifetime identity. Use native
package identity, owner/parent HWND, and WINDOWPLACEMENT as observations. Model OS
restart registration as an activation candidate, not a saved-document contract.
No inferred argument/file association or accessibility property grants authority.

Strengthen executable continuity with volume/file identity alongside the existing
locked SHA-256 check. Distinguish another process of the same executable from the
captured process on restore. Preserve encrypted origins, single-attempt journals,
imports without authority, all-target close preflight, and semantic verification.

Investigate placement only through creation-time startup hints on a newly launched
process; never mutate an arbitrary observed HWND. Attempt only unambiguous normal
single-window captures, and report an observed match separately from semantic
verification. Applications may ignore startup hints. Maximize/minimize, shared
windows, changed displays, and unknown mappings must not acquire geometry claims.

Exercise all fifteen requested cases with native policy tests and disposable
Windows fixtures, clearly marking simulations (PID reuse, inaccessible security,
package identity) rather than pretending to provision privileged/MSIX fixtures.
Run existing Rust/TypeScript checks, isolated adapter regressions and packaged E2E.

## Windows evidence reviewed

- [IsWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iswindow): HWND reuse defeats a lifetime proof made from repeated snapshots.
- [GetApplicationRestartSettings](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getapplicationrestartsettings): a bounded registered argument string is available with PROCESS_VM_READ; this is not a state recovery guarantee.
- [RegisterApplicationRestart](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-registerapplicationrestart): the application supplies arguments and may change them during shutdown. A pre-close observation need not be the final recovery recipe.
- [Restart Manager application guidelines](https://learn.microsoft.com/en-us/windows/win32/rstmgr/guidelines-for-applications): applications must save their own state and working directory. Installer shutdown is process/resource scoped, not authority over a selected logical window. Do not invoke RmShutdown or recovery callbacks.
- [STARTUPINFO](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-startupinfow): initial geometry is advisory and applies only to applications accepting default creation coordinates.
- [WINDOWPLACEMENT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-windowplacement): normal-position workspace coordinates must not be treated as screen coordinates.
- [GetPackageFullName](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getpackagefullname): package identity is observable, but activation does not identify a saved logical window.

Command line and cwd are not universal reconstruction recipes. A launch argument
can perform a one-shot action; an existing file can contain executable content;
file association, protocol, Start Menu, App Paths and AUMID identify routes rather
than saved state. UI Automation does not expose a universal dirty/saved contract.
Keep those classes blocked until a validated semantic adapter supplies the missing
evidence. Chrome, VS Code, service, Explorer and terminal boundaries stay intact.
