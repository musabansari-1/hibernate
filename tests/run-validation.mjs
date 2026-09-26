// Test-only native registration. Never replace a user's product host registration.
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,cp,readFile,writeFile,readdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {randomUUID} from 'node:crypto';
const run=resolve('.validation/suite-'+Date.now());await mkdir(run,{recursive:true});
const packageDir=process.env.HIBERNATE_TEST_PACKAGE;
let vscodeExtension=process.env.HIBERNATE_TEST_VSCODE_EXTENSION_DIR;
if(packageDir&&!vscodeExtension){
 const archives=(await readdir(packageDir)).filter(p=>p.endsWith('.vsix'));
 if(archives.length!==1)throw new Error('Package must contain exactly one VS Code extension archive');
 const unpack=join(run,'vscode-package');
 await promisify(execFile)('powershell.exe',['-NoProfile','-Command','Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::ExtractToDirectory($env:HIBERNATE_FIXTURE_VSIX, $env:HIBERNATE_FIXTURE_UNPACK)'],{env:{...process.env,HIBERNATE_FIXTURE_VSIX:join(packageDir,archives[0]),HIBERNATE_FIXTURE_UNPACK:unpack},windowsHide:true});
 vscodeExtension=join(unpack,'extension');
}
const extension=join(run,'chrome-extension');await cp(packageDir?join(packageDir,'chrome-extension'):'extensions/chrome/dist',extension,{recursive:true});
const name='local.contextsleep.fixture.'+randomUUID().replaceAll('-','');
for(const file of await readdir(extension)){if(file.endsWith('.js')){const path=join(extension,file);await writeFile(path,(await readFile(path,'utf8')).replaceAll('local.projecthibernate.bridge',name));}}
const host=process.env.HIBERNATE_TEST_HOST??(packageDir?join(packageDir,'hibernate-host.exe'):resolve('native/target/debug/hibernate-host.exe'));
const manifest=join(run,'native-host.json');await writeFile(manifest,JSON.stringify({name,description:'Isolated ContextSleep validation fixture',path:host,type:'stdio',allowed_origins:['chrome-extension://lilgdmjjanejoihlpegfklkfaofebfhn/']}));
const env={...process.env,HIBERNATE_TEST_HOST:host,HIBERNATE_TEST_NATIVE_HOST_NAME:name,HIBERNATE_TEST_CHROME_EXTENSION_DIR:extension,HIBERNATE_TEST_CHROME_EXTENSION:extension,HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]'};
if(vscodeExtension)env.HIBERNATE_TEST_VSCODE_EXTENSION_DIR=vscodeExtension;
if(packageDir)env.HIBERNATE_TEST_DESKTOP=join(packageDir,'ContextSleep.exe');
const scripts=process.argv.slice(2);
const selected=scripts.length?scripts:['browser-integration','browser-close-safety','reviewed-browser','quick-sleep','vscode-integration','vscode-close-reopen','desktop-smoke','project-discovery','project-checkpoint','project-hibernate','project-vscode','chrome-cold-restore','vscode-cold-restore','service-checkpoint-lifecycle','vscode-multiwindow'];
const results=[];let registered=false;
try{
 await promisify(execFile)('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('tests/fixture-bridge.ps1'),'-Name',name,'-Manifest',manifest],{windowsHide:true});registered=true;
 for(const test of selected){
  console.log('RUN '+test);let output='';const began=Date.now();
  const child=spawn(process.execPath,[resolve(`tests/${test}.mjs`)],{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
  child.stdout.on('data',s=>output+=s);child.stderr.on('data',s=>output+=s);
  const timer=setTimeout(()=>child.kill(),240000);
  const code=await new Promise((r,j)=>{child.on('exit',r);child.on('error',j);});clearTimeout(timer);
  await writeFile(join(run,test+'.log'),output);results.push({test,code,elapsedMs:Date.now()-began,log:join(run,test+'.log')});
  console.log(`${code===0?'PASS':'FAIL'} ${test} (${Math.round((Date.now()-began)/1000)}s)`);
  if(code!==0)console.log(output.slice(-2500));
 }
}finally{
 if(registered)await promisify(execFile)('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('tests/fixture-bridge.ps1'),'-Name',name,'-Manifest',manifest,'-Remove'],{windowsHide:true});
 await writeFile(join(run,'results.json'),JSON.stringify(results,null,2));console.log('Evidence: '+run);
}
process.exitCode=results.some(r=>r.code!==0)?1:0;
