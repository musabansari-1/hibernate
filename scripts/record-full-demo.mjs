import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createServer as netServer } from 'node:net';

const execFileAsync = promisify(execFile);
const latest = JSON.parse(await readFile(resolve('dist/latest-package.json'), 'utf8'));
const packageDir = latest.directory;
const desktopExe = join(packageDir, 'ContextSleep.exe');
const host = join(packageDir, 'hibernate-host.exe');
const chromeExtension = resolve('extensions/chrome/dist');
const vscodeExtension = resolve('extensions/vscode');
const codeExe = (await readFile('.validation/vscode-executable.txt', 'utf8')).trim();
const registrationScript = join(packageDir, 'scripts', 'register-native-host.ps1');
const stableExtensionId = 'lilgdmjjanejoihlpegfklkfaofebfhn';
const outputDir = resolve('demo-assets/contextsleep-full-demo');
const runDir = join(outputDir, 'run-' + Date.now());
const dataDir = join(runDir, 'data');
const project = join(runDir, 'ContextSleep Demo Workspace');
const chromeProfile = join(runDir, 'chrome-profile');
const vscodeUser = join(runDir, 'vscode-user');
const vscodeExtensions = join(runDir, 'vscode-extensions');
const vscodeReady = join(runDir, 'vscode-ready.json');
const failureFile = join(runDir, 'failure.txt');
const rawVideo = join(outputDir, 'contextsleep-full-demo-raw.mp4');
const finalVideo = join(outputDir, 'contextsleep-full-demo.mp4');
const captureDir = join(outputDir, 'live-frames');
const captureFps = 12;
await mkdir(project, { recursive: true });
await mkdir(outputDir, { recursive: true });
await writeFile(join(project, 'alpha.txt'), 'ContextSleep demo workspace\n\nAPI integration notes\n- Chrome research session\n- VS Code implementation\n- Safe local restore\n');
await writeFile(join(project, 'beta.txt'), 'Launch checklist\n\n[done] capture context\n[done] review safe close\n[todo] restore and verify\n');

