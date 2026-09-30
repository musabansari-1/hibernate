// Production lifecycle: actual Tauri UI + pinned private app and generated PDFs.
import {chromium} from '@playwright/test';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,copyFile,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {createServer} from 'node:net';
import assert from 'node:assert/strict';
const run=resolve('.validation/deep-freeze-ui/run-'+Date.now());await mkdir(join(run,'profile'),{recursive:true});
const source=resolve('.validation/deep-freeze-catalog/sumatra-3.6.1/SumatraPDF-3.6.1-64.exe');
assert.equal(createHash('sha256').update(await readFile(source)).digest('hex'),'719f689b34f47be8ca105ce8484948474dafde0e106bab599e4a89326070c3d0');
const fixture=join(run,'SumatraPDF.exe');await copyFile(source,fixture);
await writeFile(join(run,'profile/SumatraPDF-settings.txt'),'ReuseInstance = false\nUseTabs = false\nRememberOpenedFiles = false\nRememberStatePerDocument = false\nCheckForUpdates = false\n');
function document(label){
 const objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>'];
 for(let page=0;page<2;page++){
  const stream=`BT /F1 24 Tf 60 700 Td (${label} - page ${page+1}) Tj ET`;
  objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 7 0 R >> >> /Contents ${4+page*2} 0 R >>`,`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`);
 }
 objects.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
 let pdf='%PDF-1.4\n';const offsets=[0];objects.forEach((o,i)=>{offsets.push(Buffer.byteLength(pdf));pdf+=`${i+1} 0 obj\n${o}\nendobj\n`;});
 const xref=Buffer.byteLength(pdf);pdf+=`xref\n0 ${objects.length+1}\n0000000000 65535 f \n`+offsets.slice(1).map(n=>`${String(n).padStart(10,'0')} 00000 n \n`).join('');
 return pdf+`trailer\n<< /Size ${objects.length+1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
}
const documents=[join(run,'first.pdf'),join(run,'second.pdf')];
for(let i=0;i<documents.length;i++)await writeFile(documents[i],document(`ContextSleep private document ${i+1}`));
// Sumatra honors SW_HIDE; this explicitly visible disposable GUI is the test target.
const app=spawn(fixture,['-new-window','-appdata',join(run,'profile'),...documents],{windowsHide:false,stdio:'ignore'});
const port=await new Promise(r=>{const s=createServer();s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
const env={...process.env,HIBERNATE_DATA_DIR:join(run,'data'),HIBERNATE_TEST_DISCOVERY_EXECUTABLES:JSON.stringify([fixture]),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
const desktopPath=resolve(process.env.DEEP_FREEZE_DESKTOP_EXE??'apps/desktop/src-tauri/target/release/project-hibernate.exe');
let desktop=spawn(desktopPath,[],{env,windowsHide:true,stdio:'ignore'});
let browser,page;
const until=async(fn,label)=>{const end=Date.now()+30000;while(Date.now()<end){const value=await fn().catch(()=>null);if(value)return value;await new Promise(r=>setTimeout(r,200));}throw new Error(label+' timed out');};
async function responsive(){
 const {stdout}=await promisify(execFile)('powershell.exe',['-NoProfile','-Command',`
$p=Get-Process -Id $env:FREEZE_FIXTURE_PID -ErrorAction Stop
if($p.Path -ne $env:FREEZE_FIXTURE_EXE){throw 'Private fixture identity changed'}
Add-Type -TypeDefinition @'
using System;using System.Text;using System.Runtime.InteropServices;
public class PrivateResponse {public delegate bool Callback(IntPtr h,IntPtr p);
[DllImport("user32.dll")]static extern bool EnumWindows(Callback c,IntPtr p);
[DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
[DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern int GetClassName(IntPtr h,StringBuilder b,int n);
[DllImport("user32.dll")]static extern IntPtr SendMessageTimeout(IntPtr h,uint m,IntPtr w,IntPtr l,uint flags,uint timeout,out IntPtr result);
public static bool Check(uint pid){int count=0;bool ok=true;EnumWindows((h,p)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid){var b=new StringBuilder(256);GetClassName(h,b,256);if(b.ToString()=="SUMATRA_PDF_FRAME"){count++;IntPtr r;ok &= SendMessageTimeout(h,0,IntPtr.Zero,IntPtr.Zero,3,300,out r)!=IntPtr.Zero;}}return true;},IntPtr.Zero);return count==2&&ok;}}
'@
[PrivateResponse]::Check($p.Id)
`],{windowsHide:true,env:{...process.env,FREEZE_FIXTURE_PID:String(app.pid),FREEZE_FIXTURE_EXE:fixture}});
 return stdout.trim()==='True';
}
try {
 browser=await until(()=>chromium.connectOverCDP(`http://127.0.0.1:${port}`),'desktop');page=browser.contexts()[0].pages()[0];
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.locator('#memory-title').waitFor();
 const ipc=(cmd,args={})=>page.evaluate(([cmd,args])=>window.__TAURI_INTERNALS__.invoke(cmd,args),[cmd,args]);
 const data=await until(async()=>{const d=await ipc('dashboard');return d.sleepMemory?.apps.length===1?d:null;},'private app memory row');
 const row=data.sleepMemory.apps[0];assert.equal(row.identity.pid,app.pid);assert.ok(row.memory.workingSet>0);assert.equal(row.assessment.safeToFreeze,true);assert.equal(row.strategy,'GenericDeepFreeze');
 assert.equal(await responsive(),true,'Both private document windows respond before sleep');
 await page.getByRole('checkbox',{name:'Select whole app SumatraPDF.exe'}).check();
 await page.getByRole('button',{name:'Sleep selected',exact:true}).click();
 await page.getByRole('dialog').waitFor();
 assert.equal(await page.getByRole('button',{name:'Confirm sleep',exact:true}).isDisabled(),true);
 await page.getByRole('button',{name:'Keep working',exact:true}).click();
 await page.getByRole('button',{name:'Sleep selected',exact:true}).click();
 await page.getByRole('checkbox',{name:'I selected every window',exact:false}).check();
 await page.getByRole('button',{name:'Confirm sleep',exact:true}).click();
 await page.getByRole('button',{name:'Wake',exact:true}).waitFor();
 const frozen=await ipc('dashboard');const operation=frozen.sleepMemory.operations.find(o=>o.canWake);assert.ok(operation);
 const windows=operation.body.review.apps[0].windows.filter(w=>w.visible);
 assert.ok(windows.some(w=>w.title.includes('first.pdf')));assert.ok(windows.some(w=>w.title.includes('second.pdf')));
 assert.equal(operation.body.targets.length,1);assert.deepEqual(operation.body.targets[0].identity,row.identity);
 assert.equal(await responsive(),false,'The selected GUI process is actually paused');
 await page.screenshot({path:join(run,'sleeping.png')});
 await new Promise(r=>setTimeout(r,10000));
 await page.getByRole('button',{name:'Wake',exact:true}).click();
 await until(async()=>!(await ipc('dashboard')).sleepMemory.operations.some(o=>o.canWake),'Wake release');
 assert.equal(await responsive(),true,'Both document windows respond after Wake');
 await assert.rejects(ipc('review_app_sleep',{ids:['forged']}));
 await ipc('cancel_app_sleep_review',{reviewId:'forged'});
 await ipc('cancel_app_sleep_review',{reviewId:'forged'});
 await assert.rejects(ipc('sleep_apps',{reviewId:'forged',acknowledged:true}));
 await assert.rejects(ipc('wake_apps',{id:'forged'}));
 const after=await ipc('dashboard');assert.deepEqual(after.sleepMemory.apps[0].identity,row.identity);assert.equal(app.exitCode,null);
 // A second operation proves that desktop teardown releases its native handles.
 const review=await ipc('review_app_sleep',{ids:[after.sleepMemory.apps[0].id]});
 await ipc('sleep_apps',{reviewId:review.id,acknowledged:true});
 await browser.close();browser=undefined;
 await new Promise(r=>{desktop.once('exit',r);desktop.kill();});
 assert.equal(await responsive(),true,'Controller exit released its owned pause before restart');
 // Restart with isolated data and a fresh CDP port, never replaying journal intent.
 const nextPort=await new Promise(r=>{const s=createServer();s.listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
 desktop=spawn(desktopPath,[],{env:{...env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${nextPort}`},windowsHide:true,stdio:'ignore'});
 browser=await until(()=>chromium.connectOverCDP(`http://127.0.0.1:${nextPort}`),'restarted desktop');page=browser.contexts()[0].pages()[0];
 page.on('pageerror',e=>errors.push(e.message));
 await page.locator('#memory-title').waitFor();
 const recovered=await ipc('dashboard');assert.ok(recovered.sleepMemory.operations.some(o=>o.state==='owner-exited'));assert.ok(!recovered.sleepMemory.operations.some(o=>o.canWake));
 assert.deepEqual(recovered.sleepMemory.apps[0].identity,row.identity);assert.equal(app.exitCode,null);
 assert.equal(await page.getByRole('alert').count(),0);assert.deepEqual(errors,[]);
 await page.setViewportSize({width:1280,height:900});await page.locator('#memory-title').scrollIntoViewIfNeeded();await page.screenshot({path:join(run,'memory.png')});
 await page.setViewportSize({width:640,height:900});await page.screenshot({path:join(run,'memory-narrow.png')});
 const results={checks:['Memory-first UI reads actual Windows physical memory','Pinned app automatically eligible without integration','Two generated PDF windows selected as one whole process','Real UI review, cancellation, acknowledgement, Sleep and Wake','Forged review/sleep/wake IDs rejected','Desktop crash and restart preserve original target identity and release owned pause','No UI runtime errors'],scope:'Production desktop lifecycle with private, ordinary-mode SumatraPDF and two generated documents',desktopPath,identity:row.identity,assessment:row.assessment,operation,recovered:recovered.sleepMemory.operations,screenshots:['sleeping.png','memory.png','memory-narrow.png']};
 await writeFile(join(run,'results.json'),JSON.stringify(results,null,2));console.log(JSON.stringify({run,...results},null,2));
} catch(error) {
 console.error('Memory UI validation failed:',error);
 await page?.screenshot({path:join(run,'failure.png')}).catch(()=>{});
 throw error;
} finally {
 await browser?.close().catch(()=>{});desktop.kill();
 if(app.exitCode===null) {
  // Cooperative cleanup is restricted to this exact harness image and PID.
  await promisify(execFile)('powershell.exe',['-NoProfile','-Command',`$p=Get-Process -Id $env:FREEZE_FIXTURE_PID -ErrorAction SilentlyContinue
if($p -and $p.Path -eq $env:FREEZE_FIXTURE_EXE){
Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public class PrivateFixtureClose {public delegate bool Callback(IntPtr h,IntPtr p);[DllImport("user32.dll")]static extern bool EnumWindows(Callback c,IntPtr p);[DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);public static void Close(uint pid){EnumWindows((h,p)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid)PostMessage(h,0x10,IntPtr.Zero,IntPtr.Zero);return true;},IntPtr.Zero);}}
'@
[PrivateFixtureClose]::Close($p.Id)
if(!$p.WaitForExit(10000)){throw 'Fixture did not close; no kill attempted'}}`],{windowsHide:true,env:{...process.env,FREEZE_FIXTURE_PID:String(app.pid),FREEZE_FIXTURE_EXE:fixture}});
 }
}
