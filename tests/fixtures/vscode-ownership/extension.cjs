// Only loaded from an isolated fixture extensions directory. No proposed APIs enabled.
const vscode=require('vscode');
const fs=require('node:fs');
const path=require('node:path');
const {execFile}=require('node:child_process');
const {promisify}=require('node:util');
exports.activate=async function(context){
 const root=process.env.HIBERNATE_OWNERSHIP_FIXTURE;
 const folder=vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
 if(!root||!folder)return;
 const relative=path.relative(root,folder);
 if(!['a','b'].includes(relative))return; // Fixture labels never used as native HWND authority.
 const label=relative;
 const emit=(name,value)=>{const target=path.join(root,`${label}-${name}.json`);fs.writeFileSync(target+'.tmp',JSON.stringify(value,null,2));fs.renameSync(target+'.tmp',target);};
 const native=async(args)=>JSON.parse((await promisify(execFile)(process.env.HIBERNATE_TEST_HOST,args,{windowsHide:true,maxBuffer:8_000_000})).stdout);
 const topology=async()=>JSON.parse((await promisify(execFile)(process.env.HIBERNATE_TOPOLOGY_PROBE,[],{windowsHide:true,maxBuffer:8_000_000})).stdout);
 let nativeHandleAccess;try{const value=vscode.window.nativeHandle;nativeHandleAccess={available:!!value};}catch(e){nativeHandleAccess={available:false,reason:String(e.message)};}
 let busy=false;
 const timer=setInterval(async()=>{
  if(busy)return;busy=true;
  try{
   emit('heartbeat',{at:Date.now(),instance:vscode.env.sessionId,dirty:vscode.workspace.textDocuments.some(d=>d.isDirty)});
   const file=path.join(root,`${label}-command.json`);
   if(!fs.existsSync(file))return;
   const command=JSON.parse(fs.readFileSync(file,'utf8'));fs.unlinkSync(file);
   if(command.action==='probe')emit(command.id,{instance:vscode.env.sessionId,extensionHostPid:process.pid,topology:await topology(),dashboard:await native(['--diagnose'])});
   else if(command.action==='dirty'){
    const document=await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(folder,'saved.txt')));
    const editor=await vscode.window.showTextDocument(document);
    await editor.edit(e=>e.insert(new vscode.Position(0,0),'Unsaved fixture edit\n'));
    emit(command.id,{dirty:document.isDirty});
   }else if(command.action==='clean'){
    // Only the fixture-created saved.txt document can be reverted by the fixture helper.
    const target=path.join(folder,'saved.txt');
    const doc=vscode.workspace.textDocuments.find(d=>d.uri.fsPath===target);
    if(doc?.isDirty){await vscode.window.showTextDocument(doc);await vscode.commands.executeCommand('workbench.action.files.revert');}
    emit(command.id,{dirty:vscode.workspace.textDocuments.some(d=>d.isDirty)});
   }else if(command.action==='close-fixture'){
    if(vscode.workspace.textDocuments.some(d=>d.isDirty))throw Error('Fixture cleanup refused dirty document');
    emit(command.id,{requested:true});
    await vscode.commands.executeCommand('workbench.action.closeWindow');
   }
  }catch(e){emit('failure',{error:String(e.stack??e)});}finally{busy=false;}
 },300);
 context.subscriptions.push({dispose:()=>clearInterval(timer)});
 try{
  await vscode.extensions.getExtension('project-hibernate.project-hibernate').activate();
  await vscode.commands.executeCommand('hibernate.connect');
  emit('ready',{instance:vscode.env.sessionId,extensionHostPid:process.pid,version:vscode.version,nativeHandleAccess,topology:await topology()});
 }catch(e){emit('failure',{error:String(e.stack??e)});}
};
