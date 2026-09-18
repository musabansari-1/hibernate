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
 ['docs/PRODUCT-STATUS.md','PRODUCT-STATUS.md'],
 ['docs/VALIDATION.md','VALIDATION.md'],
 ['docs/TERMINAL-OWNERSHIP.md','TERMINAL-OWNERSHIP.md'],
 ['docs/VSCODE-MULTIWINDOW-OWNERSHIP.md','VSCODE-MULTIWINDOW-OWNERSHIP.md']
])await cp(resolve(source),join(directory,target),{recursive:true});
for(const name of ['PRODUCT-STATUS.md','VALIDATION.md']){const file=join(directory,name);await writeFile(file,(await readFile(file,'utf8')).replaceAll('](../README.md)','](START-HERE.md)').replaceAll('](../README.md#','](START-HERE.md#'));}
await writeFile(join(directory,'START-HERE.md'),`# ContextSleep ${version} — private beta

Run **ContextSleep.exe** on Windows 10/11 with WebView2 and Windows Terminal installed. Keep hibernate-host.exe beside it. This folder does not need Node, Rust or this source checkout to run.

ContextSleep is a local-first private beta for putting away supported development-workspace context and reconstructing it with verification. The packaged VS Code + Chrome + approved-service lifecycle has passed end to end. This build is unsigned; Explorer and arbitrary terminal windows are Context Restore only, ambiguous close targets stay open, and actual Windows reboot recovery is not yet validated. No adapter claims arbitrary app state or full process hibernation. Review SAFETY.md. Closing the dashboard hides it to the tray; use the tray's Quit action to exit.

## Optional setup helper

SETUP.ps1 makes no changes when run without switches. After loading the Chrome extension folder and copying its ID, you can explicitly ask it to install the packaged VSIX, register this package's native host, and launch the desktop:

\`\`\`powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\SETUP.ps1 -InstallVSCode -RegisterChrome -ChromeExtensionId YOUR_EXTENSION_ID -Launch
\`\`\`

Chrome still requires one manual browser-security step: load the prepared unpacked extension folder in chrome://extensions and copy its extension ID. The desktop app can prepare that folder and register the local bridge after you paste the ID. VS Code setup now installs the packaged VSIX and prepares a stable local bridge automatically; hibernate.nativeHostPath remains available as an advanced override. Run SETUP.ps1 without switches to see the command-line alternatives.

## Chrome integration

1. In chrome://extensions, enable Developer mode and load the chrome-extension folder.
2. Copy its 32-character extension ID.
3. From PowerShell in this folder, run the following with your extension ID:

\`\`\`powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\register-native-host.ps1 -ExtensionId YOUR_EXTENSION_ID -HostPath .\\hibernate-host.exe
\`\`\`

The script registers only this host and extension for your account. It refuses to replace another installation's registration. Use the same command with -Unregister to remove this registration before moving the folder. Existing snapshots remain in your local application data.

## VS Code integration

Open ContextSleep and use **Connect apps → Install VS Code integration**. ContextSleep installs contextsleep-${version}.vsix and prepares a stable local bridge under your Windows profile. Open or reload a trusted local repository and the extension auto-connects. Advanced users can still set hibernate.nativeHostPath manually to override the bridge path.

ContextSleep: Open Instrumented Project Terminal creates a no-profile PowerShell shell that reports its working directory. Existing arbitrary terminal commands are never replayed. Reviewed clean VS Code close can be verified when its workbench window is unambiguous and its GUI process exits. Multiple workbench windows sharing a process block reviewed closure.

## First project

Create a workspace, choose its folder, assign its app windows, then **Capture workspace context**. Choose **Review and sleep**, inspect the saved state, select supported windows/services and confirm saved work. All selected parts are checked before the first close and rechecked before each close. A failed or unverified close stops later work. Unsupported contexts remain visible and may continue using resources. Before relying on cold restore for Chrome, open **Chrome profile binding** once and save that enrolled profile's directory (for example \`Default\`). With the VS Code extension installed and its native-host path configured, checkpoint restore can reopen the saved local workspace and wait for the same integration identity to reconnect. Restarting saved development services requires selecting the checkpoint service part and explicitly approving its saved direct Node/Python command.

Upgrade the desktop, native host and integrations together. This build upgrades the database to schema 5 for durable native close observation. Older builds cannot open the upgraded database. Interrupted project operations are never replayed automatically; queued project closes are canceled at startup.

Data is stored in %LOCALAPPDATA%\\ProjectHibernate. Semantic payloads and .hibernate backups use current-account DPAPI. IDs and resource metrics are not encrypted; this does not protect against programs running as your account. Keep the original account and integration identities when relying on saved contexts.

## Distribution integrity

SHA256SUMS.json records packaged file hashes. Hashes detect accidental changes; they do not replace publisher signing. This build has no installer or automatic updates.
`);
const hashes={};async function hashFiles(folder){for(const entry of await readdir(folder,{withFileTypes:true})){const path=join(folder,entry.name);if(entry.isDirectory())await hashFiles(path);else hashes[relative(directory,path).replaceAll('\\','/')]=createHash('sha256').update(await readFile(path)).digest('hex');}}
await hashFiles(directory);await writeFile(join(directory,'SHA256SUMS.json'),JSON.stringify(hashes,null,2));
await writeFile(resolve('dist/latest-package.json'),JSON.stringify({version,directory,createdAt:new Date().toISOString()},null,2));console.log(`Windows distribution: ${directory}`);
