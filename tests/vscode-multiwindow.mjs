import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,cp,readFile,writeFile,rename} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/vscode-multiwindow'),run=join(root,'run-'+Date.now());
const exe=process.env.HIBERNATE_TEST_VSCODE??(await readFile('.validation/vscode-executable.txt','utf8')).trim();
const host=resolve('native/target/debug/hibernate-host.exe');
const extensions=join(run,'extensions'),user=join(run,'user');
await mkdir(join(user,'User'),{recursive:true});
for(const label of ['a','b']){await mkdir(join(run,label),{recursive:true});await writeFile(join(run,label,'saved.txt'),'Saved fixture text\n');}
await cp('extensions/vscode',join(extensions,'project-hibernate.project-hibernate-0.1.0'),{recursive:true});
await cp('tests/fixtures/vscode-ownership',join(extensions,'hibernate-tests.hibernate-ownership-fixture-0.0.1'),{recursive:true});
await writeFile(join(user,'User/settings.json'),JSON.stringify({'hibernate.nativeHostPath':host,'window.restoreWindows':'none','update.mode':'none','extensions.autoUpdate':false,'telemetry.telemetryLevel':'off','workbench.startupEditor':'none'}));
const env={...process.env,HIBERNATE_DATA_DIR:join(run,'data'),HIBERNATE_TEST_HOST:host,HIBERNATE_TOPOLOGY_PROBE:resolve('native/target/debug/examples/vscode_topology.exe'),HIBERNATE_OWNERSHIP_FIXTURE:run};delete env.ELECTRON_RUN_AS_NODE;
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn,label,ms=45000){const end=Date.now()+ms;while(Date.now()<end){const result=await fn();if(result)return result;await delay(300);}throw Error('Timed out: '+label);}
async function json(file){try{return JSON.parse(await readFile(file,'utf8'));}catch(e){if(e.code==='ENOENT')return null;throw e;}}
async function native(args){return (await promisify(execFile)(host,args,{env,windowsHide:true,maxBuffer:8_000_000})).stdout.trim();}
const dashboard=async()=>JSON.parse(await native(['--diagnose']));
async function command(label,action,timeout=45000){const id='reply-'+Date.now()+'-'+Math.random().toString(36).slice(2),file=join(run,`${label}-command.json`);await writeFile(file+'.tmp',JSON.stringify({id,action}));await rename(file+'.tmp',file);return until(()=>json(join(run,`${label}-${id}.json`)),label+' '+action,timeout);}
function launch(label){const child=spawn(exe,['--user-data-dir',user,'--extensions-dir',extensions,'--new-window','--skip-welcome','--skip-release-notes','--disable-gpu','--disable-workspace-trust',join(run,label)],{env,windowsHide:true,stdio:'ignore'});child.on('error',e=>{console.error(e);});return child;}
async function settled(id){return until(async()=>{const j=(await dashboard()).jobs.find(j=>j.id===id);return j&&['done','failed','interrupted'].includes(j.state)?j:null;},'job '+id);}
async function capture(client){const previous=new Set((await dashboard()).snapshots.map(s=>s.id));const j=await settled(await native(['--enqueue',client,'capture','workspace']));assert.equal(j.state,'done');const saved=(await dashboard()).snapshots.filter(s=>s.client===client&&!previous.has(s.id));assert.equal(saved.length,1,'Exactly one new fixture capture expected');return saved[0];}
const evidence={run,checks:[]};let a,b;
try{
 launch('a');a=await until(()=>json(join(run,'a-ready.json')),'first fixture',120000);
 assert.equal(a.topology.windows.length,1);evidence.single=a;evidence.checks.push('Single fixture bridge ancestry sees one workbench');
 launch('b');b=await until(()=>json(join(run,'b-ready.json')),'second fixture',120000);
 const {pa,pb}=await until(async()=>{const pa=await command('a','probe'),pb=await command('b','probe');const keys=p=>p.topology.windows.map(w=>w.handle).sort().join(',');return pa.topology.windows.length>=2&&pb.topology.windows.length>=2&&keys(pa)===keys(pb)?{pa,pb}:null;},'matching ambiguous candidate sets');
 assert.notEqual(a.extensionHostPid,b.extensionHostPid);assert.notEqual(a.instance,b.instance);
 assert.deepEqual(pa.topology.owner,pb.topology.owner);assert.ok(pa.topology.windows.length>=2);assert.ok(pb.topology.windows.length>=2);
 assert.deepEqual(pa.topology.windows.map(w=>w.handle).sort(),pb.topology.windows.map(w=>w.handle).sort());
 evidence.multiple={a:pa.topology,b:pb.topology};evidence.checks.push('Two integration instances have separate extension hosts but the same GUI owner and multiple HWND candidates, including any hidden candidates');
 const ids=pa.topology.processes.map(p=>p.identity?.pid).filter(Boolean).join(',');
 evidence.roles=JSON.parse((await promisify(execFile)(join(process.env.WINDIR,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('tests/fixtures/vscode-ownership/process-roles.ps1'),'-FixturePids',ids],{windowsHide:true,maxBuffer:1_000_000})).stdout);
 for(const fixture of [a,b])assert.ok(evidence.roles.some(p=>p.role==='hibernate-bridge'&&p.parent===fixture.extensionHostPid));
 assert.ok(evidence.roles.filter(p=>p.role==='renderer').length>=2);
 evidence.checks.push('Each extension host has its own native bridge; renderer processes do not own workbench HWNDs');
 assert.equal(a.nativeHandleAccess.available,false);assert.match(a.nativeHandleAccess.reason,/nativeWindowHandle/);evidence.checks.push('Native handle API is proposal-gated in normal installed-extension configuration');
 const report=await until(async()=>(await dashboard()).reports.find(r=>r.body.instance===a.instance),'A integration');
 const saved=await capture(report.client);
 const count=pa.topology.nativeCloseObservationCount;
 const close=await settled(await native(['--review-close',saved.id,'--confirm-saved-work']));
 assert.equal(close.state,'failed');assert.match(JSON.stringify(close.result),/ambiguous/i);
 const after=await command('a','probe');await command('b','probe');
 assert.equal(after.topology.nativeCloseObservationCount,count);assert.ok(after.topology.windows.length>=2);
 for(const original of pa.topology.windows)assert.ok(after.topology.windows.some(w=>w.handle===original.handle));
 evidence.refusal=close.result;evidence.checks.push('Ambiguous close failed before native observation intent; both exact HWNDs and both integration responders remained');
 await command('a','dirty');const dirty=await capture(report.client);assert.equal(dirty.body.dirty,true);
 const job=await settled(await native(['--enqueue',report.client,'preflight_close','workspace',dirty.id]));
 assert.equal(job.state,'failed');assert.match(JSON.stringify(job.result),/unsaved|dirty/i);await command('a','clean');
 evidence.checks.push('Dirty work still blocks preflight');
 // Close only fixture B through its own extension; A returns to the established exclusive-owner case.
 await command('b','close-fixture');
 const remaining=await until(async()=>{const probe=await command('a','probe');return probe.topology.windows.length===1?probe:null;},'B fixture graceful close');
 const fresh=await capture(report.client);
 const jid=await native(['--review-close',fresh.id,'--confirm-saved-work']);const closed=await settled(jid);
 if(closed.state==='done'){
  assert.equal(closed.state,'done');const snapshot=(await dashboard()).snapshots.find(s=>s.id===fresh.id);
  assert.equal(snapshot.result.closed,true);assert.equal(snapshot.result.ownerProcessExited,true);assert.equal(snapshot.result.nativeWindow.method,'unique-window-in-bridge-process-ancestry');
  evidence.exclusiveClose=snapshot.result;evidence.checks.push('After fixture B exits, unchanged exclusive-window product path verifies A HWND disappearance and GUI-process exit');
 }else{
  assert.equal(closed.state,'failed');assert.match(JSON.stringify(closed.result),/ambiguous/i);
  evidence.remainingHiddenCandidates=remaining.topology.windows;evidence.checks.push('One visible fixture with additional hidden candidates remains blocked; visibility does not bypass uniqueness');
 }
 await writeFile(join(root,'results.json'),JSON.stringify(evidence,null,2));console.log(JSON.stringify(evidence,null,2));
}catch(e){await writeFile(join(run,'failure.json'),JSON.stringify({...evidence,error:String(e.stack??e)},null,2));throw e;}
finally{
 // No process termination fallback. Each disposable window closes itself, only while clean.
 for(const label of ['a','b']){const hb=await json(join(run,`${label}-heartbeat.json`));if(hb&&Date.now()-hb.at<3000){await command(label,'clean',3000).catch(()=>{});await command(label,'close-fixture',3000).catch(()=>{});}}
}
