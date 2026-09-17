import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { createServer as netServer } from 'node:net';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';

const root = resolve('.validation/service-checkpoint-lifecycle');
const run = join(root, 'run-' + Date.now());
const project = join(run, 'project');
await mkdir(project, { recursive: true });
const serviceScript = join(project, 'fixture-service.cjs');
await writeFile(serviceScript, `const http=require('node:http');\nconst port=Number(process.argv[2]);\nconst server=http.createServer((req,res)=>{res.statusCode=req.url==='/health'?200:404;res.end(req.url==='/health'?'ok':'missing');});\nserver.listen(port,'127.0.0.1');\nlet closing=false;function close(){if(closing)return;closing=true;server.close(()=>process.exit(0));setTimeout(()=>process.exit(2),5000).unref();}\nprocess.on('SIGBREAK',close);process.on('SIGINT',close);process.on('SIGTERM',close);\n`);

async function freePort() {
  return new Promise(resolvePort => {
    const server = netServer();
    server.listen(0, '127.0.0.1', () => {
      const value = server.address().port;
      server.close(() => resolvePort(value));
    });
  });
}
const servicePort = await freePort();
const desktopPort = await freePort();
const health = `http://127.0.0.1:${servicePort}/health`;
const desktopExe = process.env.HIBERNATE_TEST_DESKTOP ?? resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe');
const host = process.env.HIBERNATE_TEST_HOST ?? resolve('native/target/debug/hibernate-host.exe');
const env = {
  ...process.env,
  HIBERNATE_DATA_DIR: join(run, 'data'),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${desktopPort}`,
};
const dashboard = async () => JSON.parse((await promisify(execFile)(host, ['--diagnose'], {
  env, windowsHide: true, maxBuffer: 8_000_000,
})).stdout);
async function until(fn, label, timeout = 90000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try { last = await fn(); if (last) return last; } catch (error) { last = error; }
    await new Promise(r => setTimeout(r, 300));
  }
  throw new Error(`Timed out: ${label}${last instanceof Error ? ` · ${last.message}` : ''}`);
}
async function endpointUp() {
  try { return (await fetch(health, { signal: AbortSignal.timeout(1200) })).status === 200; }
  catch { return false; }
}
let desktop, webview, page;
async function launchDesktop() {
  desktop = spawn(desktopExe, [], { env, windowsHide: true, stdio: 'ignore' });
  webview = await until(async () => {
    try { return await chromium.connectOverCDP(`http://127.0.0.1:${desktopPort}`); }
    catch { return null; }
  }, 'desktop');
  page = webview.contexts()[0].pages()[0];
  await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
}
async function stopDesktop() {
  if (webview) await webview.close().catch(() => {});
  webview = null; page = null;
  if (desktop && desktop.exitCode === null) {
    const exited = new Promise(r => desktop.once('exit', r));
    desktop.kill();
    await Promise.race([exited, new Promise(r => setTimeout(r, 5000))]);
  }
  desktop = null;
}

