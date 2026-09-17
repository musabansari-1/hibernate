import {chromium} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdir,readdir,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/reviewed-browser');await mkdir(root,{recursive:true});const run=join(root,'run-'+Date.now());
const env={...process.env,HIBERNATE_DATA_DIR:join(run,'data')};const host=resolve('native/target/debug/hibernate-host.exe');
const native=args=>{const r=spawnSync(host,args,{env,encoding:'utf8',windowsHide:true});if(r.status!==0)throw new Error(r.stderr||'Native command failed');return r.stdout.trim();};
const dashboard=()=>JSON.parse(native(['--diagnose']));
async function until(fn,label){for(let i=0;i<100;i++){const v=await fn();if(v)return v;await new Promise(r=>setTimeout(r,300));}throw new Error('Timed out: '+label);}
const finish=id=>until(()=>{const job=dashboard().jobs.find(j=>j.id===id);return job&&['done','failed'].includes(job.state)?job:null;},id);
const server=createServer((req,res)=>res.end(`<title>Reviewed project ${req.url}</title><input id="work">${req.url==='/dirty'?'<script>onbeforeunload=e=>{e.preventDefault();e.returnValue=""}</script>':''}`));await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin=`http://127.0.0.1:${server.address().port}`;
const candidates=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',candidates[0],'chrome-win64','chrome.exe');const extension=resolve('extensions/chrome/dist');
const context=await chromium.launchPersistentContext(join(run,'profile'),{executablePath,env,headless:true,channel:'chromium',args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});const checks=[];
try{
 const sw=context.serviceWorkers()[0]??await context.waitForEvent('serviceworker');const report=await until(()=>dashboard().reports.find(r=>r.adapter==='chrome'),'bridge');
 const create=async urls=>{const w=await sw.evaluate(url=>chrome.windows.create({url}),urls);await until(()=>sw.evaluate(async id=>(await chrome.tabs.query({windowId:id})).every(t=>t.status==='complete'),w.id),'page load');return w;};
 const capture=async w=>{const j=await finish(native(['--enqueue',report.client,'capture',String(w.id)]));assert.equal(j.state,'done');return dashboard().snapshots.find(s=>s.body.sourceWindow===w.id);};
 const clean=await create([origin+'/one',origin+'/two',origin+'/three']);const s=await capture(clean);
 assert.equal((await finish(native(['--review-close',s.id,'--confirm-saved-work']))).state,'done');assert.equal(await sw.evaluate(async id=>(await chrome.windows.getAll()).some(w=>w.id===id),clean.id),false);checks.push('Reviewed ordinary HTTP project window closed, with native resource observations recorded');
 assert.ok(dashboard().snapshots.find(v=>v.id===s.id).after);assert.equal((await finish(native(['--enqueue',report.client,'resume','-',s.id]))).state,'done');assert.equal(dashboard().snapshots.find(v=>v.id===s.id).state,'verified');checks.push('Ordinary project URLs restored and verified after reviewed close');
 const dirty=await create(origin+'/dirty');const page=await until(()=>context.pages().find(p=>p.url()===origin+'/dirty'),'dirty fixture page');await page.locator('#work').fill('Preserve this work');const captured=await capture(dirty);let sawDialog=false;page.on('dialog',async d=>{sawDialog=true;await d.dismiss();});
 const canceled=await finish(native(['--review-close',captured.id,'--confirm-saved-work']));assert.equal(canceled.state,'failed',JSON.stringify(canceled));assert.ok(sawDialog,JSON.stringify(canceled));assert.equal(await page.locator('#work').inputValue(),'Preserve this work');checks.push('Canceling Chrome beforeunload during a reviewed operation preserved unsaved input and reported failure');
 assert.throws(()=>native(['--review-close',captured.id,'--confirm-saved-work']),/already used|Capture/);checks.push('Consumed review approval cannot be replayed');await page.evaluate(()=>{onbeforeunload=null;});
 const changed=await create(origin+'/unchanged');const before=await capture(changed);await sw.evaluate(({id,url})=>chrome.tabs.create({windowId:id,url}),{id:changed.id,url:origin+'/new'});assert.equal((await finish(native(['--review-close',before.id,'--confirm-saved-work']))).state,'failed');assert.ok(await sw.evaluate(id=>chrome.windows.get(id),changed.id));checks.push('Live tab changes after capture prevented closure');
 const result={checks};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(result);
}finally{await context.close();await new Promise(r=>server.close(r));}
