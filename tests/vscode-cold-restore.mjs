import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer as netServer } from 'node:net';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';

const root = resolve('.validation/vscode-cold-restore');
const run = join(root, 'run-' + Date.now());
const project = join(run, 'project');
const userData = join(run, 'user');
const extensionsDir = join(run, 'extensions');
await mkdir(project, { recursive: true });
for (const name of ['alpha.txt', 'beta.txt'])
  await writeFile(join(project, name), `Saved fixture ${name}\n`);

const codeExe = (process.env.HIBERNATE_TEST_VSCODE ?? await readFile('.validation/vscode-executable.txt', 'utf8')).trim();
const vscodeExtension = process.env.HIBERNATE_TEST_VSCODE_EXTENSION_DIR ?? resolve('extensions/vscode');
const host = process.env.HIBERNATE_TEST_HOST ?? resolve('native/target/debug/hibernate-host.exe');
const desktopExe = process.env.HIBERNATE_TEST_DESKTOP ?? resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe');
const port = await new Promise(resolvePort => {
  const server = netServer();
  server.listen(0, '127.0.0.1', () => {
    const value = server.address().port;
    server.close(() => resolvePort(value));
  });
});
const ready = join(run, 'initial-ready.json');
const env = {
  ...process.env,
 HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',
  HIBERNATE_DATA_DIR: join(run, 'data'),
  HIBERNATE_TEST_HOST: host,
  HIBERNATE_TEST_FAILURE: join(run, 'failure.txt'),
  HIBERNATE_TEST_READY: ready,
  HIBERNATE_PROJECT_PHASE: 'capture',
  HIBERNATE_TEST_VSCODE: codeExe,
  HIBERNATE_TEST_VSCODE_USER_DATA: userData,
  HIBERNATE_TEST_VSCODE_EXTENSIONS_DIR: extensionsDir,
  HIBERNATE_TEST_VSCODE_EXTENSION: vscodeExtension,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
};
delete env.ELECTRON_RUN_AS_NODE;

const dashboard = async () => JSON.parse((await promisify(execFile)(host, ['--diagnose'], {
  env,
  windowsHide: true,
  maxBuffer: 8_000_000,
})).stdout);
async function until(fn, label, timeout = 90000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const failure = await readFile(env.HIBERNATE_TEST_FAILURE, 'utf8').catch(() => null);
    if (failure) throw new Error(failure);
    const value = await fn();
    if (value) return value;
    await new Promise(r => setTimeout(r, 300));
  }
  throw new Error('Timed out: ' + label);
}

