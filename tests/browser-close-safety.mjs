import { chromium } from '@playwright/test';
import { createServer } from 'node:http';
import { readdir, mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('.validation/close-safety');await mkdir(root,{recursive:true});
const candidates=(await readdir(join(process.env.LOCALAPPDATA,'ms-playwright'))).filter(p=>/^chromium-\d+$/.test(p)).sort().reverse();
const executablePath=process.env.HIBERNATE_TEST_CHROMIUM??join(process.env.LOCALAPPDATA,'ms-playwright',candidates[0],'chrome-win64','chrome.exe');
const extension=(process.env.HIBERNATE_TEST_CHROME_EXTENSION_DIR??resolve('extensions/chrome/dist'));
const server=createServer((_,res)=>res.end('<title>Hibernate close safety fixture</title><input id="work"><script>onbeforeunload=e=>{e.preventDefault();e.returnValue=""}</script>'));
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const context=await chromium.launchPersistentContext(join(root,'profile-'+Date.now()),{env:{...process.env,HIBERNATE_DATA_DIR:join(root,'data-'+Date.now())},executablePath,headless:true,channel:'chromium',args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
try{
 const worker=context.serviceWorkers()[0]??await context.waitForEvent('serviceworker');
 const win=await worker.evaluate(url=>chrome.windows.create({url}),`http://127.0.0.1:${server.address().port}`);
 let page;for(let i=0;i<40;i++){page=context.pages().find(p=>p.url().startsWith(`http://127.0.0.1:${server.address().port}`));if(page)break;await new Promise(r=>setTimeout(r,100));}
 assert.ok(page);await page.locator('#work').fill('Keep my unsaved fixture');
 let dialogSeen=false;page.on('dialog',async dialog=>{dialogSeen=true;await dialog.dismiss();});
 const closing=worker.evaluate(async id=>{try{await chrome.windows.remove(id);return 'resolved';}catch(e){return String(e);}},win.id);
 await new Promise(r=>setTimeout(r,2500));
 const present=await worker.evaluate(async id=>(await chrome.windows.getAll()).some(w=>w.id===id),win.id);
 assert.equal(dialogSeen,true,'Chrome must surface beforeunload when closing through the extension API');
 assert.equal(present,true,'Canceling the native beforeunload dialog must preserve the window');
 assert.equal(await page.locator('#work').inputValue(),'Keep my unsaved fixture');
 await page.evaluate(()=>{onbeforeunload=null;});await worker.evaluate(id=>chrome.windows.remove(id),win.id);await closing;
 const result={checks:['Chrome extension windows.remove surfaced a real beforeunload dialog','Canceling close preserved the window and unsaved input','Removing the fixture guard then allowed closure']};
 await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(result);
}finally{await context.close();await new Promise(r=>server.close(r));}
