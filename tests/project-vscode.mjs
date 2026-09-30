// A real project sequence must continue past verified VS Code closure to Chrome.
import {chromium} from '@playwright/test';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,readFile,writeFile,readdir} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createServer as netServer} from 'node:net';
import {resolve,join} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/project-vscode'),run=join(root,'run-'+Date.now()),project=join(run,'project');
await mkdir(project,{recursive:true});for(const name of ['alpha.txt','beta.txt'])await writeFile(join(project,name),`Saved fixture ${name}\n`);
const server=createServer((req,res)=>res.end('<title>Mixed project browser</title><p>Saved local fixture</p>'));await new Promise(r=>server.listen(0,'127.0.0.1',r));const url=`http://127.0.0.1:${server.address().port}/`;
const port=await new Promise(r=>{const s=netServer();s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
const desktopExe=process.env.HIBERNATE_TEST_DESKTOP??resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe');
const host=process.env.HIBERNATE_TEST_HOST??resolve('native/target/debug/hibernate-host.exe');
const codeExe=(process.env.HIBERNATE_TEST_VSCODE??await readFile('.validation/vscode-executable.txt','utf8')).trim();
const env={...process.env,HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',HIBERNATE_DATA_DIR:join(run,'data'),HIBERNATE_TEST_HOST:host,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`,HIBERNATE_TEST_FAILURE:join(run,'failure.txt')};delete env.ELECTRON_RUN_AS_NODE;
const dashboard=async()=>JSON.parse((await promisify(execFile)(host,['--diagnose'],{env,windowsHide:true,maxBuffer:8_000_000})).stdout);
async function until(fn,label,timeout=65000){const end=Date.now()+timeout;while(Date.now()<end){const failure=await readFile(env.HIBERNATE_TEST_FAILURE,'utf8').catch(()=>null);if(failure)throw new Error(failure);const value=await fn();if(value)return value;await new Promise(r=>setTimeout(r,300));}throw new Error('Timed out: '+label);}
let chrome,code,desktop,webview,page;const errors=[];
async function startCode(phase){
 const ready=join(run,phase+'-ready.json');
 code=spawn(codeExe,['--user-data-dir',join(run,'user'),'--extensions-dir',join(run,'extensions'),'--extensionDevelopmentPath='+(process.env.HIBERNATE_TEST_VSCODE_EXTENSION_DIR??resolve('extensions/vscode')),'--extensionTestsPath='+resolve('tests/vscode-project-runner.cjs'),'--skip-welcome','--skip-release-notes','--disable-gpu','--disable-workspace-trust',project],{env:{...env,HIBERNATE_PROJECT_PHASE:phase,HIBERNATE_TEST_READY:ready},windowsHide:true,stdio:'ignore'});
 const marker=await until(async()=>JSON.parse(await readFile(ready,'utf8').catch(()=>'null')),'VS Code ready');
 return until(async()=>(await dashboard()).reports.find(r=>r.adapter==='vscode'&&r.body.instance===marker.instance&&Date.now()-r.at<15000),'fresh VS Code report');
}
try{
 const versions=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
 const executablePath=join(process.env.LOCALAPPDATA,'ms-playwright',versions[0],'chrome-win64','chrome.exe'),extension=(process.env.HIBERNATE_TEST_CHROME_EXTENSION_DIR??resolve('extensions/chrome/dist'));
 chrome=await chromium.launchPersistentContext(join(run,'chrome'),{executablePath,env,headless:true,channel:'chromium',args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
 const sw=chrome.serviceWorkers()[0]??await chrome.waitForEvent('serviceworker');const window=await sw.evaluate(url=>chrome.windows.create({url}),url);
 await startCode('capture');
 desktop=spawn(desktopExe,[],{env,windowsHide:true,stdio:'ignore'});
 webview=await until(async()=>{try{return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);}catch{return null;}},'desktop');page=webview.contexts()[0].pages()[0];page.on('pageerror',e=>errors.push(e.message));
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Workspace name').fill('Mixed project fixture');await page.getByLabel('Project folder (manual association)').fill(project);await page.getByRole('button',{name:'Save',exact:true}).click();await page.getByRole('button',{name:'Workspace',exact:true}).click();
 // Selection order puts VS Code first, so Chrome can only close if its native verification completes.
 await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel('Select project',{exact:true}).check();await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel('Select Mixed project browser',{exact:true}).check();await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel('Include folders and terminals').check();
 await page.getByRole('button',{name:'Save this workspace',exact:true}).click();
 const checkpoint=await until(async()=>{const c=(await dashboard()).checkpoints[0];return c&&['captured','partial'].includes(c.state)?c:null;},'project capture');assert.equal(checkpoint.state,'captured',JSON.stringify(checkpoint.body.captures));assert.equal(checkpoint.body.entries[0].kind,'vscode');
 const card=page.locator('.checkpoint').first();await card.getByRole('button', { name: 'Check sleep readiness',exact:true}).click();const dialog=page.getByRole('dialog',{name:'What can sleep right now?'});await dialog.waitFor();
 await dialog.getByRole('button',{name:'Select all safe',exact:true}).click();await dialog.getByRole('checkbox',{name:'I saved the work in every selected window and reviewed what can be reconstructed.',exact:true}).check();await dialog.getByRole('button', { name: /^Sleep \d+ windows?$/,exact:true}).click();
 const close=await until(async()=>{const h=(await dashboard()).checkpoints[0]?.hibernations[0];return h&&['closed-selected','partial','blocked'].includes(h.state)?h:null;},'mixed project close');
 assert.equal(close.state,'closed-selected',JSON.stringify(close.body));assert.deepEqual(close.body.steps.filter(s=>s.state==='closed').map(s=>s.kind),['vscode','chrome']);assert.ok(close.body.steps.some(s=>s.kind==='terminal'&&s.state==='retained'));
 const vscodeClose=close.body.steps.find(s=>s.kind==='vscode');assert.equal(vscodeClose.result.adapterResult.ownerProcessExited,true);assert.equal(vscodeClose.result.adapterResult.closed,true);
 assert.equal(await sw.evaluate(async id=>(await chrome.windows.getAll()).some(w=>w.id===id),window.id),false);
 await until(()=>code.exitCode!==null||code.signalCode!==null,'isolated VS Code exit');
 await startCode('restore');
 for(const entry of checkpoint.body.entries.filter(e=>['vscode','chrome'].includes(e.kind)))await card.getByRole('checkbox',{name:entry.label,exact:true}).check();
 await card.getByRole('button', { name: /^Restore \d+ parts?$/,exact:true}).click();
 const restored=await until(async()=>(await dashboard()).checkpoints[0]?.runs.find(r=>r.state==='finished'),'mixed project reconstruction');
 for(const step of restored.body.steps.filter(s=>['vscode','chrome'].includes(s.kind))){assert.equal(step.state,'observed',JSON.stringify(step));assert.equal(step.result.adapterResult.verified,true,JSON.stringify(step));}
 assert.equal(restored.body.steps.find(s=>s.kind==='vscode').result.adapterResult.facts.find(f=>f.capability==='Saved text editors').observed,2);
 await page.reload();await page.locator('.checkpoint').first().getByText('Selected windows closed',{exact:true}).waitFor();assert.equal(await page.getByRole('alert').count(),0);assert.deepEqual(errors,[]);
 await page.locator('.checkpoint').first().screenshot({path:join(root,'checkpoint.png')});
 const result={desktop:desktopExe,host,checks:['One project review preflighted VS Code and Chrome','Native VS Code window disappearance and GUI-process exit verified; coordinator continued to Chrome','Both selected windows closed; unsupported terminal context retained','Reconnected original VS Code workspace and reconstructed a missing editor; 2/2 editors and 1/1 Chrome tabs verified','Close and reconstruction observations persisted across dashboard reload'],close:vscodeClose.result.adapterResult,restore:restored.body.steps.filter(s=>['vscode','chrome'].includes(s.kind)).map(s=>({kind:s.kind,result:s.result.adapterResult})),limitations:['Isolated saved local fixtures; no arbitrary dirty work, existing shells or services closed','Multiple VS Code workbench windows in one GUI process block reviewed closure','No Windows reboot or exclusive project RAM attribution']};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}catch(error){if(page)await page.screenshot({path:join(root,'failure.png'),fullPage:true}).catch(()=>{});throw error;}finally{if(webview)await webview.close();if(desktop&&desktop.exitCode===null)desktop.kill();if(code&&code.exitCode===null)code.kill();if(chrome)await chrome.close();await new Promise(r=>server.close(r));}
