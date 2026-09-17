// Real Chrome API adapter test. Does not register or substitute for the native messaging bridge.
import { chromium } from '@playwright/test';
import { build } from 'esbuild';
import { mkdir, cp, readFile, writeFile, readdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/adapter');const extension=join(root,'extension');await mkdir(extension,{recursive:true});
await cp('extensions/chrome/public',extension,{recursive:true});
await build({entryPoints:['extensions/chrome/src/adapter.ts'],outfile:join(extension,'test-adapter.js'),bundle:true,format:'esm'});
await build({entryPoints:['extensions/chrome/src/fixture.ts','extensions/chrome/src/placeholder.ts'],outdir:extension,bundle:true,format:'esm'});
await writeFile(join(extension,'test-worker.js'),"import {adapter,fixture,identity} from './test-adapter.js'; globalThis.testAdapter={adapter,fixture,identity};");
const manifest=JSON.parse(await readFile(join(extension,'manifest.json'),'utf8'));manifest.name='Hibernate isolated adapter test';manifest.permissions=manifest.permissions.filter(p=>!['nativeMessaging','alarms'].includes(p));manifest.background.service_worker='test-worker.js';await writeFile(join(extension,'manifest.json'),JSON.stringify(manifest));
const candidates=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',candidates[0],'chrome-win64','chrome.exe');
const profileDir=join(root,'profile-'+Date.now());
const launch=()=>chromium.launchPersistentContext(profileDir,{executablePath,channel:'chromium',headless:true,args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
let context=await launch();const getWorker=async()=>context.serviceWorkers()[0]??context.waitForEvent('serviceworker');let sw=await getWorker();
async function until(fn,label){for(let i=0;i<100;i++){if(await fn())return;await new Promise(r=>setTimeout(r,100));}throw new Error('Timed out: '+label);}
const checks=[];
try{
  await sw.evaluate(()=>globalThis.testAdapter.fixture());
  const windowId=await sw.evaluate(async()=>{const w=(await chrome.windows.getAll({populate:true})).find(w=>w.tabs?.some(t=>t.url?.includes('fixture.html?tab=')));for(let i=13;i<=50;i++)await chrome.tabs.create({windowId:w.id,url:chrome.runtime.getURL('fixture.html?tab='+i),active:false});const tabs=await chrome.tabs.query({windowId:w.id});await chrome.tabs.update(tabs[17].id,{active:true});const g=await chrome.tabs.group({tabIds:tabs.slice(20,30).map(t=>t.id)});await chrome.tabGroups.update(g,{title:'Second group',color:'blue',collapsed:true});return w.id;});
  await until(()=>sw.evaluate(async id=>(await chrome.tabs.query({windowId:id})).every(t=>t.status==='complete'),windowId),'50 fixture pages load');
  const state=await sw.evaluate(id=>globalThis.testAdapter.adapter.capture(String(id)),windowId);assert.equal(state.tabs.length,50);
  assert.equal((await sw.evaluate(s=>globalThis.testAdapter.adapter.safety(s),state)).safe,true);
  await sw.evaluate(async s=>{await chrome.storage.local.set({'adapter-test-snapshot':s});await globalThis.testAdapter.adapter.close(s);},state);
  assert.equal(await sw.evaluate(async id=>(await chrome.windows.getAll()).some(w=>w.id===id),windowId),false);checks.push('50 real tabs captured; two groups; safe close verified');
  await context.close();context=await launch();sw=await getWorker();
  const restored=await sw.evaluate(async()=>{await globalThis.testAdapter.identity();const s=(await chrome.storage.local.get('adapter-test-snapshot'))['adapter-test-snapshot'];const receipt=await globalThis.testAdapter.adapter.restore(s,crypto.randomUUID());return {receipt,result:await globalThis.testAdapter.adapter.verify(s,receipt)};});
  assert.equal(restored.result.verified,true,JSON.stringify(restored.result));checks.push('Browser restart; 50/50 tab order, active tab, pin and groups verified');
  let tabs=await sw.evaluate(id=>chrome.tabs.query({windowId:id}),restored.receipt.windowId);
  assert.equal(tabs.filter(t=>t.url?.includes('placeholder.html#')).length,49);assert.equal(tabs.filter(t=>t.url?.includes('fixture.html?tab=')).length,1);checks.push('49 background destinations deferred; one active destination navigated');
  const id=tabs.find(t=>t.url?.includes('placeholder.html#')).id;await sw.evaluate(id=>chrome.tabs.update(id,{active:true}),id);await until(()=>sw.evaluate(async id=>(await chrome.tabs.get(id)).url?.includes('fixture.html?tab='),id),'lazy activation');checks.push('Tab activation wakes original destination');
  // A page need not have dirty flags to be unsafe. No request to close an ordinary page is allowed.
  const changed=structuredClone(state);changed.tabs[0].url='https://example.com';assert.equal((await sw.evaluate(s=>globalThis.testAdapter.adapter.safety(s),changed)).safe,false);checks.push('Ordinary page conservatively blocked');
  const result={browser:executablePath,scope:'Chrome adapter only; native messaging and desktop orchestration are not covered',checks};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}finally{await context.close();}
