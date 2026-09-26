// Real desktop/native IPC with isolated protocol fixtures. No user application is closed.
import {chromium} from '@playwright/test';
import {spawn,spawnSync} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createServer} from 'node:net';
import assert from 'node:assert/strict';
const root=resolve('.validation/project-discovery'),run=join(root,'run-'+Date.now()),project=join(run,'discovered-work');
await mkdir(join(project,'.git'),{recursive:true});await mkdir(join(project,'frontend'),{recursive:true});
await writeFile(join(project,'.git','HEAD'),'ref: refs/heads/discovery-fixture\n');
const port=await new Promise(r=>{const s=createServer();s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
const env={...process.env,HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',HIBERNATE_DATA_DIR:join(run,'data'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
const host=process.env.HIBERNATE_TEST_HOST??resolve('native/target/debug/hibernate-host.exe');
const reporter=spawn(host,[],{env,windowsHide:true,stdio:['pipe','pipe','pipe']});
let buffer=Buffer.alloc(0),requestId=0,focused=true;
const pending=new Map();const errors=[];
reporter.stdout.on('data',chunk=>{buffer=Buffer.concat([buffer,chunk]);while(buffer.length>=4&&buffer.length>=buffer.readUInt32LE(0)+4){const length=buffer.readUInt32LE(0),reply=JSON.parse(buffer.subarray(4,length+4));buffer=buffer.subarray(length+4);const task=pending.get(reply.requestId);if(task){pending.delete(reply.requestId);clearTimeout(task.timer);reply.ok?task.resolve(reply.value):task.reject(new Error(reply.error));}}});
function poll(){return new Promise((resolveReply,reject)=>{const id=++requestId;pending.set(id,{resolve:resolveReply,reject,timer:setTimeout(()=>{pending.delete(id);reject(new Error('Reporter timed out'));},15000)});const body={instance:'discovery-test-window',label:'Discovery protocol fixture',signals:[{source:'vscode',label:'Fixture workspace',path:pathToFileURL(project).href},{source:'terminal',label:'Fixture frontend cwd',path:pathToFileURL(join(project,'frontend')).href},{source:'browser',label:'Unowned localhost',url:'http://localhost:3000/'}],items:[{id:'workspace',title:'Discovery fixture workspace',focused,detail:'Protocol fixture; no application commands',compatibility:'Experimental'}]};const data=Buffer.from(JSON.stringify({requestId:id,op:'poll',adapter:'vscode',client:'discovery-fixture',body}));const header=Buffer.alloc(4);header.writeUInt32LE(data.length);reporter.stdin.write(Buffer.concat([header,data]));});}
function dashboard(){const r=spawnSync(host,['--diagnose'],{env,windowsHide:true,encoding:'utf8'});assert.equal(r.status,0,r.stderr);return JSON.parse(r.stdout);}
async function until(fn,label){for(let i=0;i<100;i++){const result=await fn();if(result)return result;await new Promise(r=>setTimeout(r,250));}throw new Error('Timed out: '+label);}
let desktop,webview,page,timer;const checks=[];
try{
 await poll();timer=setInterval(()=>void poll().catch(e=>errors.push(e.message)),2000);
 desktop=spawn(process.env.HIBERNATE_TEST_DESKTOP??resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe'),[],{env,windowsHide:true,stdio:'ignore'});
 webview=await until(async()=>{try{return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);}catch{return null;}},'desktop WebView2');
 page=webview.contexts()[0].pages()[0];page.on('pageerror',e=>errors.push(e.message));
 await page.getByRole('button',{name:'Settings',exact:true}).click();const candidate=page.locator('.candidate-list > li').filter({hasText:project});
 await candidate.getByRole('button',{name:'Add',exact:true}).waitFor();
 assert.equal(await candidate.count(),1);await candidate.getByText('Why this folder?',{exact:true}).click();await candidate.getByText(/^Fixture frontend cwd/).waitFor();
 const before=dashboard();assert.equal(before.projects.length,1);assert.equal(before.memberships.length,0);assert.equal(before.projects[0].lastObservedActive,null);
 checks.push('Automatic native discovery grouped two reported folders into one Git repository with visible evidence; no project or membership was silently created');
 await candidate.getByRole('button',{name:'Add',exact:true}).click();
 const selected=await until(()=>{const d=dashboard();return d.workspace.projectPath===project?d.workspace:null;},'discovered project created');
 await candidate.getByRole('button',{name:'Open',exact:true}).waitFor();
 assert.equal(dashboard().projects.filter(p=>p.projectPath===project).length,1);assert.equal(dashboard().memberships.length,0);
 await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByLabel('Workspace for Discovery fixture workspace',{exact:true}).selectOption(selected.id);
 const active=await until(()=>dashboard().projects.find(p=>p.id===selected.id)?.lastObservedActive,'assigned activity observation');
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.locator('.workspace-tile.is-active').getByText(/Active now|Last seen/).waitFor();
 checks.push('Accepting a suggestion created one durable project; foreground activity was recorded only after explicit window assignment');
 focused=false;await poll();const last=dashboard().projects.find(p=>p.id===selected.id).lastObservedActive;assert.ok(last>=active);
 await page.reload();await page.getByRole('button',{name:'Settings',exact:true}).click();await page.locator('.workspace-tile.is-active').getByText('discovered-work',{exact:true}).waitFor();await poll();assert.equal(dashboard().projects.find(p=>p.id===selected.id).lastObservedActive,last);
 checks.push('Project membership and last foreground observation survived reload; background reports did not advance activity');
 clearInterval(timer);timer=undefined;reporter.stdin.end();await until(()=>reporter.exitCode!==null,'reporter disconnect');
 await page.getByRole('button',{name:'Scan again',exact:true}).click();await until(async()=>await candidate.count()===0,'disconnected suggestions cleared');
 await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByText('Offline',{exact:true}).waitFor();
 assert.equal(dashboard().projects.find(p=>p.id===selected.id).lastObservedActive,last);assert.equal(dashboard().projects.length,2);
 checks.push('Disconnected folder reports stopped producing live suggestions while saved project identity and historical activity remained intact');
 await page.getByLabel('Appearance',{exact:true}).selectOption('light');await page.screenshot({path:join(root,'dashboard-light.png'),fullPage:true});
 assert.equal(await page.getByRole('alert').count(),0);assert.deepEqual(errors,[]);
 const result={checks,limitations:['Folder/focus reports are isolated protocol fixtures; this test does not assert new OS foreground observations','Activity gaps do not prove project inactivity; no reclaimable-memory estimate is generated','Actual adapter capture/restore behavior has separate integration tests']};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}catch(e){if(page)await page.screenshot({path:join(root,'failure.png'),fullPage:true}).catch(()=>{});throw e;}finally{if(timer)clearInterval(timer);if(webview)await webview.close();if(desktop&&desktop.exitCode===null)desktop.kill();if(reporter.exitCode===null)reporter.kill();for(const p of pending.values())clearTimeout(p.timer);}
