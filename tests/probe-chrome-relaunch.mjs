import {chromium} from '@playwright/test';
import {execFile,spawn} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,readdir,rm} from 'node:fs/promises';
import {join,resolve} from 'node:path';
const run=resolve('.validation/chrome-relaunch-probe/run-'+Date.now());await mkdir(run,{recursive:true});
const env={...process.env,HIBERNATE_DATA_DIR:join(run,'data')};
const host=resolve('native/target/debug/hibernate-host.exe'),extension=resolve('extensions/chrome/dist');
const versions=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(v=>/^chromium-\d+$/.test(v)).sort().reverse();
const executable=join(process.env.LOCALAPPDATA,'ms-playwright',versions[0],'chrome-win64','chrome.exe'),profile=join(run,'profile');
const dash=async()=>JSON.parse((await promisify(execFile)(host,['--diagnose'],{env,windowsHide:true,maxBuffer:8_000_000})).stdout);
const until=async(fn,label,ms=30000)=>{const end=Date.now()+ms;while(Date.now()<end){const v=await fn();if(v)return v;await new Promise(r=>setTimeout(r,300));}throw Error('Timed out: '+label);};
let ctx,child;
try{
 ctx=await chromium.launchPersistentContext(profile,{executablePath:executable,headless:true,env,args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
 const sw=ctx.serviceWorkers()[0]??await ctx.waitForEvent('serviceworker');const id=new URL(sw.url()).hostname;
 const first=await until(async()=>(await dash()).reports.find(r=>r.adapter==='chrome'&&Date.now()-r.at<5000),'initial report');
 await ctx.close();ctx=null;
 await until(async()=>{const r=(await dash()).reports.find(r=>r.client===first.client);return r&&r.at===0;},'disconnect marker');
 child=spawn(executable,[`--user-data-dir=${profile}`,'--profile-directory=Default','--no-first-run',`--disable-extensions-except=${extension}`,`--load-extension=${extension}`,`chrome-extension://${id}/bootstrap.html`],{env,windowsHide:true,stdio:'ignore'});
 const second=await until(async()=>(await dash()).reports.find(r=>r.client===first.client&&Date.now()-r.at<5000),'automatic reconnect',40000);
 console.log(JSON.stringify({extensionId:id,client:first.client,reconnected:true,at:second.at},null,2));
}finally{if(ctx)await ctx.close().catch(()=>{});if(child&&!child.killed)child.kill();}
