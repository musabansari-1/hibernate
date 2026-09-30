import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/vscode-close'),run=join(root,'run-'+Date.now());await mkdir(join(run,'project'),{recursive:true});
const exe=process.env.HIBERNATE_TEST_VSCODE??(await readFile('.validation/vscode-executable.txt','utf8')).trim();
const env={...process.env,HIBERNATE_DATA_DIR:join(run,'data'),HIBERNATE_TEST_HOST:process.env.HIBERNATE_TEST_HOST??resolve('native/target/debug/hibernate-host.exe'),HIBERNATE_TEST_MARKER:join(run,'close.json'),HIBERNATE_TEST_FAILURE:join(run,'failure.txt'),HIBERNATE_TEST_RESULT:join(root,'results.json')};delete env.ELECTRON_RUN_AS_NODE;
async function phase(value){
 const child=spawn(exe,['--user-data-dir',join(run,'user'),'--extensions-dir',join(run,'extensions'),'--extensionDevelopmentPath='+(process.env.HIBERNATE_TEST_VSCODE_EXTENSION_DIR??resolve('extensions/vscode')),'--extensionTestsPath='+resolve('tests/vscode-close-runner.cjs'),'--skip-welcome','--skip-release-notes','--disable-gpu','--disable-workspace-trust',join(run,'project')],{env:{...env,HIBERNATE_CLOSE_PHASE:value},windowsHide:true,stdio:['ignore','pipe','pipe']});
 let output='',timedOut=false;child.stdout.on('data',b=>{output+=b;});child.stderr.on('data',b=>{output+=b;});
 const timer=setTimeout(()=>{timedOut=true;child.kill();},90000);try{const code=await new Promise((r,j)=>{child.once('exit',r);child.once('error',j);});assert.equal(timedOut,false,'Isolated close/reopen test timed out');const failure=await readFile(env.HIBERNATE_TEST_FAILURE,'utf8').catch(()=>null);assert.equal(failure,null,failure??output);
 // VS Code's test launcher reports 1 when a reviewed window close ends an intentionally pending runner.
 if(value==='close')assert.ok([0,1].includes(code),output);else assert.equal(code,0,output);
 }finally{clearTimeout(timer);}
}
await phase('close');
const marker=JSON.parse(await readFile(env.HIBERNATE_TEST_MARKER,'utf8'));
const end=Date.now()+15000;let data;while(Date.now()<end){data=JSON.parse((await promisify(execFile)(env.HIBERNATE_TEST_HOST,['--diagnose'],{env,windowsHide:true,maxBuffer:8_000_000})).stdout);if(data.reports.find(r=>r.client===marker.client)?.at===0)break;await new Promise(r=>setTimeout(r,300));}
assert.equal(data.reports.find(r=>r.client===marker.client)?.at,0,'Native host must record the ended window connection');
const closed=data.snapshots.find(s=>s.id===marker.snapshotId);assert.equal(closed?.state,'closed',JSON.stringify(closed?.result));assert.equal(closed.result.closed,true);assert.equal(closed.result.facts[0].observed,1);assert.equal(closed.result.nativeWindow.method,'unique-window-in-bridge-process-ancestry');assert.ok(closed.result.nativeWindow.processStarted);assert.ok(closed.before.totalPhysical>0);assert.ok(closed.after.totalPhysical>0);
assert.equal(data.jobs.find(j=>j.id===marker.closeJob)?.state,'done');
assert.equal(closed.result.ownerProcessExited,true);
await writeFile(env.HIBERNATE_TEST_MARKER,JSON.stringify({...marker,closeObservation:closed.result}));
await phase('restore');console.log(await readFile(env.HIBERNATE_TEST_RESULT,'utf8'));
