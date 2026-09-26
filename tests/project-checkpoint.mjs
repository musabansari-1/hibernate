import {chromium} from '@playwright/test';
import {spawn,spawnSync} from 'node:child_process';
import {mkdir,readdir,writeFile,realpath} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createServer as netServer} from 'node:net';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/project-checkpoint'),run=join(root,'run-'+Date.now()),project=join(run,'project');
await mkdir(join(project,'.git'),{recursive:true});await writeFile(join(project,'.git','HEAD'),'ref: refs/heads/checkpoint-test\n');await writeFile(join(project,'.git','config'),'[remote "origin"]\nurl = https://github.com/fixture/checkpoint.git\n');
const requests=[];const server=createServer((req,res)=>{requests.push(req.url);res.end(req.url==='/health'?'ok':`<title>Checkpoint fixture ${req.url}</title><p>Isolated test page</p>`);});await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin=`http://127.0.0.1:${server.address().port}`;
const port=await new Promise(r=>{const s=netServer();s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
const env={...process.env,HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',HIBERNATE_DATA_DIR:join(run,'data'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
const host=process.env.HIBERNATE_TEST_HOST??resolve('native/target/debug/hibernate-host.exe'),exe=process.env.HIBERNATE_TEST_DESKTOP??resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe');
function dashboard(){const p=spawnSync(host,['--diagnose'],{env,windowsHide:true,encoding:'utf8'});assert.equal(p.status,0,p.stderr);return JSON.parse(p.stdout);}
async function until(fn,label,timeout=65000){const deadline=Date.now()+timeout;while(Date.now()<deadline){const v=await fn();if(v)return v;await new Promise(r=>setTimeout(r,350));}throw new Error('Timed out: '+label);}
const candidates=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',candidates[0],'chrome-win64','chrome.exe'),extension=(process.env.HIBERNATE_TEST_CHROME_EXTENSION_DIR??resolve('extensions/chrome/dist'));
let chrome,desktop,webview,page;
async function launchDesktop(){desktop=spawn(exe,[],{env,windowsHide:true,stdio:'ignore'});webview=await until(async()=>{try{return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);}catch{return null;}},'desktop');page=webview.contexts()[0].pages()[0];await page.locator('#apps-title').waitFor();}
async function stopDesktop(){if(webview)await webview.close();if(desktop&&desktop.exitCode===null){const exited=new Promise(r=>desktop.once('exit',r));desktop.kill();await exited;}}
const errors=[];const checks=[];
try{
 chrome=await chromium.launchPersistentContext(join(run,'profile'),{executablePath,env,headless:true,channel:'chromium',args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
 const sw=chrome.serviceWorkers()[0]??await chrome.waitForEvent('serviceworker');
 const source=await sw.evaluate(urls=>chrome.windows.create({url:urls}),[origin+'/one',origin+'/two',origin+'/three']);
 await until(()=>dashboard().reports.some(r=>r.body.items.some(i=>i.id===String(source.id))),'source report');
 await launchDesktop();page.on('pageerror',e=>errors.push(e.message));
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Workspace name').fill('Checkpoint project');await page.getByLabel('Project folder (manual association)').fill(project);await page.getByRole('button',{name:'Save',exact:true}).click();await page.getByRole('button',{name:'Workspace',exact:true}).click();
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Service name',{exact:true}).fill('Captured frontend');await page.getByLabel('Health endpoint',{exact:true}).fill(origin+'/health');await page.getByRole('button',{name:'Add service',exact:true}).click();
 await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.locator('.window-row').filter({hasText:'Checkpoint fixture'}).first().getByRole('checkbox').check();
 await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel('Include folders and terminals').check();await page.getByRole('button',{name:'Save this workspace'}).click();
 const point=await until(()=>dashboard().checkpoints.find(c=>c.state==='captured'),'checkpoint capture');
 assert.equal(point.body.captures.length,2);assert.equal(point.body.entries.filter(e=>e.kind==='chrome').length,1);assert.equal(point.body.entries.filter(e=>e.kind==='terminal').length,1);assert.equal(point.body.repository.branch,'checkpoint-test');
 const projectId=dashboard().workspace.id;assert.equal(dashboard().projects.find(p=>p.id===projectId).snapshotCount,2);checks.push('Actual desktop UI captured Chrome and local context into one durable project checkpoint');
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Service name',{exact:true}).fill('Added later');await page.getByLabel('Health endpoint',{exact:true}).fill(origin+'/later');await page.getByRole('button',{name:'Add service',exact:true}).click();
 await page.getByRole('button',{name:'New workspace',exact:true}).click();await page.getByLabel('New project name',{exact:true}).fill('Second project');await page.getByLabel('Project folder',{exact:true}).fill(project);await page.getByRole('button',{name:'Create',exact:true}).click();
 await until(()=>dashboard().workspace.name==='Second project','project switch');assert.equal(dashboard().checkpoints.length,0);assert.equal(dashboard().projects.find(p=>p.id===projectId).snapshotCount,2);
 await page.locator('.workspace-tile').filter({hasText:'Checkpoint project'}).click();await until(()=>dashboard().workspace.id===projectId,'switch back');checks.push('Project switching scopes checkpoints and preserves accurate per-project snapshot counts');
 await page.getByRole('button',{name:'Workspace',exact:true}).click();const card=page.locator('.checkpoint');for(const entry of point.body.entries)await card.getByRole('checkbox',{name:entry.label,exact:true}).setChecked(entry.kind==='chrome');await card.getByRole('button', { name: /^Restore \d+ parts?$/}).click();
 const restored=await until(()=>dashboard().checkpoints[0]?.runs.find(r=>r.state==='finished'),'project reconstruction');
 const browserStep=restored.body.steps.find(s=>s.kind==='chrome');assert.equal(browserStep.state,'observed',JSON.stringify(browserStep));assert.equal(browserStep.result.adapterResult.facts[0].observed,3);assert.equal(browserStep.result.adapterResult.facts[0].expected,3);assert.equal(browserStep.result.snapshotState,'verified');assert.equal(restored.body.steps.find(s=>s.kind==='terminal').state,'skipped');
 const serviceStep=restored.body.steps.find(s=>s.kind==='services');assert.equal(serviceStep.state,'skipped');assert.equal(restored.body.readiness.verified,false);checks.push('Selective restore retained unapproved service startup and terminal context; readiness stayed incomplete');
 await card.getByText('Chrome tab URLs, order, pins and active state',{exact:true}).waitFor();await page.screenshot({path:join(root,'dashboard.png'),fullPage:true});
 const windowsBefore=await sw.evaluate(async()=>(await chrome.windows.getAll()).length);await stopDesktop();await launchDesktop();page.on('pageerror',e=>errors.push(e.message));
 await page.locator('.checkpoint').getByText('Chrome tab URLs, order, pins and active state',{exact:true}).waitFor();assert.equal(await sw.evaluate(async()=>(await chrome.windows.getAll()).length),windowsBefore);assert.equal(dashboard().checkpoints[0].runs.length,1);checks.push('Desktop process restart preserved checkpoint results without replaying app launches');
 const duplicate=spawn(exe,[],{env,windowsHide:true,stdio:'ignore'});await until(()=>duplicate.exitCode!==null,'second instance exits',15000);assert.equal(desktop.exitCode,null);checks.push('A second desktop launch did not take over or reset the active reconstruction journal');
 assert.equal(await page.getByRole('alert').count(),0);assert.deepEqual(errors,[]);
 const result={checks,limitations:['Browser organization and HTTP response verified; page content, command replay and process ownership not claimed','Terminal was deliberately skipped; its working-directory handshake has a separate live integration test','Desktop restart is not a Windows reboot']};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}catch(e){if(page){console.error(await page.getByRole('alert').allTextContents().catch(()=>[]));await page.screenshot({path:join(root,'failure.png'),fullPage:true}).catch(()=>{});}throw e;}finally{await stopDesktop();if(chrome)await chrome.close();await new Promise(r=>server.close(r));}
