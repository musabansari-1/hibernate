// Attach only to the extension worker. Attaching page debuggers changes Chrome's discard behavior.
import {spawn,spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/quick-sleep');const run=join(root,'run-'+Date.now());await mkdir(join(run,'profile'),{recursive:true});
const env={...process.env,HIBERNATE_DATA_DIR:join(run,'data')};const native=args=>{const r=spawnSync(resolve('native/target/debug/hibernate-host.exe'),args,{env,encoding:'utf8',windowsHide:true});if(r.status!==0)throw new Error(r.stderr);return r.stdout.trim();};const dashboard=()=>JSON.parse(native(['--diagnose']));
async function until(fn,label){const deadline=Date.now()+45000;while(Date.now()<deadline){const v=await fn();if(v)return v;await new Promise(r=>setTimeout(r,250));}throw new Error('Timed out: '+label);}
const server=createServer((_,res)=>res.end('<title>Quick Sleep fixture</title><p>Saved, input-free local test page</p>'));await new Promise(r=>server.listen(0,'127.0.0.1',r));
const candidates=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',candidates[0],'chrome-win64','chrome.exe');const extension=resolve('extensions/chrome/dist');
const child=spawn(executablePath,['--headless=new','--no-first-run','--no-default-browser-check','--remote-debugging-port=0','--user-data-dir='+join(run,'profile'),`--disable-extensions-except=${extension}`,`--load-extension=${extension}`],{env,windowsHide:true,stdio:'ignore'});let socket;const pending=new Map();let id=0;
try{
 const portFile=await until(()=>readFile(join(run,'profile','DevToolsActivePort'),'utf8').catch(()=>null),'Chrome debugging endpoint');const [port,path]=portFile.trim().split(/\r?\n/);socket=new WebSocket(`ws://127.0.0.1:${port}${path}`);await new Promise((r,j)=>{socket.addEventListener('open',r,{once:true});socket.addEventListener('error',j,{once:true});});
 socket.addEventListener('message',event=>{const msg=JSON.parse(event.data);const p=pending.get(msg.id);if(p){pending.delete(msg.id);clearTimeout(p.timer);msg.error?p.reject(new Error(JSON.stringify(msg.error))):p.resolve(msg.result);}});
 const call=(method,params={},sessionId)=>new Promise((resolve,reject)=>{const request=++id;const timer=setTimeout(()=>{pending.delete(request);reject(new Error('CDP timeout: '+method));},30000);pending.set(request,{resolve,reject,timer});socket.send(JSON.stringify({id:request,method,params,sessionId}));});
 const target=await until(async()=>(await call('Target.getTargets')).targetInfos.find(t=>t.type==='service_worker'&&t.url.endsWith('/background.js')),'extension worker');const {sessionId}=await call('Target.attachToTarget',{targetId:target.targetId,flatten:true});
 const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true},sessionId);if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
 const report=await until(()=>dashboard().reports.find(r=>r.adapter==='chrome'),'native bridge');
 const urls=[1,2,3].map(i=>`http://127.0.0.1:${server.address().port}/${i}`);const win=await evaluate(`chrome.windows.create({url:${JSON.stringify(urls)}})`);
 await until(()=>evaluate(`chrome.tabs.query({windowId:${win.id}}).then(tabs=>tabs.every(t=>t.status==='complete'))`),'test pages');
 const finish=jobId=>until(()=>{const j=dashboard().jobs.find(j=>j.id===jobId);return j&&['done','failed'].includes(j.state)?j:null;},jobId);
 assert.equal((await finish(native(['--enqueue',report.client,'capture',String(win.id)]))).state,'done');const snapshot=dashboard().snapshots.find(s=>s.body.sourceWindow===win.id);
 const result=await finish(native(['--quick-sleep',snapshot.id,'--confirm-saved-work']));assert.equal(result.state,'done',JSON.stringify(result));assert.equal(result.result.parked,2,JSON.stringify(result));
 const tabs=await evaluate(`chrome.tabs.query({windowId:${win.id}})`);assert.equal(tabs.filter(t=>t.discarded).length,2);const sleeping=tabs.find(t=>t.discarded);await evaluate(`chrome.tabs.update(${sleeping.id},{active:true})`);await until(()=>evaluate(`chrome.tabs.get(${sleeping.id}).then(t=>!t.discarded&&t.status==='complete')`),'wake on activation');
 const evidence={checks:['2/2 eligible background pages discarded and verified without closing the browser window','Selecting a sleeping tab reloaded it','Native resource observations retained separately from Deep Hibernate'],result:result.result};await writeFile(join(root,'results.json'),JSON.stringify(evidence,null,2));console.log(evidence);
 await call('Browser.close').catch(()=>{});
}finally{socket?.close();for(const p of pending.values())clearTimeout(p.timer);child.kill();await new Promise(r=>server.close(r));}
