import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createServer as netServer } from 'node:net';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';

const root = resolve('.validation/packaged-v1');
const run = join(root, 'run-' + Date.now());
const project = join(run, 'project');
const chromeProfile = join(run, 'chrome');
const vscodeUser = join(run, 'vscode-user');
const vscodeExtensions = join(run, 'vscode-extensions');
await mkdir(project, { recursive: true });
for (const name of ['alpha.txt', 'beta.txt']) await writeFile(join(project, name), `Saved ${name}\n`);
const serviceScript = join(project, 'v1-service.cjs');
await writeFile(serviceScript, `const http=require('node:http');\nconst port=Number(process.argv[2]);\nconst server=http.createServer((req,res)=>{res.statusCode=req.url==='/health'?200:404;res.end(req.url==='/health'?'ok':'missing');});\nserver.listen(port,'127.0.0.1');\nlet closing=false;function close(){if(closing)return;closing=true;server.close(()=>process.exit(0));setTimeout(()=>process.exit(2),5000).unref();}\nprocess.on('SIGBREAK',close);process.on('SIGINT',close);process.on('SIGTERM',close);\n`);

async function freePort() { return new Promise(r => { const s=netServer(); s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));}); }); }
const desktopPort=await freePort(), chromeDebugPort=await freePort(), servicePort=await freePort();
const fixtureServer=createServer((_,res)=>res.end('<title>Packaged V1 browser</title><p>fixture</p>'));
await new Promise(r=>fixtureServer.listen(0,'127.0.0.1',r));
const fixtureUrl=`http://127.0.0.1:${fixtureServer.address().port}/`;
const health=`http://127.0.0.1:${servicePort}/health`;

