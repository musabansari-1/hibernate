// Build a self-contained Windows folder. Packaging never registers integrations or publishes files.
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {cp,mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {resolve,join,relative} from 'node:path';
const root=resolve('.');
async function run(executable,args){await new Promise((resolveRun,reject)=>{const child=spawn(executable,args,{cwd:root,stdio:'inherit',windowsHide:true});child.once('error',reject);child.once('exit',code=>code===0?resolveRun():reject(new Error(`${executable} exited ${code}`)));});}
if(process.platform!=='win32')throw new Error('Build the Windows distribution on Windows.');
await run(process.execPath,['scripts/desktop.mjs','build','--no-bundle']);
await run('cargo',['build','--manifest-path','native/Cargo.toml','--release','--offline','--bin','hibernate-host']);
await run(process.execPath,['scripts/package-vscode.mjs']);
const {version}=JSON.parse(await readFile('package.json','utf8'));
const directory=resolve('dist',`ContextSleep-${version}-${new Date().toISOString().replace(/[:.]/g,'-')}`);await mkdir(directory,{recursive:true});
for(const [source,target] of [
 ['apps/desktop/src-tauri/target/release/project-hibernate.exe','ContextSleep.exe'],
 ['native/target/release/hibernate-host.exe','hibernate-host.exe'],
 ['extensions/chrome/dist','chrome-extension'],
 [`extensions/vscode/project-hibernate-${version}.vsix`,`contextsleep-${version}.vsix`],
 ['scripts/register-native-host.ps1','scripts/register-native-host.ps1'],
 ['scripts/setup-package.ps1','SETUP.ps1'],
 ['docs/SAFETY.md','SAFETY.md'],
 ['docs/ARCHITECTURE.md','ARCHITECTURE.md'],
 ['docs/GENERIC-APP-ARCHITECTURE.md','GENERIC-APP-ARCHITECTURE.md'],
 ['docs/RECONSTRUCTION-PLAN.md','RECONSTRUCTION-PLAN.md'],
 ['docs/GENERIC-APP-VALIDATION.md','GENERIC-APP-VALIDATION.md'],
 ['docs/PRODUCT-STATUS.md','PRODUCT-STATUS.md'],
 ['docs/VALIDATION.md','VALIDATION.md'],
 ['docs/TERMINAL-OWNERSHIP.md','TERMINAL-OWNERSHIP.md'],
 ['docs/VSCODE-MULTIWINDOW-OWNERSHIP.md','VSCODE-MULTIWINDOW-OWNERSHIP.md']
])await cp(resolve(source),join(directory,target),{recursive:true});
for(const name of ['PRODUCT-STATUS.md','VALIDATION.md']){const file=join(directory,name);await writeFile(file,(await readFile(file,'utf8')).replaceAll('](../README.md)','](START-HERE.md)').replaceAll('](../README.md#','](START-HERE.md#'));}
await writeFile(join(directory,'START-HERE.md'),`# ContextSleep ${version} — private beta

Run **ContextSleep.exe** on Windows 10/11 with WebView2 and Windows Terminal installed. Keep hibernate-host.exe beside it. This folder does not need Node, Rust or this source checkout to run.

ContextSleep is a local-first private beta for putting away supported development-workspace context and reconstructing it with verification. The packaged VS Code + Chrome + approved-service lifecycle has passed end to end. This build is unsigned; Explorer and arbitrary terminal windows are Context Restore only, ambiguous close targets stay open, and actual Windows reboot recovery is not yet validated. No adapter claims arbitrary app state or full process hibernation. Review SAFETY.md. Closing the dashboard hides it to the tray; use the tray's Quit action to exit.

## Fast app setup

The easiest path is inside ContextSleep: choose **Connect apps -> Set up apps automatically**. ContextSleep installs the packaged VS Code extension, prepares the stable local bridge, pre-registers Chrome for the bundled stable extension ID, opens chrome://extensions, and opens the exact extension folder.

Chrome still requires one browser-security confirmation: enable Developer mode, click **Load unpacked**, and select the folder ContextSleep opened. No extension-ID copy/paste and no manual Connect/refresh step are required; the extension reconnects automatically after Chrome loads it.

SETUP.ps1 remains an optional command-line fallback. It uses the same stable Chrome extension ID automatically:

\`\`\`powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\SETUP.ps1 -InstallVSCode -RegisterChrome -Launch
\`\`\`

Run SETUP.ps1 without switches to see the available fallback actions.

## Chrome integration

1. Prefer **Connect apps -> Set up apps automatically** in ContextSleep.
2. When Chrome opens chrome://extensions, enable Developer mode and click **Load unpacked**.
3. Select the chrome-extension folder ContextSleep opened. The local bridge is already registered for the bundled stable ID and the extension connects automatically.

Advanced/manual fallback only:

\`\`\`powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\register-native-host.ps1 -ExtensionId lilgdmjjanejoihlpegfklkfaofebfhn -HostPath .\\hibernate-host.exe
\`\`\`

The script registers only this host and extension for your account. It refuses to replace another application's registration. Existing snapshots remain in your local application data.

## VS Code integration

Open ContextSleep and use **Connect apps → Install VS Code integration**. ContextSleep installs contextsleep-${version}.vsix and prepares a stable local bridge under your Windows profile. Open or reload a trusted local repository and the extension auto-connects. Advanced users can still set hibernate.nativeHostPath manually to override the bridge path.

ContextSleep: Open Instrumented Project Terminal creates a no-profile PowerShell shell that reports its working directory. Existing arbitrary terminal commands are never replayed. Reviewed clean VS Code close can be verified when its workbench window is unambiguous and its GUI process exits. Multiple workbench windows sharing a process block reviewed closure.

## First project

Create a workspace, choose its folder, assign its app windows, then **Save this workspace**. Choose **Check sleep readiness**, inspect the saved state, select supported windows/services and confirm saved work. All selected parts are checked before the first close and rechecked before each close. A failed or unverified close stops later work. Unsupported contexts remain visible and may continue using resources. Before relying on cold restore for Chrome, open **Chrome profile binding** once and save that enrolled profile's directory (for example \`Default\`). With the VS Code extension installed and its native-host path configured, checkpoint restore can reopen the saved local workspace and wait for the same integration identity to reconnect. Restarting saved development services requires selecting the checkpoint service part and explicitly approving its saved direct Node/Python command.

Open Windows applications are discovered without per-app connection setup. Generic context preserves window/executable metadata and may relaunch an eligible captured GUI executable with no arguments. Generic contexts stay open during Sleep, do not reconstruct documents or unsaved state, and cannot make workspace readiness verified. App-specific launch metadata, protected/system apps and ambiguous routes remain context-only. Chrome/VS Code semantic support retains its existing approval and safety boundaries.

Upgrade the desktop, native host and integrations together. This build upgrades the database to schema 6 for local generic capture authority and launch-attempt journals. Older builds cannot open the upgraded database. Interrupted project operations are never replayed automatically; queued project closes are canceled at startup. Generic imports do not acquire launch authority.

Data is stored in %LOCALAPPDATA%\\ProjectHibernate. Semantic payloads and .hibernate backups use current-account DPAPI. IDs and resource metrics are not encrypted; this does not protect against programs running as your account. Keep the original account and integration identities when relying on saved contexts.

## Distribution integrity

SHA256SUMS.json records packaged file hashes. Hashes detect accidental changes; they do not replace publisher signing. This build has no installer or automatic updates.
`);
const hashes={};async function hashFiles(folder){for(const entry of await readdir(folder,{withFileTypes:true})){const path=join(folder,entry.name);if(entry.isDirectory())await hashFiles(path);else hashes[relative(directory,path).replaceAll('\\','/')]=createHash('sha256').update(await readFile(path)).digest('hex');}}
await hashFiles(directory);await writeFile(join(directory,'SHA256SUMS.json'),JSON.stringify(hashes,null,2));
await writeFile(resolve('dist/latest-package.json'),JSON.stringify({version,directory,createdAt:new Date().toISOString()},null,2));console.log(`Windows distribution: ${directory}`);