async function freePort() {
  return new Promise(resolvePort => {
    const s = netServer();
    s.listen(0, '127.0.0.1', () => {
      const p = s.address().port;
      s.close(() => resolvePort(p));
    });
  });
}
const desktopPort = await freePort();
const chromeDebugPort = await freePort();
const env = {
  ...process.env,
  HIBERNATE_DATA_DIR: dataDir,
  HIBERNATE_TEST_HOST: host,
  HIBERNATE_TEST_FAILURE: failureFile,
  HIBERNATE_TEST_READY: vscodeReady,
  HIBERNATE_PROJECT_PHASE: 'capture',
  HIBERNATE_TEST_CHROME_EXTENSION: chromeExtension,
  HIBERNATE_TEST_CHROME_DEBUG_PORT: String(chromeDebugPort),
  HIBERNATE_TEST_VSCODE: codeExe,
  HIBERNATE_TEST_VSCODE_USER_DATA: vscodeUser,
  HIBERNATE_TEST_VSCODE_EXTENSIONS_DIR: vscodeExtensions,
  HIBERNATE_TEST_VSCODE_EXTENSION: vscodeExtension,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${desktopPort}`,
};
delete env.ELECTRON_RUN_AS_NODE;

async function dashboard() {
  const { stdout } = await execFileAsync(host, ['--diagnose'], { env, windowsHide: true, maxBuffer: 12_000_000 });
  return JSON.parse(stdout);
}
async function hostText(args) {
  const { stdout } = await execFileAsync(host, args, { env, windowsHide: true, maxBuffer: 8_000_000 });
  return stdout.trim();
}
async function until(fn, label, timeout = 120000, interval = 300) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    const failure = await readFile(failureFile, 'utf8').catch(() => null);
    if (failure) throw new Error(failure);
    try {
      last = await fn();
      if (last) return last;
    } catch (e) { last = e; }
    await new Promise(r => setTimeout(r, interval));
  }
  throw new Error(`Timed out: ${label}${last instanceof Error ? ` - ${last.message}` : ''}`);
}
async function waitJob(id, label) {
  return until(async () => {
    const d = await dashboard();
    const job = d.jobs.find(j => j.id === id);
    return job && ['done', 'failed', 'expired', 'interrupted'].includes(job.state) ? job : null;
  }, label);
}

const versions = (await readdir(join(process.env.LOCALAPPDATA, 'ms-playwright')))
  .filter(v => /^chromium-\d+$/.test(v)).sort().reverse();
if (!versions.length) throw new Error('Playwright Chromium is unavailable.');
const chromeExe = join(process.env.LOCALAPPDATA, 'ms-playwright', versions[0], 'chrome-win64', 'chrome.exe');

let chrome;
let relaunchedChrome;
let code;
let desktop;
let webview;
let page;
let captureActive = false;
let captureLoop;
let captureFrames = 0;
let registeredChromiumForDemo = false;
let failed = false;

async function ensureDemoNativeHostRegistration() {
  const check = String.raw`$k='HKCU:\Software\Chromium\NativeMessagingHosts\local.projecthibernate.bridge'; if(Test-Path $k){(Get-Item $k).GetValue('')}`;
  const existing = (await execFileAsync('powershell.exe', ['-NoProfile', '-Command', check], { windowsHide: true })).stdout.trim();
  if (existing) return;
  await execFileAsync('powershell.exe', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', registrationScript,
    '-ExtensionId', stableExtensionId, '-Browser', 'Chromium', '-HostPath', host,
  ], { windowsHide: true, maxBuffer: 8_000_000 });
  registeredChromiumForDemo = true;
}
async function removeDemoNativeHostRegistration() {
  if (!registeredChromiumForDemo) return;
  await execFileAsync('powershell.exe', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', registrationScript,
    '-ExtensionId', stableExtensionId, '-Browser', 'Chromium', '-HostPath', host, '-Unregister',
  ], { windowsHide: true, maxBuffer: 8_000_000 }).catch(() => {});
}

async function setWindowSize(pid) {
  const script = String.raw`
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class DemoWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
}
'@
$p=Get-Process -Id ${pid}
$end=(Get-Date).AddSeconds(20)
while($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $end){ Start-Sleep -Milliseconds 200; $p.Refresh() }
if($p.MainWindowHandle -eq 0){ throw 'ContextSleep window handle unavailable' }
[DemoWin]::ShowWindowAsync($p.MainWindowHandle,9) | Out-Null
[DemoWin]::SetWindowPos($p.MainWindowHandle,[IntPtr]::Zero,40,25,1280,760,0x0040) | Out-Null
[DemoWin]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 400
$r=New-Object DemoWin+RECT
if(-not [DemoWin]::GetWindowRect($p.MainWindowHandle,[ref]$r)){ throw 'Could not read ContextSleep window bounds' }
[pscustomobject]@{title=$p.MainWindowTitle;left=$r.Left;top=$r.Top;width=($r.Right-$r.Left);height=($r.Bottom-$r.Top)} | ConvertTo-Json -Compress
`;
  const { stdout } = await execFileAsync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', script], { windowsHide: true });
  return JSON.parse(stdout.trim().split(/\r?\n/).filter(Boolean).at(-1));
}

async function installOverlay() {
  await page.evaluate(() => {
    const style = document.createElement('style');
    style.textContent = `
      #demo-cursor{position:fixed;z-index:2147483646;width:30px;height:30px;border:2px solid #79f2b4;border-radius:50%;pointer-events:none;left:0;top:0;transform:translate(-80px,-80px);transition:transform .65s cubic-bezier(.22,1,.36,1),background .2s,box-shadow .2s;box-shadow:0 0 0 6px rgba(121,242,180,.11),0 8px 24px rgba(0,0,0,.35)}
      #demo-cursor.click{background:rgba(121,242,180,.38);box-shadow:0 0 0 15px rgba(121,242,180,.07),0 8px 24px rgba(0,0,0,.35)}
      #demo-caption{position:fixed;z-index:2147483645;left:50%;bottom:24px;transform:translateX(-50%) translateY(12px);min-width:520px;max-width:900px;padding:14px 20px;border-radius:14px;background:rgba(5,13,10,.96);border:1px solid rgba(121,242,180,.26);box-shadow:0 18px 55px rgba(0,0,0,.45);color:#eef8f2;font:600 15px/1.5 Inter,Segoe UI,sans-serif;text-align:center;opacity:0;transition:opacity .25s,transform .25s;pointer-events:none}
      #demo-caption.show{opacity:1;transform:translateX(-50%) translateY(0)}
      #demo-watermark{position:fixed;z-index:2147483644;top:16px;right:18px;padding:7px 10px;border-radius:999px;background:rgba(5,13,10,.86);border:1px solid rgba(121,242,180,.2);color:#a8bbb0;font:700 10px/1.2 Inter,Segoe UI,sans-serif;letter-spacing:.08em;text-transform:uppercase;pointer-events:none}
      #demo-intro,#demo-outro{position:fixed;z-index:2147483647;inset:0;display:grid;place-items:center;background:radial-gradient(circle at 50% 40%,rgba(55,170,111,.20),transparent 42%),#06100b;color:white;transition:opacity .7s;pointer-events:none}
      #demo-intro.hide{opacity:0}
      #demo-outro{display:none;background:radial-gradient(circle at 50% 42%,rgba(55,170,111,.24),transparent 42%),rgba(4,12,8,.985)}
      #demo-outro.show{display:grid}
      .demo-inner{text-align:center;max-width:820px;padding:44px}
      .demo-pill{display:inline-block;margin-bottom:20px;padding:8px 12px;border-radius:999px;border:1px solid rgba(121,242,180,.3);color:#79f2b4;font:700 10px/1 Inter,Segoe UI,sans-serif;letter-spacing:.12em;text-transform:uppercase}
      .demo-inner h1,.demo-inner h2{font:700 44px/1.08 Inter,Segoe UI,sans-serif;margin:0 0 14px;letter-spacing:-.04em}
      .demo-inner em{font-style:normal;color:#79f2b4}
      .demo-inner p{font:500 18px/1.55 Inter,Segoe UI,sans-serif;color:#adbbb2;margin:0 auto;max-width:720px}
      .demo-stack{display:flex;justify-content:center;gap:10px;flex-wrap:wrap;margin-top:24px}
      .demo-stack span{padding:8px 12px;border-radius:10px;border:1px solid rgba(255,255,255,.1);background:rgba(255,255,255,.045);font:650 12px/1.2 Inter,Segoe UI,sans-serif;color:#dce7e0}
      .demo-seal{width:82px;height:82px;margin:0 auto 20px;border:2px solid #79f2b4;border-radius:50%;display:grid;place-items:center;color:#79f2b4;font-size:40px;box-shadow:0 0 38px rgba(121,242,180,.2)}
    `;
    document.head.appendChild(style);
    document.body.insertAdjacentHTML('beforeend', `
      <div id="demo-cursor"></div>
      <div id="demo-caption"></div>
      <div id="demo-watermark">Real workflow · isolated demo data</div>
      <div id="demo-intro"><div class="demo-inner"><span class="demo-pill">Full end-to-end demo</span><h1>Put an entire workspace to sleep.<br/><em>Bring the context back.</em></h1><p>This real run uses a disposable Chrome research session and an isolated VS Code project. No personal tabs or files are touched.</p><div class="demo-stack"><span>Chrome · 12 tabs</span><span>VS Code · 2 editors</span><span>Fail-closed safety</span><span>Verified restore</span></div></div></div>
      <div id="demo-outro"><div class="demo-inner"><div class="demo-seal">✓</div><h2>Workspace restored.<br/><em>Verified READY.</em></h2><p>ContextSleep did not assume success because an app launched. It observed both saved capabilities after reconstruction.</p></div></div>
    `);
  });
}
async function caption(text, ms = 2600) {
  await page.evaluate(({ text }) => {
    const el = document.querySelector('#demo-caption');
    el.textContent = text;
    el.classList.add('show');
  }, { text });
  await page.waitForTimeout(ms);
}
async function hideCaption() {
  await page.evaluate(() => document.querySelector('#demo-caption')?.classList.remove('show'));
}
async function point(locator, text, settle = 1000) {
  await locator.scrollIntoViewIfNeeded();
  const box = await locator.boundingBox();
  if (!box) throw new Error(`Cannot locate demo target: ${text}`);
  await page.evaluate(({ x, y, text }) => {
    const cursor = document.querySelector('#demo-cursor');
    cursor.style.transform = `translate(${x - 15}px,${y - 15}px)`;
    const caption = document.querySelector('#demo-caption');
    caption.textContent = text;
    caption.classList.add('show');
  }, { x: box.x + box.width / 2, y: box.y + box.height / 2, text });
  await page.waitForTimeout(settle);
}
async function hold(locator, text, ms = 2800) {
  await point(locator, text);
  await page.waitForTimeout(ms);
}
async function guidedClick(locator, text, after = 1800) {
  await point(locator, text);
  await page.evaluate(() => document.querySelector('#demo-cursor')?.classList.add('click'));
  await page.waitForTimeout(180);
  await locator.click();
  await page.evaluate(() => document.querySelector('#demo-cursor')?.classList.remove('click'));
  await page.waitForTimeout(after);
}

async function startRecorder(_frame) {
  await rm(captureDir, {recursive: true, force: true});
  await mkdir(captureDir, {recursive: true});
  captureFrames = 0;
  captureActive = true;
  captureLoop = (async () => {
    const frameMs = 1000 / captureFps;
    let nextAt = Date.now();
    while (captureActive) {
      const file = join(captureDir, `frame-${String(++captureFrames).padStart(6, '0')}.jpg`);
      await page.screenshot({path: file, type: 'jpeg', quality: 90, animations: 'allow'});
      nextAt += frameMs;
      const delay = Math.max(0, nextAt - Date.now());
      if (delay) await new Promise(r => setTimeout(r, delay));
      else nextAt = Date.now();
    }
  })();
  await page.waitForTimeout(250);
}
async function stopRecorder() {
  if (!captureActive && !captureLoop) return;
  captureActive = false;
  if (captureLoop) await captureLoop.catch(() => {});
  captureLoop = null;
  if (!captureFrames) return;
  const ffmpegExe = 'C:\\ffmpeg\\bin\\ffmpeg.exe';
  await execFileAsync(ffmpegExe, [
    '-y', '-hide_banner', '-loglevel', 'warning',
    '-framerate', String(captureFps), '-i', join(captureDir, 'frame-%06d.jpg'),
    '-vf', 'pad=ceil(iw/2)*2:ceil(ih/2)*2',
    '-c:v', 'libx264', '-preset', 'veryfast', '-crf', '18', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', rawVideo,
  ], {windowsHide: true, maxBuffer: 12_000_000});
}

try {
  await ensureDemoNativeHostRegistration();

  chrome = await chromium.launchPersistentContext(chromeProfile, {
    executablePath: chromeExe,
    headless: true,
    env,
    args: [`--disable-extensions-except=${chromeExtension}`, `--load-extension=${chromeExtension}`],
  });
  const sw = chrome.serviceWorkers()[0] ?? await chrome.waitForEvent('serviceworker');
  let chromeReport = await until(async () => (await dashboard()).reports.find(r => r.adapter === 'chrome' && Date.now() - r.at < 15000), 'Chrome report');
  await new Promise(r => setTimeout(r, 1200));
  let fixtureJob;
  for (let attempt = 1; attempt <= 2; attempt++) {
    chromeReport = await until(async () => (await dashboard()).reports.find(r => r.adapter === 'chrome' && Date.now() - r.at < 6000), `fresh Chrome report attempt ${attempt}`);
    const fixtureId = await hostText(['--enqueue', chromeReport.client, 'fixture', '-']);
    fixtureJob = await waitJob(fixtureId, `12-tab Chrome fixture attempt ${attempt}`);
    if (fixtureJob.state === 'done') break;
    if (attempt === 2) throw new Error(`Chrome fixture failed twice: ${JSON.stringify(fixtureJob)}`);
    await new Promise(r => setTimeout(r, 1400));
  }
  const fixtureWindow = await until(async () => {
    const windows = await sw.evaluate(async () => chrome.windows.getAll({ populate: true }));
    const ranked = windows.map(w => ({window: w, fixtureTabs: w.tabs?.filter(t => t.url?.includes('/fixture.html?tab=')).length ?? 0}))
      .filter(v => v.fixtureTabs > 0)
      .sort((a, b) => b.fixtureTabs - a.fixtureTabs);
    return ranked[0]?.fixtureTabs >= 12 ? ranked[0].window : null;
  }, 'complete 12-tab Chrome fixture window');
  await sw.evaluate(async keep => {
    const windows = await chrome.windows.getAll({ populate: true });
    for (const w of windows) if (w.id !== keep) await chrome.windows.remove(w.id);
  }, fixtureWindow.id);
  await until(async () => {
    const report = (await dashboard()).reports.find(r => r.client === chromeReport.client);
    return report?.body.items.length === 1 && report.body.items[0].detail === '12 tabs' ? report : null;
  }, 'single Chrome demo window');

  code = spawn(codeExe, [
    '--user-data-dir', vscodeUser,
    '--extensions-dir', vscodeExtensions,
    '--extensionDevelopmentPath=' + vscodeExtension,
    '--extensionTestsPath=' + resolve('tests/vscode-project-runner.cjs'),
    '--skip-welcome', '--skip-release-notes', '--disable-gpu', '--disable-workspace-trust', project,
  ], { env, windowsHide: true, stdio: 'ignore' });
  const marker = await until(async () => JSON.parse(await readFile(vscodeReady, 'utf8').catch(() => 'null')), 'VS Code ready');
  await until(async () => (await dashboard()).reports.find(r => r.adapter === 'vscode' && r.body.instance === marker.instance && Date.now() - r.at < 15000), 'VS Code report');

  desktop = spawn(desktopExe, [], { env, windowsHide: false, stdio: 'ignore' });
  webview = await until(async () => {
    try { return await chromium.connectOverCDP(`http://127.0.0.1:${desktopPort}`); }
    catch { return null; }
  }, 'ContextSleep WebView2');
  const pages = webview.contexts()[0].pages();
  page = pages.find(p => !['about:blank', 'data:,'].includes(p.url())) ?? pages[0] ?? await webview.contexts()[0].waitForEvent('page');
  if (page.url() === 'about:blank') await page.waitForURL(url => url.href.startsWith('http://tauri.localhost/'), { timeout: 30000 });
  await page.waitForLoadState('domcontentloaded');
  await page.evaluate(() => {
    localStorage.setItem('hibernate-setup-hidden', '1');
    localStorage.setItem('hibernate-theme', 'dark');
  });
  await page.reload();

  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const activeCard = page.locator('.settings-card').filter({ hasText: 'Active workspace' }).first();
  await activeCard.getByLabel('Name').fill('Launch Demo Workspace');
  await activeCard.getByLabel('Folder').fill(project);
  await activeCard.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByRole('button', { name: 'Workspace', exact: true }).click();
  await page.getByRole('heading', { name: /Step 2:/ }).waitFor({ timeout: 30000 });

  const chromeCard = page.locator('.app-card').filter({ hasText: 'Chrome' }).first();
  await chromeCard.getByText('Chrome profile binding', { exact: true }).click();
  await chromeCard.getByLabel('Chrome profile folder').fill('Default');
  await chromeCard.getByLabel(/chrome.exe path/).fill(chromeExe);
  await chromeCard.getByLabel(/User-data root/).fill(chromeProfile);
  await chromeCard.getByRole('button', { name: 'Save profile binding', exact: true }).click();
  await chromeCard.getByText(/Saved for cold start:/).waitFor({ timeout: 15000 });
  await page.locator('.hero').scrollIntoViewIfNeeded();

  const captureFrame = await setWindowSize(desktop.pid);
  console.log('CAPTURE_FRAME', captureFrame);
  await installOverlay();
  await startRecorder(captureFrame);

  await page.waitForTimeout(4200);
  await page.evaluate(() => document.querySelector('#demo-intro')?.classList.add('hide'));
  await page.waitForTimeout(900);

  await caption('One-time integrations are already connected. ContextSleep can see the disposable Chrome and VS Code windows live.', 3300);

  const appsHeading = page.getByRole('heading', { name: 'Which windows belong to this workspace?', exact: true });
  await hold(appsHeading, 'Here is the daily starting point: choose which open windows belong to this workspace.', 2600);
  await hold(page.locator('.app-card').filter({ hasText: 'Chrome' }).first(), 'Chrome: a 12-tab synthetic research session. The tabs are safe demo pages with real memory allocated.', 3000);
  await hold(page.locator('.app-card').filter({ hasText: 'VS Code' }).first(), 'VS Code: an isolated local project with two saved editors. No unsaved work is present.', 3000);

  await guidedClick(page.getByRole('button', { name: 'Add detected windows', exact: true }), 'Step 1 — Add the two detected windows to this workspace.', 1900);
  await page.getByRole('heading', { name: /Step 3: Save this workspace/ }).waitFor({ timeout: 15000 });
  await guidedClick(page.getByRole('button', { name: 'Select saved workspace windows', exact: true }), 'Step 2 — Select the windows we want ContextSleep to preserve and rebuild.', 1800);

  const contextOption = page.getByText('Include folders and terminals', { exact: true });
  await hold(contextOption, 'Folders and terminal working directories can also be saved as restore-only context. Commands and history are never replayed.', 3000);
  const contextCheckbox = page.locator('.capture-bar input[type="checkbox"]').first();
  await point(contextCheckbox, 'Include the project folder and terminal working-directory context in this checkpoint.', 900);
  await contextCheckbox.check();
  await page.waitForTimeout(1200);

  await guidedClick(page.getByRole('button', { name: 'Save this workspace', exact: true }).last(), 'Step 3 — Save reconstructable context first. Nothing is closed yet.', 2200);
  const checkpoint = page.locator('.checkpoint').first();
  await checkpoint.waitFor({ timeout: 45000 });
  await hold(checkpoint.locator('.checkpoint-head'), 'A durable local checkpoint now contains both application contexts.', 3200);

  await guidedClick(checkpoint.getByRole('button', { name: 'Check sleep readiness', exact: true }), 'Step 4 — Review exactly what is safe to close before freeing memory.', 1200);
  const review = page.getByRole('dialog', { name: 'What can sleep right now?' });
  await review.waitFor({ timeout: 20000 });
  await hold(review.locator('.readiness-summary'), 'Both demo windows reached the safe-review stage. If ownership or state were ambiguous, ContextSleep would keep that item open.', 4200);
  const retained = review.locator('.review-group.is-retained');
  if (await retained.count()) {
    await hold(retained, 'Folder and terminal context is intentionally protected here: ContextSleep saves it for restore, but does not pretend it can safely close arbitrary shell or Explorer state.', 4200);
  }

  await guidedClick(review.getByRole('button', { name: 'Select all safe', exact: true }), 'Select only the targets ContextSleep proved eligible.', 1500);
  const consent = review.getByLabel('I saved the work in every selected window and reviewed what can be reconstructed.');
  await point(consent, 'The user explicitly confirms the work is saved before any close is allowed.', 1100);
  await consent.check();
  await page.waitForTimeout(1700);

  await guidedClick(review.getByRole('button', { name: /Sleep 2 windows/ }), 'Now sleep the two reviewed windows. ContextSleep rechecks each one immediately before closing.', 700);
  await caption('Closing in order… VS Code ownership is verified natively, then the Chrome window is closed and observed to disappear.', 3500);
  await page.getByText('Workspace asleep', { exact: true }).waitFor({ timeout: 120000 });
  await until(() => code.exitCode !== null || code.signalCode !== null, 'isolated VS Code process exit', 60000);
  await caption('Workspace asleep. The applications are gone, but the checkpoint remains on this PC.', 4200);

  await chrome.close();
  chrome = null;
  await until(async () => (await dashboard()).reports.find(r => r.client === chromeReport.client)?.at === 0, 'Chrome profile offline', 30000);

  const restoreButton = checkpoint.getByRole('button', { name: /^Restore \d+ parts?$/ }).first();
  await guidedClick(restoreButton, 'Step 5 — Bring the workspace back from the saved checkpoint.', 900);
  await caption('ContextSleep cold-launches the enrolled Chrome profile and reopens the saved VS Code project.', 3500);
  await caption('It waits for the original integrations to reconnect, reconstructs the saved tabs/editors, and checks the evidence.', 3800);

  const ready = checkpoint.locator('.ready-banner');
  await ready.waitFor({ timeout: 180000 });
  // Restored Chrome/VS Code may briefly take foreground. Bring ContextSleep back for the proof screen.
  await setWindowSize(desktop.pid);
  await page.waitForTimeout(1200);
  await ready.scrollIntoViewIfNeeded();
  await hold(ready, 'Step 6 — READY is shown only after both saved capabilities are observed again.', 6000);
  await hold(ready.locator('.ready-facts'), 'Chrome’s tab organization and the VS Code editor context were verified after reconstruction.', 5000);

  const finalState = await dashboard();
  const readiness = finalState.checkpoints[0]?.runs[0]?.body?.readiness;
  if (!readiness?.verified) throw new Error(`Demo finished without verified READY: ${JSON.stringify(readiness)}`);
  const kinds = new Set(readiness.facts.filter(f => f.status === 'verified').map(f => f.kind));
  if (!kinds.has('chrome') || !kinds.has('vscode')) throw new Error(`Expected Chrome + VS Code verification, got: ${JSON.stringify(readiness.facts)}`);

  await hideCaption();
  await page.evaluate(() => document.querySelector('#demo-outro')?.classList.add('show'));
  await page.waitForTimeout(5000);
  await stopRecorder();

  const ffmpegExe = 'C:\\ffmpeg\\bin\\ffmpeg.exe';
  await execFileAsync(ffmpegExe, [
    '-y', '-hide_banner', '-loglevel', 'warning', '-i', rawVideo,
    '-vf', 'scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:color=0x06100b',
    '-r', '30', '-c:v', 'libx264', '-preset', 'medium', '-crf', '18', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', finalVideo,
  ], { windowsHide: true, maxBuffer: 12_000_000 });

  relaunchedChrome = await until(async () => {
    try { return await chromium.connectOverCDP(`http://127.0.0.1:${chromeDebugPort}`); }
    catch { return null; }
  }, 'restored Chrome debug connection', 30000);

  await writeFile(join(outputDir, 'result.json'), JSON.stringify({
    packageDir,
    output: finalVideo,
    synthetic: true,
    apps: ['Chrome - 12 tabs', 'VS Code - 2 saved editors'],
    readiness,
    createdAt: new Date().toISOString(),
  }, null, 2));
  console.log(JSON.stringify({ ok: true, finalVideo, readiness }, null, 2));
} catch (error) {
  failed = true;
  console.error(error?.stack || error);
  throw error;
} finally {
  await stopRecorder().catch(() => {});
  if (relaunchedChrome) await relaunchedChrome.close().catch(() => {});
  if (chrome) await chrome.close().catch(() => {});
  if (webview) await webview.close().catch(() => {});
  if (desktop && desktop.exitCode === null) desktop.kill();
  if (code && code.exitCode === null) code.kill();
  await removeDemoNativeHostRegistration().catch(() => {});
  await execFileAsync('powershell.exe', ['-NoProfile', '-Command',
    "$needle=$env:DEMO_VSCODE_USER; Get-CimInstance Win32_Process -Filter \"Name='Code.exe'\" | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($needle) } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"
  ], { env: { ...process.env, DEMO_VSCODE_USER: vscodeUser }, windowsHide: true }).catch(() => {});
  if (failed) console.error(`Full-demo artifacts kept at ${outputDir}`);
}
