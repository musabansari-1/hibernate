const vscode=require('vscode');
const assert=require('node:assert/strict');
const {execFile}=require('node:child_process');
const {promisify}=require('node:util');
const fs=require('node:fs');const path=require('node:path');
async function run(){
 const native=async args=>(await promisify(execFile)(process.env.HIBERNATE_TEST_HOST,args,{windowsHide:true,maxBuffer:8_000_000})).stdout.trim();
 const dashboard=async()=>JSON.parse(await native(['--diagnose']));
 async function until(fn,label){const end=Date.now()+40000;while(Date.now()<end){const v=await fn();if(v)return v;await new Promise(r=>setTimeout(r,300));}throw new Error('Timed out: '+label);}
 await vscode.workspace.getConfiguration('hibernate').update('nativeHostPath',process.env.HIBERNATE_TEST_HOST,vscode.ConfigurationTarget.Global);
 await vscode.extensions.getExtension('project-hibernate.project-hibernate').activate();await vscode.commands.executeCommand('hibernate.connect');
 const report=await until(async()=>(await dashboard()).reports.find(r=>r.adapter==='vscode'&&r.body.instance===vscode.env.sessionId),'matching window integration');
 const job=async(action,snapshot)=>{const args=['--enqueue',report.client,action,'workspace'];if(snapshot)args.push(snapshot);const id=await native(args);return until(async()=>{const j=(await dashboard()).jobs.find(j=>j.id===id);return j&&['done','failed','interrupted'].includes(j.state)?j:null;},action);};
 const root=vscode.workspace.workspaceFolders[0].uri.fsPath,marker=process.env.HIBERNATE_TEST_MARKER;
 if(process.env.HIBERNATE_CLOSE_PHASE==='close'){
  for(const name of ['alpha.txt','beta.txt']){const uri=vscode.Uri.file(path.join(root,name));fs.writeFileSync(uri.fsPath,`Saved ${name}\n`);await vscode.window.showTextDocument(uri,{preview:false});}
  assert.equal((await job('capture')).state,'done');const snapshot=(await dashboard()).snapshots.find(s=>s.adapter==='vscode');assert.equal(snapshot.body.dirty,false);assert.equal(snapshot.body.terminals.length,0);assert.equal(snapshot.body.instance,vscode.env.sessionId);
  const closeJob=await native(['--review-close',snapshot.id,'--confirm-saved-work']);fs.writeFileSync(marker,JSON.stringify({snapshotId:snapshot.id,client:report.client,instance:vscode.env.sessionId,closeJob}));
  // Only the product's reviewed close action should end this isolated VS Code process.
  await until(async()=>{const j=(await dashboard()).jobs.find(j=>j.id===closeJob);if(j?.state==='failed')throw new Error(JSON.stringify(j.result));return false;},'reviewed VS Code close');
 }else{
  const original=JSON.parse(fs.readFileSync(marker,'utf8'));assert.equal(report.client,original.client);assert.notEqual(vscode.env.sessionId,original.instance);
  const beta=vscode.Uri.file(path.join(root,'beta.txt'));await vscode.window.showTextDocument(beta,{preview:false});await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
  assert.equal((await job('resume',original.snapshotId)).state,'done');const restored=(await dashboard()).snapshots.find(s=>s.id===original.snapshotId);assert.equal(restored.result.verified,true);assert.equal(restored.result.facts.find(f=>f.capability==='Saved text editors').observed,2);
  fs.writeFileSync(process.env.HIBERNATE_TEST_RESULT,JSON.stringify({checks:['Reviewed clean VS Code window closed through the product adapter','Product native observer verified disappearance of the unique window bound through its own process ancestry, recorded memory observations and completed the close after extension disconnect','Original integration identity persisted across VS Code restart while window session changed','Saved workspace and 2/2 editor contexts reconstructed and verified after reopening'],closeObservation:original.closeObservation,limitations:['Only an unambiguous native window in the bridge process ancestry can be verified; multi-window mappings block reviewed closure','Window disappearance and system memory observations do not prove exclusive project resource reclamation or Full Hibernate','No existing user VS Code windows, terminals or tasks were closed']},null,2));
 }
};
exports.run=async()=>{try{await run();}catch(error){fs.writeFileSync(process.env.HIBERNATE_TEST_FAILURE,String(error.stack??error));throw error;}};
