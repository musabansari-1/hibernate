import { chromium } from '@playwright/test';
import { spawnSync, execFile } from 'node:child_process';
import {promisify} from 'node:util';
import { mkdir, writeFile, readdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/browser');await mkdir(root,{recursive:true});
const runDir=join(root,'run-'+Date.now());
const env={...process.env,HIBERNATE_DATA_DIR:join(runDir,'data')};
const host=resolve('native/target/debug/hibernate-host.exe');
const extension=(process.env.HIBERNATE_TEST_CHROME_EXTENSION_DIR??resolve('extensions/chrome/dist'));
const candidates=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',candidates[0],'chrome-win64','chrome.exe');
const launch=()=>chromium.launchPersistentContext(join(runDir,'profile'),{executablePath,headless:true,channel:'chromium',env,args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
let context=await launch();
const worker=async()=>context.serviceWorkers()[0]??await context.waitForEvent('serviceworker');
let sw=await worker();const extensionId=new URL(sw.url()).hostname;
if(process.argv.includes('--probe')){const bridge=await sw.evaluate(async()=>{try{return await chrome.runtime.sendNativeMessage('local.projecthibernate.bridge',{op:'probe'});}catch(e){return String(e);}});console.log(JSON.stringify({extensionId,executablePath,bridge}));await context.close();process.exit(0);}
function native(args){const r=spawnSync(host,args,{env,encoding:'utf8',windowsHide:true});if(r.status!==0)throw new Error(r.stderr||`Native host exited ${r.status}`);return r.stdout.trim();}
const dashboard=()=>JSON.parse(native(['--diagnose']));
// Keep the event loop available: Playwright must service paused target attachments as tabs are created.
const asyncDashboard=async()=>JSON.parse((await promisify(execFile)(host,['--diagnose'],{env,windowsHide:true,maxBuffer:8_000_000})).stdout);
async function until(fn,label,timeout=60000){const start=Date.now();while(Date.now()-start<timeout){const value=await fn();if(value)return value;await new Promise(r=>setTimeout(r,500));}throw new Error(`Timed out: ${label}`);}
async function job(client,action,target='-',snapshot){const args=['--enqueue',client,action,target];if(snapshot)args.push(snapshot);const id=native(args);const record=await until(async()=>{const j=(await asyncDashboard()).jobs.find(j=>j.id===id);return j&&['done','failed','expired','interrupted'].includes(j.state)?j:null;},action);return record;}
const results={browser:executablePath,checks:[],memory:null};
try{
  const report=await until(()=>dashboard().reports.find(r=>r.adapter==='chrome'&&Date.now()-r.at<15000),'native bridge connection');
  const fixture=await job(report.client,'fixture');assert.equal(fixture.state,'done',JSON.stringify(fixture));
  const windowId=await sw.evaluate(async()=>{const windows=await chrome.windows.getAll({populate:true});return windows.find(w=>w.tabs?.some(t=>t.url?.includes('/fixture.html?tab=')))?.id;});assert.ok(windowId);
  await sw.evaluate(async id=>{
    const existing=await chrome.tabs.query({windowId:id});
    for(let i=existing.length+1;i<=50;i++)await chrome.tabs.create({windowId:id,url:chrome.runtime.getURL('fixture.html?tab='+i),active:false});
    const tabs=await chrome.tabs.query({windowId:id});await chrome.tabs.update(tabs[17].id,{active:true});
    const g=await chrome.tabs.group({tabIds:tabs.slice(20,30).map(t=>t.id)});await chrome.tabGroups.update(g,{title:'Second group',color:'blue',collapsed:true});
  },windowId);
  await until(()=>sw.evaluate(async id=>(await chrome.tabs.query({windowId:id})).every(t=>t.status==='complete'),windowId),'fixture load');
  const fixturePages=context.pages().filter(p=>p.url().includes('/fixture.html?tab='));
  const allocated=await Promise.all(fixturePages.map(p=>p.evaluate(()=>globalThis.hibernateFixture?.byteLength??0)));
  assert.equal(allocated.length,50);assert.ok(allocated.every(n=>n===8*1024*1024),'Every validation page must actually allocate its documented workload');
  results.fixtureAllocatedBytes=allocated.reduce((a,b)=>a+b,0);
  const hibernated=await job(report.client,'hibernate',String(windowId));assert.equal(hibernated.state,'done',JSON.stringify(hibernated));
  const snapshot=dashboard().snapshots.find(s=>s.client===report.client&&s.state==='closed');assert.ok(snapshot);assert.equal(snapshot.body.tabs.length,50);assert.ok(snapshot.after);assert.ok(snapshot.body.project.id);
  assert.equal(await sw.evaluate(async id=>(await chrome.windows.getAll()).some(w=>w.id===id),windowId),false);results.checks.push('50-tab window captured durably, safety checked and close verified');
  results.memory={before:snapshot.before,after:snapshot.after,availablePhysicalDelta:snapshot.after.availablePhysical-snapshot.before.availablePhysical,chromeWorkingSetDecrease:snapshot.before.chromeWorkingSet-snapshot.after.chromeWorkingSet};
  // Close/reopen the entire isolated browser and native host; never touch the user's browser profile.
  await context.close();context=await launch();sw=await worker();
  await until(()=>dashboard().reports.some(r=>r.client===report.client&&Date.now()-r.at<5000),'profile reconnect after restart');
  assert.ok(dashboard().snapshots.find(s=>s.id===snapshot.id));results.checks.push('Snapshot survived browser/native-host restart and profile identity persisted');
  const resumed=await job(report.client,'resume','-',snapshot.id);assert.equal(resumed.state,'done',JSON.stringify(resumed));
  const verified=dashboard().snapshots.find(s=>s.id===snapshot.id);assert.equal(verified.state,'verified',JSON.stringify(verified.result));results.checks.push('50/50 tab organization, active tab, pin and two groups verified');
  const tabs=await sw.evaluate(async id=>{const data=await chrome.storage.local.get('restore:'+id);return chrome.tabs.query({windowId:data['restore:'+id].windowId});},snapshot.id);
  assert.equal(tabs.filter(t=>t.url?.includes('/placeholder.html#')).length,49);assert.equal(tabs.filter(t=>t.url?.includes('/fixture.html?tab=')).length,1);results.checks.push('49 background destinations not navigated; one active destination loaded');
  const first=tabs.find(t=>t.url?.includes('/placeholder.html#'));await sw.evaluate(id=>chrome.tabs.update(id,{active:true}),first.id);
  await until(()=>sw.evaluate(async id=>(await chrome.tabs.get(id)).url?.includes('/fixture.html?tab='),first.id),'lazy tab activation');results.checks.push('Selecting a placeholder loaded its original URL');
  // A live local HTTP page must be blocked, with a durable capture retained and its window left open.
  const {createServer}=await import('node:http');const server=createServer((_,res)=>res.end('<title>Unsaved work safety test</title><input value="Do not close">'));
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  try{
    const unsafe=await sw.evaluate(url=>chrome.windows.create({url}),`http://127.0.0.1:${server.address().port}`);
    const blocked=await job(report.client,'hibernate',String(unsafe.id));assert.equal(blocked.state,'failed');assert.ok(await sw.evaluate(id=>chrome.windows.get(id),unsafe.id));results.checks.push('Ordinary page close blocked and window remained open');
  }finally{server.close();}
  await writeFile(join(root,'results.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));
}finally{await context.close();}
