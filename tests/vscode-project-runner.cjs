// Holds a clean, isolated VS Code workspace for the desktop coordinator test.
const vscode=require('vscode');
const fs=require('node:fs');
const path=require('node:path');
exports.run=async()=>{
 try{
  await vscode.workspace.getConfiguration('hibernate').update('nativeHostPath',process.env.HIBERNATE_TEST_HOST,vscode.ConfigurationTarget.Global);
  await vscode.extensions.getExtension('project-hibernate.project-hibernate').activate();
  await vscode.commands.executeCommand('hibernate.connect');
  const root=vscode.workspace.workspaceFolders[0].uri.fsPath;
  for(const name of ['alpha.txt','beta.txt'])await vscode.window.showTextDocument(vscode.Uri.file(path.join(root,name)),{preview:false});
  if(process.env.HIBERNATE_PROJECT_PHASE==='restore')await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
  fs.writeFileSync(process.env.HIBERNATE_TEST_READY,JSON.stringify({instance:vscode.env.sessionId}));
  await new Promise(resolve=>setTimeout(resolve,180000));
 }catch(error){fs.writeFileSync(process.env.HIBERNATE_TEST_FAILURE,String(error.stack??error));throw error;}
};