const checks = [];
try {
  await launchDesktop();
  await page.getByLabel('Workspace name').fill('Service lifecycle fixture');
  await page.getByLabel('Project folder (manual association)').fill(project);
  await page.getByRole('button', { name: 'Save', exact: true }).click();

  await page.getByLabel('Service name', { exact: true }).fill('Lifecycle service');
  await page.getByLabel('Health endpoint', { exact: true }).fill(health);
  await page.getByRole('button', { name: 'Add service', exact: true }).click();
  const service = await until(async () => (await dashboard()).services.find(s => s.name === 'Lifecycle service'), 'saved service');

  await page.getByText('Approved service startup', { exact: true }).click();
  await page.getByLabel('Runtime executable').fill(process.execPath);
  await page.getByLabel('Literal arguments (JSON array)').fill(JSON.stringify([serviceScript, String(servicePort)]));
  await page.getByLabel('Working directory').fill(project);
  await page.getByRole('button', { name: 'Save startup definition', exact: true }).click();
  await until(async () => (await dashboard()).services.find(s => s.id === service.id)?.launch, 'saved launch definition');

  await page.getByText('Approved service startup', { exact: true }).click();
  await page.getByLabel('I reviewed this saved command and approve running it now.').check();
  await page.getByRole('button', { name: 'Start approved service and verify', exact: true }).click();
  const running = await until(async () => {
    const value = (await dashboard()).services.find(s => s.id === service.id);
    return value?.launchRun?.state === 'verified' && value.launchRun.verified === true ? value : null;
  }, 'verified service launch');
  assert.equal(await endpointUp(), true);
  const firstIdentity = running.launchRun.identity;
  checks.push('Packaged desktop launched the explicitly approved Node runtime and verified unique listener ownership');

  await page.getByLabel('Include project Explorer folders and a new terminal definition').check();
  await page.getByRole('button', { name: 'Capture & review workspace sleep', exact: true }).click();
  const checkpoint = await until(async () => {
    const value = (await dashboard()).checkpoints[0];
    return value?.state === 'captured' ? value : null;
  }, 'service checkpoint capture');
  assert.equal(checkpoint.body.services.length, 1);
  assert.equal(checkpoint.body.services[0].launchRun.identity.pid, firstIdentity.pid);

  const card = page.locator('.checkpoint').first();
  await card.getByRole('button', { name: 'Review workspace sleep', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Review workspace sleep' });
  await dialog.getByRole('button', { name: 'Select supported windows', exact: true }).click();
  await dialog.getByRole('checkbox', { name: 'I have saved the work in every selected window and reviewed what can be reconstructed.', exact: true }).check();
  await dialog.getByRole('button', { name: 'Sleep selected windows', exact: true }).click();
  const hibernation = await until(async () => {
    const value = (await dashboard()).checkpoints[0]?.hibernations[0];
    return value?.state === 'closed-selected' ? value : null;
  }, 'graceful service hibernation', 120000);
  const serviceClose = hibernation.body.steps.find(s => s.kind === 'services');
  assert.equal(serviceClose.state, 'closed', JSON.stringify(serviceClose));
  assert.equal(serviceClose.result.closed, true, JSON.stringify(serviceClose));
  assert.equal(serviceClose.result.services[0].result.verified, true, JSON.stringify(serviceClose));
  await until(async () => !(await endpointUp()), 'service endpoint disappearance');
  const stopped = (await dashboard()).services.find(s => s.id === service.id);
  assert.equal(stopped.shutdownRun.verified, true);
  assert.equal(stopped.launchRun.state, 'stopped');
  checks.push('Deep Hibernate gracefully stopped only the checkpoint-owned verified service and observed both process exit and listener disappearance');

  await stopDesktop();
  await new Promise(r => setTimeout(r, 1000));
  assert.equal(await endpointUp(), false);
  await launchDesktop();
  const afterRestart = (await dashboard()).services.find(s => s.id === service.id);
  assert.equal(afterRestart.launchRun.state, 'stopped');
  assert.equal(await endpointUp(), false);
  checks.push('Desktop-process restart preserved the stopped service state and did not replay a launch automatically');

  const restoredCard = page.locator('.checkpoint').first();
  await restoredCard.getByRole('checkbox', { name: /Restart checkpoint services:/ }).check();
  await restoredCard.getByLabel('I reviewed these saved commands and approve restarting them from this checkpoint.').check();
  await restoredCard.getByRole('button', { name: 'Restore selected context', exact: true }).click();
  const restored = await until(async () => (await dashboard()).checkpoints[0]?.runs.find(r => r.state === 'finished'), 'checkpoint service restart', 120000);
  const serviceStep = restored.body.steps.find(s => s.kind === 'services');
  assert.equal(serviceStep.state, 'observed', JSON.stringify(serviceStep));
  assert.equal(serviceStep.result.verified, true, JSON.stringify(serviceStep));
  const restarted = await until(async () => {
    const value = (await dashboard()).services.find(s => s.id === service.id);
    return value?.launchRun?.verified === true && value.launchRun.identity?.pid !== firstIdentity.pid ? value : null;
  }, 'new verified service process');
  assert.equal(await endpointUp(), true);
  assert.notEqual(restarted.launchRun.identity.processStarted, firstIdentity.processStarted);
  assert.equal(restored.body.readiness.status, 'incomplete');
  assert.ok(restored.body.readiness.facts.some(f => f.kind === 'services' && f.status === 'verified'));
  assert.ok(restored.body.readiness.facts.some(f => f.status === 'not-restored'));
  checks.push('Explicit checkpoint approval restarted the saved command as a new verified process; readiness correctly stayed incomplete because local context was deliberately skipped');

  const result = { checks, firstIdentity, restartedIdentity: restarted.launchRun.identity, readiness: restored.body.readiness };
  await writeFile(join(root, 'results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally {
  await stopDesktop().catch(() => {});
  // Test-only cleanup: terminate only Node processes whose command line contains this unique fixture script path.
  await promisify(execFile)('powershell.exe', ['-NoProfile', '-Command',
    "$needle=$env:HIBERNATE_SERVICE_FIXTURE; Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($needle) } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"
  ], { env: { ...process.env, HIBERNATE_SERVICE_FIXTURE: serviceScript }, windowsHide: true }).catch(() => {});
  await readFile(join(root, 'results.json'), 'utf8').catch(() => null);
}