const packageDir=process.env.HIBERNATE_TEST_PACKAGE;
if(!packageDir) throw new Error('Set HIBERNATE_TEST_PACKAGE to the built ContextSleep distribution directory.');
const desktopExe=join(packageDir,'ContextSleep.exe');
const host=join(packageDir,'hibernate-host.exe');
const chromeExtension=process.env.HIBERNATE_TEST_CHROME_EXTENSION_DIR??resolve('extensions/chrome/dist');
const vscodeExtension=process.env.HIBERNATE_TEST_VSCODE_EXTENSION_DIR??resolve('extensions/vscode');
const codeExe=(process.env.HIBERNATE_TEST_VSCODE??await readFile('.validation/vscode-executable.txt','utf8')).trim();
const versions=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(v=>/^chromium-\d+$/.test(v)).sort().reverse();
const chromeExe=join(process.env.LOCALAPPDATA,'ms-playwright',versions[0],'chrome-win64','chrome.exe');
const vscodeReady=join(run,'vscode-ready.json');
const env={
 ...process.env,
 HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',
 HIBERNATE_DATA_DIR:join(run,'data'),
 HIBERNATE_TEST_HOST:host,
 HIBERNATE_TEST_FAILURE:join(run,'failure.txt'),
 HIBERNATE_TEST_READY:vscodeReady,
 HIBERNATE_PROJECT_PHASE:'capture',
 HIBERNATE_TEST_CHROME_EXTENSION:chromeExtension,
 HIBERNATE_TEST_CHROME_DEBUG_PORT:String(chromeDebugPort),
 HIBERNATE_TEST_VSCODE:codeExe,
 HIBERNATE_TEST_VSCODE_USER_DATA:vscodeUser,
 HIBERNATE_TEST_VSCODE_EXTENSIONS_DIR:vscodeExtensions,
 HIBERNATE_TEST_VSCODE_EXTENSION:vscodeExtension,
 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${desktopPort}`,
};
delete env.ELECTRON_RUN_AS_NODE;
const dashboard=async()=>JSON.parse((await promisify(execFile)(host,['--diagnose'],{env,windowsHide:true,maxBuffer:12_000_000})).stdout);
async function until(fn,label,timeout=120000){const end=Date.now()+timeout;let last;while(Date.now()<end){const failure=await readFile(env.HIBERNATE_TEST_FAILURE,'utf8').catch(()=>null);if(failure)throw new Error(failure);try{last=await fn();if(last)return last;}catch(e){last=e;}await new Promise(r=>setTimeout(r,300));}throw new Error(`Timed out: ${label}${last instanceof Error?` · ${last.message}`:''}`);}
async function endpointUp(){try{return (await fetch(health,{signal:AbortSignal.timeout(1200)})).status===200;}catch{return false;}}

let chrome, relaunchedChrome, code, desktop, webview, page;
async function launchDesktop(){desktop=spawn(desktopExe,[],{env,windowsHide:true,stdio:'ignore'});webview=await until(async()=>{try{return await chromium.connectOverCDP(`http://127.0.0.1:${desktopPort}`);}catch{return null;}},'packaged desktop');page=webview.contexts()[0].pages()[0];await page.locator('#apps-title').waitFor();}
async function stopDesktop(){if(webview)await webview.close().catch(()=>{});webview=null;page=null;if(desktop&&desktop.exitCode===null){const exited=new Promise(r=>desktop.once('exit',r));desktop.kill();await Promise.race([exited,new Promise(r=>setTimeout(r,5000))]);}desktop=null;}

const checks=[];
try{
 chrome=await chromium.launchPersistentContext(chromeProfile,{executablePath:chromeExe,headless:true,env,args:[`--disable-extensions-except=${chromeExtension}`,`--load-extension=${chromeExtension}`]});
 const sw=chrome.serviceWorkers()[0]??await chrome.waitForEvent('serviceworker');
 const chromeWindow=await sw.evaluate(url=>chrome.windows.create({url}),fixtureUrl);
 const chromeReport=await until(async()=>(await dashboard()).reports.find(r=>r.adapter==='chrome'&&Date.now()-r.at<5000),'Chrome report');

 code=spawn(codeExe,['--user-data-dir',vscodeUser,'--extensions-dir',vscodeExtensions,'--extensionDevelopmentPath='+vscodeExtension,'--extensionTestsPath='+resolve('tests/vscode-project-runner.cjs'),'--skip-welcome','--skip-release-notes','--disable-gpu','--disable-workspace-trust',project],{env,windowsHide:true,stdio:'ignore'});
 const marker=await until(async()=>JSON.parse(await readFile(vscodeReady,'utf8').catch(()=>'null')),'VS Code ready');
 const vscodeReport=await until(async()=>(await dashboard()).reports.find(r=>r.adapter==='vscode'&&r.body.instance===marker.instance&&Date.now()-r.at<15000),'VS Code report');
 const initialVscodeInstance=vscodeReport.body.instance;

 await launchDesktop();
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Workspace name').fill('Packaged V1 fixture');
 await page.getByLabel('Project folder (manual association)').fill(project);
 await page.getByRole('button',{name:'Save',exact:true}).click();await page.getByRole('button',{name:'Workspace',exact:true}).click();
 const projectId=(await dashboard()).workspace.id;

 await page.getByText('Chrome profile binding',{exact:true}).click();
 await page.getByLabel('Chrome profile folder').fill('Default');
 await page.getByLabel('chrome.exe path optional').fill(chromeExe);
 await page.getByLabel('User-data root portable Chrome only').fill(chromeProfile);
 await page.getByRole('button',{name:'Save profile binding',exact:true}).click();
 await until(async()=>(await dashboard()).chromeLaunches?.some(v=>v.client===chromeReport.client&&v.profileDirectory==='Default'),'Chrome cold-start binding');

 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Service name',{exact:true}).fill('V1 service');
 await page.getByLabel('Health endpoint',{exact:true}).fill(health);
 await page.getByRole('button',{name:'Add service',exact:true}).click();
 const service=await until(async()=>(await dashboard()).services.find(s=>s.name==='V1 service'),'service definition');
 await page.getByText('Startup command',{exact:true}).click();
 await page.getByLabel('Runtime executable').fill(process.execPath);
 await page.getByLabel('Literal arguments JSON array').fill(JSON.stringify([serviceScript,String(servicePort)]));
 await page.getByLabel('Working directory').fill(project);
 await page.getByRole('button',{name:'Save startup command',exact:true}).click();
 await until(async()=>(await dashboard()).services.find(s=>s.id===service.id)?.launch,'saved service launch');
 if (!await page.getByLabel('I reviewed this saved command and approve running it now.').isVisible()) await page.getByText('Startup command', { exact: true }).click();
  await page.getByLabel('I reviewed this saved command and approve running it now.').check();
 await page.getByRole('button',{name:'Start and verify',exact:true}).click();
 const initialService=await until(async()=>{const s=(await dashboard()).services.find(v=>v.id===service.id);return s?.launchRun?.verified===true?s:null;},'verified initial service');
 assert.equal(await endpointUp(),true);

 await page.getByRole('button',{name:'Workspace',exact:true}).click();const current=await dashboard();
 for(const report of [vscodeReport,chromeReport]){
  const fresh=current.reports.find(r=>r.client===report.client);
  const item=report.adapter==='chrome'?fresh.body.items.find(i=>i.title==='Packaged V1 browser'):fresh.body.items[0];
  assert.ok(item,`Missing selectable ${report.adapter} fixture item`);
  const selector=page.getByLabel(`Workspace for ${item.title}`);if(await selector.count())await selector.selectOption(projectId);
  await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel(`Select ${item.title}`,{exact:true}).check();
 }
 await page.getByRole('button',{name:'Save this workspace',exact:true}).last().click();
 const checkpoint=await until(async()=>{const c=(await dashboard()).checkpoints[0];return c?.state==='captured'?c:null;},'complete V1 checkpoint');
 assert.deepEqual(checkpoint.body.entries.map(e=>e.kind),['vscode','chrome']);
 assert.equal(checkpoint.body.services.length,1);

 const card=page.locator('.checkpoint').first();
 await card.getByRole('button', { name: 'Check sleep readiness',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'What can sleep right now?'});
 await dialog.getByRole('button',{name:'Select all safe',exact:true}).click();
 await dialog.getByRole('checkbox',{name:'I saved the work in every selected window and reviewed what can be reconstructed.',exact:true}).check();
 await dialog.getByRole('button', { name: /^Sleep \d+ windows?$/,exact:true}).click();
 const closed=await until(async()=>{const h=(await dashboard()).checkpoints[0]?.hibernations[0];return h?.state==='closed-selected'?h:null;},'combined hibernation',150000);
 assert.deepEqual(closed.body.steps.filter(s=>s.state==='closed').map(s=>s.kind),['vscode','chrome','services']);
 assert.equal(closed.body.steps.find(s=>s.kind==='vscode').result.adapterResult.ownerProcessExited,true);
 assert.equal(closed.body.steps.find(s=>s.kind==='services').result.services[0].result.verified,true);
 assert.equal(await endpointUp(),false);
 await until(()=>code.exitCode!==null||code.signalCode!==null,'VS Code process exit');
 assert.equal(await sw.evaluate(async id=>(await chrome.windows.getAll()).some(w=>w.id===id),chromeWindow.id),false);
 checks.push('One packaged checkpoint preflighted and closed VS Code, Chrome and the verified service in order with observed completion');

 await chrome.close();chrome=null;
 await until(async()=>(await dashboard()).reports.find(r=>r.client===chromeReport.client)?.at===0,'Chrome profile offline');
 await until(async()=>(await dashboard()).reports.find(r=>r.client===vscodeReport.client)?.at===0,'VS Code offline');
 await stopDesktop();
 assert.equal(await endpointUp(),false);
 await launchDesktop();
 assert.equal((await dashboard()).checkpoints[0].hibernations[0].state,'closed-selected');
 assert.equal(await endpointUp(),false);
 checks.push('Packaged desktop restart preserved the hibernated journal and did not replay apps or service launches');

 const restoreCard=page.locator('.checkpoint').first();
 for(const entry of checkpoint.body.entries)await restoreCard.getByRole('checkbox',{name:entry.label,exact:true}).check();
 await restoreCard.getByRole('checkbox',{name:/Restart saved services:/}).check();
 await restoreCard.getByLabel('I reviewed these saved commands and approve restarting them.').check();
 await restoreCard.getByRole('button', { name: /^Restore \d+ parts?$/,exact:true}).click();
 const restored=await until(async()=>(await dashboard()).checkpoints[0]?.runs.find(r=>r.state==='finished'),'complete packaged reconstruction',180000);
 assert.equal(restored.body.readiness.status,'ready',JSON.stringify(restored.body.readiness));
 assert.equal(restored.body.readiness.verified,true);
 for(const kind of ['vscode','chrome','services']){const step=restored.body.steps.find(s=>s.kind===kind);assert.equal(step.state,'observed',JSON.stringify(step));}
 assert.equal(restored.body.steps.find(s=>s.kind==='vscode').result.adapterResult.verified,true);
 assert.equal(restored.body.steps.find(s=>s.kind==='chrome').result.adapterResult.verified,true);
 assert.equal(restored.body.steps.find(s=>s.kind==='services').result.verified,true);
 assert.equal(restored.body.steps.find(s=>s.kind==='vscode').result.adapterResult.facts.find(f=>f.capability==='Saved text editors').observed,2);
 assert.equal(await endpointUp(),true);
 const restartedService=(await dashboard()).services.find(s=>s.id===service.id);assert.notEqual(restartedService.launchRun.identity.processStarted,initialService.launchRun.identity.processStarted);
 relaunchedChrome=await until(async()=>{try{return await chromium.connectOverCDP(`http://127.0.0.1:${chromeDebugPort}`);}catch{return null;}},'cold-launched Chrome debug connection');
 const fresh=await dashboard();
 assert.ok(fresh.reports.find(r=>r.client===chromeReport.client&&Date.now()-r.at<5000));
 const restoredVscode=fresh.reports.find(r=>r.client===vscodeReport.client&&Date.now()-r.at<15000);assert.ok(restoredVscode);assert.notEqual(restoredVscode.body.instance,initialVscodeInstance);
 checks.push('After product restart, restore cold-launched the enrolled Chrome profile and saved VS Code workspace, restarted the approved service, and overall readiness became ready');

 await page.reload();
 await page.locator('.checkpoint').first().getByText('Verified ready',{exact:true}).waitFor();
 assert.equal(await page.getByRole('alert').count(),0);
 checks.push('Ready evidence persisted across dashboard reload');
 const result={packageDir,checks,close:closed.body.steps.map(s=>({kind:s.kind,state:s.state})),readiness:restored.body.readiness,service:{initial:initialService.launchRun.identity,restarted:restartedService.launchRun.identity},limitations:['Actual Windows reboot not performed by this automated run','Explorer/terminal context intentionally excluded from this checkpoint because existing arbitrary shell state is not a V1 automatic-close capability']};
 await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
} finally {
 if(relaunchedChrome){await (await relaunchedChrome.newBrowserCDPSession()).send('Browser.close').catch(()=>{});await relaunchedChrome.close().catch(()=>{});}if(chrome)await chrome.close().catch(()=>{});await stopDesktop().catch(()=>{});if(code&&code.exitCode===null)code.kill();
 await promisify(execFile)('powershell.exe',['-NoProfile','-Command',"$svc=$env:HIBERNATE_SERVICE_FIXTURE;$vscode=$env:HIBERNATE_VSCODE_USER; Get-CimInstance Win32_Process | Where-Object { ($_.Name -eq 'node.exe' -and $_.CommandLine -and $_.CommandLine.Contains($svc)) -or ($_.Name -eq 'Code.exe' -and $_.CommandLine -and $_.CommandLine.Contains($vscode)) } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"] ,{env:{...process.env,HIBERNATE_SERVICE_FIXTURE:serviceScript,HIBERNATE_VSCODE_USER:vscodeUser},windowsHide:true}).catch(()=>{});
 await new Promise(r=>fixtureServer.close(r));
}
