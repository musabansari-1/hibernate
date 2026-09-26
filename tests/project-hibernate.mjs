// Real Chrome + Tauri orchestration; mutations affect only owned local test windows.
import {chromium} from '@playwright/test';
import {spawn,spawnSync} from 'node:child_process';
import {mkdir,readdir,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createServer as netServer} from 'node:net';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/project-hibernate'),run=join(root,'run-'+Date.now()),project=join(run,'project');
await mkdir(project,{recursive:true});
const server=createServer((req,res)=>res.end(`<title>Project close ${req.url.slice(1)}</title><p>Owned saved fixture page</p>`));await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin=`http://127.0.0.1:${server.address().port}`;
const port=await new Promise(r=>{const s=netServer();s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
const env={...process.env,HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',HIBERNATE_DATA_DIR:join(run,'data'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
const host=process.env.HIBERNATE_TEST_HOST??resolve('native/target/debug/hibernate-host.exe'),exe=process.env.HIBERNATE_TEST_DESKTOP??resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe');
function dashboard(){const p=spawnSync(host,['--diagnose'],{env,windowsHide:true,encoding:'utf8'});assert.equal(p.status,0,p.stderr);return JSON.parse(p.stdout);}
async function until(fn,label,timeout=65000){const deadline=Date.now()+timeout;while(Date.now()<deadline){const v=await fn();if(v)return v;await new Promise(r=>setTimeout(r,300));}throw new Error('Timed out: '+label);}
const versions=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',versions[0],'chrome-win64','chrome.exe'),extension=(process.env.HIBERNATE_TEST_CHROME_EXTENSION_DIR??resolve('extensions/chrome/dist'));
let chrome,desktop,webview,page,sw;const errors=[],checks=[];
async function capture(names){
 for(const checkbox of await page.locator('.app-card input[type=checkbox]').all())await checkbox.uncheck();
 await page.getByRole('button',{name:'Workspace',exact:true}).click();
 for(const name of names)await page.getByLabel(`Select Project close ${name}`,{exact:true}).check();
 await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel('Include folders and terminals').check();
 const previous=dashboard().checkpoints[0]?.id;
 await page.getByRole('button',{name:'Save this workspace',exact:true}).click();
 const checkpoint=await until(()=>{const c=dashboard().checkpoints[0];return c&&c.id!==previous&&['captured','partial'].includes(c.state)?c:null;},'new checkpoint');
 assert.equal(checkpoint.state,'captured',JSON.stringify(checkpoint.body.captures));return checkpoint;
}
async function approve(){
 await page.locator('.checkpoint').first().getByRole('button', { name: 'Check sleep readiness',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'What can sleep right now?'});await dialog.waitFor();
 assert.ok(await dialog.getByText('Protected - kept open',{exact:false}).count()>0);
 await dialog.getByRole('button',{name:'Select all safe',exact:true}).click();
 await dialog.getByRole('checkbox',{name:'I saved the work in every selected window and reviewed what can be reconstructed.',exact:true}).check();
 await dialog.getByRole('button', { name: /^Sleep \d+ windows?$/,exact:true}).click();
 return until(()=>{const h=dashboard().checkpoints[0]?.hibernations[0];return h&&['blocked','partial','closed-selected'].includes(h.state)?h:null;},'project close result');
}
async function present(ids){return sw.evaluate(async ids=>{const windows=await chrome.windows.getAll();return ids.map(id=>windows.some(w=>w.id===id));},ids);}
try{
 chrome=await chromium.launchPersistentContext(join(run,'profile'),{executablePath,env,headless:true,channel:'chromium',args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});sw=chrome.serviceWorkers()[0]??await chrome.waitForEvent('serviceworker');
 const ids=[];for(const name of ['a','b','c'])ids.push((await sw.evaluate(url=>chrome.windows.create({url}),origin+'/'+name)).id);
 await sw.evaluate(async id=>{const tabs=await chrome.tabs.query({windowId:id});const group=await chrome.tabs.group({tabIds:[tabs[0].id],createProperties:{windowId:id}});await chrome.tabGroups.update(group,{title:'Project research',color:'blue'});},ids[1]);
 await until(()=>dashboard().reports.some(r=>ids.every(id=>r.body.items.some(i=>i.id===String(id)))),'fixture reports');
 desktop=spawn(exe,[],{env,windowsHide:true,stdio:'ignore'});webview=await until(async()=>{try{return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);}catch{return null;}},'desktop');page=webview.contexts()[0].pages()[0];page.on('pageerror',e=>errors.push(e.message));
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Workspace name').fill('Project Hibernate fixture');await page.getByLabel('Project folder (manual association)').fill(project);await page.getByRole('button',{name:'Save',exact:true}).click();await page.getByRole('button',{name:'Workspace',exact:true}).click();
 await capture(['a','b','c']);
 await sw.evaluate(({id,url})=>chrome.tabs.create({windowId:id,url,active:false}),{id:ids[1],url:origin+'/changed-before-review'});
 const blocked=await approve();assert.equal(blocked.state,'blocked');assert.deepEqual(await present(ids),[true,true,true]);assert.ok(blocked.body.steps.some(s=>s.state==='blocked'));assert.ok(blocked.body.steps.some(s=>s.state==='not_attempted'));assert.equal(dashboard().jobs.some(j=>j.action==='close'),false);
 checks.push('A changed second window failed project preflight; all three windows remained open and no close job was dispatched');
 await capture(['a','b','c']);
 await sw.evaluate(({a,b,url})=>{const listener=id=>{if(id===a){chrome.windows.onRemoved.removeListener(listener);void chrome.tabs.create({windowId:b,url,active:false});}};chrome.windows.onRemoved.addListener(listener);},{a:ids[0],b:ids[1],url:origin+'/changed-during-close'});
 const partial=await approve();assert.equal(partial.state,'partial');assert.deepEqual(await present(ids),[false,true,true]);assert.equal(partial.body.steps.filter(s=>s.state==='closed').length,1);assert.equal(partial.body.steps.filter(s=>s.state==='unverified').length,1);assert.equal(partial.body.steps.filter(s=>s.state==='not_attempted').length,1);
 checks.push('A change after preflight stopped the sequence after one verified close; the changed window and later window remained open');
 await capture(['b','c']);const completed=await approve();assert.equal(completed.state,'closed-selected');assert.deepEqual(await present(ids),[false,false,false]);assert.equal(completed.body.steps.filter(s=>s.state==='closed').length,2);assert.ok(completed.body.steps.some(s=>s.state==='retained'&&s.kind==='terminal'));assert.ok(completed.body.before.totalPhysical>0);assert.ok(completed.body.after.totalPhysical>0);
 checks.push('One review closed both selected unchanged windows with disappearance verification, retained unsupported context, and recorded real resource observations');
 const point=dashboard().checkpoints[0];const card=page.locator('.checkpoint').first();
 for(const entry of point.body.entries.filter(e=>e.kind==='chrome'))await card.getByRole('checkbox',{name:entry.label,exact:true}).check();
 await card.getByRole('button', { name: /^Restore \d+ parts?$/,exact:true}).click();const restored=await until(()=>dashboard().checkpoints[0]?.runs.find(r=>r.state==='finished'),'restore');
 const verification=[];for(const step of restored.body.steps.filter(s=>s.kind==='chrome')){assert.equal(step.state,'observed');assert.equal(step.result.snapshotState,'verified');assert.equal(step.result.adapterResult.verified,true);verification.push({label:step.label,facts:step.result.adapterResult.facts});}
 checks.push('The hibernated windows reconstructed from their original checkpoint and verified saved tab organization');
 await page.reload();await page.locator('.checkpoint').first().getByText('Selected windows closed',{exact:true}).waitFor();assert.equal(dashboard().checkpoints[0].hibernations[0].id,completed.id);assert.equal(await page.getByRole('alert').count(),0);assert.deepEqual(errors,[]);
 await page.locator('.checkpoint').first().screenshot({path:join(root,'checkpoint.png')});
 const result={desktop:exe,host,checks,verification,memory:{before:completed.body.before,after:completed.body.after},limitations:['Owned local Chrome windows only; arbitrary web form state is not captured','Ambiguous or unobserved VS Code closes stop subsequent project closes','System memory observations are not exclusive project RAM; retained services/terminal contexts remain outside automatic close','Desktop reload is not a Windows reboot']};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}catch(e){if(page){console.error(await page.getByRole('alert').allTextContents().catch(()=>[]));await page.screenshot({path:join(root,'failure.png'),fullPage:true}).catch(()=>{});}throw e;}finally{if(webview)await webview.close();if(desktop&&desktop.exitCode===null)desktop.kill();if(chrome)await chrome.close();await new Promise(r=>server.close(r));}
