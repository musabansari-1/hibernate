# ContextSleep — private beta

A local-first Windows app for putting a **development workspace** away and reconstructing it with explicit verification instead of guessing. The current beta combines a Tauri 2 desktop app, React/TypeScript UI, Rust Windows layer, SQLite journal, Chrome Manifest V3 extension and VS Code extension. Core operations run locally and require no cloud or runtime AI service.

**The supported V1 lifecycle is working and packaged, but this is still a private beta.** VS Code, Chrome and explicitly approved direct Node/Python services can participate in a durable project checkpoint; the validated packaged flow can close supported parts, restart ContextSleep, cold-launch the enrolled Chrome profile and saved VS Code workspace, restart approved services, and return project readiness to `ready`. Explorer and instrumented terminals remain Context Restore only, ambiguous VS Code window mappings fail closed, the build is unsigned, and an actual Windows reboot validation is still outstanding. No adapter is labelled full process hibernation.

## Run

Open Windows apps now appear automatically without an adapter setup step. Select app contexts alongside Chrome/VS Code entries and save one workspace checkpoint. Generic context saves window/executable metadata and may reopen an eligible unchanged app executable; it **does not capture documents or unsaved state, automatically close unknown apps, or verify their working state**. Specialized adapters add capabilities only where supported. See [architecture and boundaries](docs/GENERIC-APP-ARCHITECTURE.md).

The compiled desktop application is `apps/desktop/src-tauri/target/release/project-hibernate.exe` after `npm.cmd run desktop:build`.

Development prerequisites: Windows 10/11, WebView2, Node 22.12+ (tested on Node 24), Rust MSVC toolchain and Visual Studio C++ build tools.

```powershell
npm.cmd ci
npm.cmd run native:build
npm.cmd run build
npm.cmd run desktop
```

`npm.cmd run dev` is a frontend-only preview: it deliberately shows no invented Windows readings. `npm.cmd run desktop:build` produces the actual standalone desktop executable; an installer and signing are not yet configured.

`npm.cmd run desktop:package` builds a runnable Windows distribution under `dist/ContextSleep-<version>-<timestamp>`. It includes the release native host, Chrome extension, VSIX, setup instructions and SHA-256 file hashes. `dist/latest-package.json` points to the latest folder. Packaging does not register integrations, install software or publish anything. The distribution is unsigned and has no installer or automatic updates.

## Connect Chrome

**Packaged beta:** ContextSleep discovers supported apps automatically on startup. It silently installs/refreshes the VS Code adapter and pre-registers Chrome using the bundled extension's stable ID. VS Code reconnects automatically whenever a trusted local project opens. Chrome reconnects automatically after one unavoidable browser approval: enable Developer mode in `chrome://extensions`, click **Load unpacked** once, and select the folder ContextSleep opens for you. There is no extension-ID copy/paste or manual Connect step.

Manual development fallback:

1. Build the extensions with `npm.cmd run build`.
2. Open `chrome://extensions`, enable Developer mode and load `extensions/chrome/dist` as an unpacked extension.
3. If native-host registration is needed manually, register the bundled stable extension ID:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/register-native-host.ps1 -ExtensionId lilgdmjjanejoihlpegfklkfaofebfhn
   ```

The extension reconnects automatically in the background. Its popup only provides a retry fallback. The native messaging registration is per-user and restricted to the ContextSleep extension ID.

### Real browser validation

Use **Test without touching real work → Create validation window**, select that window, then **Capture workspace context**. The pages label their synthetic memory allocation. The automated fixture test exercises durable capture, safety checks, window closure, disappearance verification, and signed resource measurements. Resume reconstructs organization with lightweight placeholders for background tabs. Selecting one loads its original destination.

For ordinary HTTP(S) windows, capture first, then use **Review & sleep** on the saved snapshot. Review recoverable state and confirm saved work. Approval expires after five minutes and is single-use. The extension freshly checks profile/session identity, tabs/groups, downloads, audio and navigation before requesting closure. Chrome's unload confirmation remains under your control; canceling preserves the window and reports failure. Form contents, uploads, authentication and DOM state are not captured. The saved-work confirmation is a user decision, not an automated proof that a website is safe.

**Quick Sleep** in the review dialog discards eligible background tabs while keeping their window open. Active, pinned and audible tabs are retained. Chrome wakes discarded pages when selected. This is a separate operation from close/reconstruction and makes no reboot or exclusive RAM-savings promise.

For cold restore, explicitly bind the enrolled Chrome profile once in the desktop UI. ContextSleep stores the approved executable/profile-directory binding, can relaunch that exact profile after a ContextSleep restart, waits for the same extension identity to reconnect, and then reconstructs the saved browser context. Removing/reinstalling the extension can invalidate its local profile identity and lazy-tab mappings; do not remove it while relying on saved browser context. Actual recovery across a Windows reboot is still unvalidated.

## Connect VS Code

**Packaged beta:** no VS Code setup is required. ContextSleep detects VS Code on startup, installs/refreshes the packaged VSIX automatically, prepares a stable per-user native bridge, and the extension reconnects whenever a trusted local project opens or reloads. `hibernate.nativeHostPath` remains available only as an advanced override.

An installable VSIX can be built using `npm.cmd run vscode:package`. Install the resulting `extensions/vscode/project-hibernate-0.1.0.vsix` through **Extensions: Install from VSIX**.

Set this **User** setting to the actual native host path:

```json
{ "hibernate.nativeHostPath": "D:\\projects\\hibernate\\native\\target\\debug\\hibernate-host.exe" }
```

Open a trusted local project and run **ContextSleep: Connect Local Bridge**. Capture saves folder/workspace identity, editor groups, visible selections/ranges, dirty-state indicators and shell-integration cwd where available. It stores no source buffers or terminal history. Reviewed close is restricted to unchanged, clean local text-editor contexts without terminals, tasks or a debugger. Native close verification requires an unambiguous workbench window in the bridge's process ancestry and observes both its disappearance and GUI-process exit. A durable observer survives bridge termination. Ambiguous mappings block closing; timeouts stay unverified; system memory observations are not exclusive project savings. VS Code owns hot-exit backups and trust decisions. Only one connected window per workspace may own queued operations; close review is bound to its captured window session.

For cold restore, ContextSleep can launch the saved local VS Code workspace and wait for the original integration identity to reconnect before reconstructing saved editors. Missing local text editors are reopened without closing existing editors. Known terminal directories are recreated in instrumented no-profile PowerShell shells; distinct terminals each require an observed cwd to count as verified. The native bridge binds prompt reports to the integration, session, PID and process creation time. Previous commands, custom editors, exact layout and source-buffer recovery are not restored. Remote, virtual and untrusted workspaces are unsupported, and multiple workbench windows sharing one GUI process remain ineligible for reviewed automatic close because exact native-window ownership cannot be proven safely.

Run **ContextSleep: Open Instrumented Project Terminal** inside VS Code to create a shell whose cwd can be captured without relying on VS Code's optional shell integration. The command saves the current project context first. The helper reports only cwd at prompts; external terminals with no reliable cwd signal remain unknown.

## Project and folder context

Create projects and assign individual integration windows. Membership persists; browser membership uses a session identity so reused numeric window IDs do not silently join another project. VS Code folder signals, available terminal cwd and Shell-exposed Explorer folders produce path-boundary matches. Read-only repository inspection adds branch and GitHub remote evidence. Localhost alone remains a suggestion, never ownership or close authorization. No process command lines are scraped.

**Discovered repositories** automatically groups reported local folders under their Git roots, including prompt folders from live instrumented terminals. Accept a suggestion to create a project or open an existing one; assignments remain explicit. Discovery checks bounded folder ancestry on launch and every 30 seconds while the dashboard is visible. It ignores stale integration reports and remote paths. Project cards retain the last foreground observation from assigned Chrome or VS Code windows. Missing observations do not prove inactivity, and exclusive project RAM estimates remain unavailable.

**Capture workspace context** saves a project checkpoint containing the selected application snapshots, capture failures, repository evidence and current service expectations. Optionally include local folder/terminal definitions. Choose **Review and sleep**, review the saved state, select supported windows and confirm saved work. Every selected window receives a fresh preflight before the first close and is checked again before its own close. A failed or unverified close stops later windows; unsupported contexts stay visible. Verified selected closure and actual memory observations are recorded separately from overall project readiness. Individual snapshot reviews remain available.

Use the checkpoint to selectively reconstruct these parts as one operation. Service checks use capture-time endpoints, even if project settings later change. Every step is journaled before execution and retains its own observations, failures or skipped state. Desktop restart marks unfinished operations interrupted, cancels queued project closes and never automatically replays them. This build upgrades the database to schema 5; use its matching desktop, native host and integrations together. Older builds cannot open the upgraded database.

Service checks also sample Windows TCP listener ownership for the connected endpoint and show its executable, PID and process creation identity when available. This helps inspect a surprising healthy response, but does not prove repository ownership or authorise process shutdown. HTTP response and listener evidence are retained separately.

**Capture folders and terminals** (Settings) stores Shell-exposed Explorer folders inside the selected project plus live process-identity-verified instrumented terminal working directories when available (up to 20). If none are live, ContextSleep saves one project-root terminal definition. Restore opens fresh no-profile PowerShell prompts and verifies the saved cwd handshake; it never replays commands, history, panes or process state. Existing arbitrary terminal windows are never automatically closed.

Save explicit HTTP loopback health endpoints under **Development services**. Verification issues bounded HTTP GET requests and never follows redirects. You can optionally save a direct Node/Python launch definition; starting or checkpoint-restoring it requires explicit review, duplicate/occupied-port checks, durable process identity and listener/HTTP verification. Only the exact previously verified ContextSleep-launched process is eligible for graceful automatic shutdown; there is no force-kill fallback. Closing the desktop hides it to its tray; use **Quit ContextSleep** to exit. A second launch brings the existing desktop forward.

## Tests

```powershell
npm.cmd test
cargo test --manifest-path native/Cargo.toml --offline
npm.cmd run test:adapter
npm.cmd run test:browser
node scripts/prepare-vscode-test.mjs
npm.cmd run test:vscode
npm.cmd run test:desktop
npm.cmd run test:checkpoint
npm.cmd run test:reviewed-browser
npm.cmd run test:quick-sleep
npm.cmd run test:terminal
```

Browser tests require Playwright's installed Chromium/Chrome for Testing. Set `HIBERNATE_TEST_CHROMIUM` to a compatible test browser executable if necessary. `test:adapter` independently exercises the real Chrome APIs. `test:browser` additionally requires the native host registration for the ID shown by `node tests/browser-integration.mjs --probe`. It uses an isolated browser profile and SQLite database; never use a personal browser profile for tests. It does not reboot Windows.

The VS Code preparation script downloads an official standalone test installation; `test:vscode` uses isolated user, extension and workspace directories. Alternatively set `HIBERNATE_TEST_VSCODE` to a compatible test executable. Build the native host and extensions before adapter tests. `test:desktop` requires the release desktop build and launches it with a separate database and a test-only WebView2 debugging port. Its screenshots include real local window titles and must remain private. Vitest only discovers this repository's unit tests, excluding downloaded integration fixtures.

See [specification coverage and remaining work](docs/PRODUCT-STATUS.md), [validation](docs/VALIDATION.md), [architecture](docs/ARCHITECTURE.md), [safety and compatibility](docs/SAFETY.md) and [decisions](docs/DECISIONS.md).

## Local storage

Default database: `%LOCALAPPDATA%\ProjectHibernate\hibernate.db`, with SQLite WAL files. `HIBERNATE_DATA_DIR` overrides the location for isolated development/tests. Semantic payloads, project metadata, integration reports and operation results use Windows current-account DPAPI encryption. This is payload encryption, not full-database encryption: IDs, operation states and resource readings remain readable. It does not protect against malicious programs running as the same account. Earlier plaintext copies/backups are not securely erased by migration; Chrome's lazy destination mappings remain in its profile-local extension storage.

Snapshots can be archived, exported as account-protected `.hibernate` files, imported, or permanently deleted after typing their ID. Imports do not carry close approval or pending commands. Backups require the original Windows account; they are not cross-account portable. Corrupt snapshots remain visibly unreadable while other history stays available. Checkpoints reference individual snapshots, so deleting a referenced snapshot makes that part unavailable to reconstruction. Exported backups are retained after deletion. No telemetry is installed; do not distribute your database or `.validation` directory.