let code, desktop, webview, page;
try {
  code = spawn(codeExe, [
    '--user-data-dir', userData,
    '--extensions-dir', extensionsDir,
    '--extensionDevelopmentPath=' + vscodeExtension,
    '--extensionTestsPath=' + resolve('tests/vscode-project-runner.cjs'),
    '--skip-welcome', '--skip-release-notes', '--disable-gpu', '--disable-workspace-trust',
    project,
  ], { env, windowsHide: true, stdio: 'ignore' });
  const marker = await until(async () => JSON.parse(await readFile(ready, 'utf8').catch(() => 'null')), 'initial VS Code ready');
  const report = await until(async () => (await dashboard()).reports.find(r => r.adapter === 'vscode' && r.body.instance === marker.instance && Date.now() - r.at < 15000), 'initial VS Code report');
  const initialInstance = report.body.instance;

  desktop = spawn(desktopExe, [], { env, windowsHide: true, stdio: 'ignore' });
  webview = await until(async () => {
    try { return await chromium.connectOverCDP(`http://127.0.0.1:${port}`); }
    catch { return null; }
  }, 'desktop');
  page = webview.contexts()[0].pages()[0];
  await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Workspace name').fill('Cold VS Code fixture');
  await page.getByLabel('Project folder (manual association)').fill(project);
  await page.getByRole('button', { name: 'Save', exact: true }).click();await page.getByRole('button',{name:'Workspace',exact:true}).click();

  const state = await dashboard();
  const item = state.reports.find(r => r.client === report.client).body.items[0];
  const projectId = state.workspace.id;
  const projectSelector = page.getByLabel(`Workspace for ${item.title}`);
  if (await projectSelector.count()) await projectSelector.selectOption(projectId);
  await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel(`Select ${item.title}`, { exact: true }).check();
  await page.getByRole('button', { name: 'Save this workspace', exact: true }).last().click();
  const checkpoint = await until(async () => {
    const value = (await dashboard()).checkpoints[0];
    return value?.state === 'captured' ? value : null;
  }, 'VS Code checkpoint capture');
  const entry = checkpoint.body.entries.find(e => e.kind === 'vscode');
  assert.ok(entry, JSON.stringify(checkpoint.body.entries));

  const card = page.locator('.checkpoint').first();
  await card.getByRole('button', { name: 'Check sleep readiness', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'What can sleep right now?' });
  await dialog.getByRole('button', { name: 'Select all safe', exact: true }).click();
  await dialog.getByRole('checkbox', { name: 'I saved the work in every selected window and reviewed what can be reconstructed.', exact: true }).check();
  await dialog.getByRole('button', { name: /^Sleep \d+ windows?$/, exact: true }).click();
  const closed = await until(async () => {
    const value = (await dashboard()).checkpoints[0]?.hibernations[0];
    return value?.state === 'closed-selected' ? value : null;
  }, 'reviewed VS Code close');
  const closeStep = closed.body.steps.find(s => s.kind === 'vscode');
  assert.equal(closeStep.result.adapterResult.ownerProcessExited, true, JSON.stringify(closeStep));
  await until(() => code.exitCode !== null || code.signalCode !== null, 'initial VS Code process exit');
  await until(async () => (await dashboard()).reports.find(r => r.client === report.client)?.at === 0, 'VS Code disconnect marker');

  await card.getByRole('checkbox', { name: entry.label, exact: true }).check();
  await card.getByRole('button', { name: /^Restore \d+ parts?$/, exact: true }).click();
  const restored = await until(async () => (await dashboard()).checkpoints[0]?.runs.find(r => r.state === 'finished'), 'VS Code cold restore');
  const step = restored.body.steps.find(s => s.kind === 'vscode');
  assert.equal(step.state, 'observed', JSON.stringify(step));
  assert.equal(step.result.adapterResult.verified, true, JSON.stringify(step));
  assert.equal(step.result.adapterResult.facts.find(f => f.capability === 'Saved text editors').observed, 2, JSON.stringify(step));
  assert.equal(restored.body.readiness.status, 'ready', JSON.stringify(restored.body.readiness));

  const reconnected = await until(async () => {
    const value = (await dashboard()).reports.find(r => r.client === report.client && Date.now() - r.at < 5000);
    return value?.body?.instance && value.body.instance !== initialInstance ? value : null;
  }, 'new VS Code session reconnect');
  const result = {
    checks: [
      'Reviewed close verified native VS Code GUI-process exit',
      'Checkpoint restore launched VS Code without manual reopening',
      'Original workspace integration identity reconnected in a new VS Code session',
      'Saved workspace and 2/2 text editors were verified after cold start',
      'Project readiness became ready',
    ],
    client: report.client,
    initialInstance,
    restoredInstance: reconnected.body.instance,
    readiness: restored.body.readiness,
  };
  await writeFile(join(root, 'results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally {
  if (webview) await webview.close().catch(() => {});
  if (desktop && desktop.exitCode === null) desktop.kill();
  if (code && code.exitCode === null) code.kill();
  // Only clean up Code.exe processes whose command line contains this test's unique user-data path.
  await promisify(execFile)('powershell.exe', ['-NoProfile', '-Command',
    "$needle=$env:HIBERNATE_CLEANUP_USER_DATA; Get-CimInstance Win32_Process -Filter \"Name='Code.exe'\" | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($needle) } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"
  ], { env: { ...process.env, HIBERNATE_CLEANUP_USER_DATA: userData }, windowsHide: true }).catch(() => {});
}
