import {chromium} from '@playwright/test';
import {spawn,spawnSync} from 'node:child_process';
import {mkdir,writeFile,readFile,readdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createServer} from 'node:net';
import {DatabaseSync} from 'node:sqlite';
import assert from 'node:assert/strict';
const root=resolve('.validation/desktop');const run=join(root,'run-'+Date.now());await mkdir(join(run,'project'),{recursive:true});
const port=await new Promise(resolvePort=>{const server=createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolvePort(port));});});
const env={...process.env,HIBERNATE_TEST_DISCOVERY_EXECUTABLES:'[]',HIBERNATE_DATA_DIR:join(run,'data'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
const child=spawn(process.env.HIBERNATE_TEST_DESKTOP??resolve('apps/desktop/src-tauri/target/release/project-hibernate.exe'),[],{env,windowsHide:true,stdio:'ignore'});
let browser;
try{
  for(let i=0;i<100;i++){try{browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);break;}catch{await new Promise(r=>setTimeout(r,250));}}
  if(!browser)throw new Error('WebView2 test debugging endpoint unavailable');
  const context=browser.contexts()[0];const page=context.pages()[0]??await context.waitForEvent('page');
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.locator('#apps-title').waitFor();
  await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Workspace name').fill('Native smoke project');
  await page.getByLabel('Project folder (manual association)').fill(join(run,'project'));
  await page.getByRole('button',{name:'Save',exact:true}).click();await page.getByRole('button',{name:'Workspace',exact:true}).click();
  await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByRole('button',{name:'Capture folders and terminals'}).click();
  await page.getByRole('button',{name:'History',exact:true}).click();await page.locator('.snapshot').filter({hasText:'Native smoke project'}).waitFor();
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('button',{name:'Export privacy-safe report',exact:true}).click();
  const exportsDir=join(run,'data','exports');let supportFile;
  for(let i=0;i<40;i++){const files=await readdir(exportsDir).catch(()=>[]);supportFile=files.find(f=>/^support-.*\.json$/.test(f));if(supportFile)break;await new Promise(r=>setTimeout(r,100));}
  assert.ok(supportFile,'Diagnostics export was not created');
  const supportRaw=await readFile(join(exportsDir,supportFile),'utf8');const support=JSON.parse(supportRaw);
  assert.equal(support.format,'project-hibernate-support-report');assert.equal(support.privacy.projectPaths,false);assert.equal(support.privacy.browserUrlsOrTitles,false);assert.ok(!supportRaw.includes('Native smoke project'));assert.ok(!supportRaw.includes(run));
  assert.equal(await page.getByRole('alert').count(),0);
  await page.screenshot({path:join(root,'dashboard.png'),fullPage:true});
  const db=new DatabaseSync(join(run,'data','hibernate.db'));db.prepare("INSERT INTO jobs VALUES(?,?,?,?,?,'interrupted',?,NULL)").run('smoke-interrupted','smoke','close',null,null,Date.now());db.close();
  await page.reload();await page.getByText('Interrupted work needs a look.').waitFor();
  await page.getByRole('button',{name:'I checked',exact:true}).click();assert.equal(await page.getByText('Interrupted work needs a look.').count(),0);
  await page.reload();await page.getByRole('button',{name:'History',exact:true}).click();await page.locator('.snapshot').filter({hasText:'Native smoke project'}).waitFor();assert.equal(await page.getByText('Interrupted work needs a look.').count(),0);
  const host=spawnSync(resolve('native/target/debug/hibernate-host.exe'),['--diagnose'],{env,encoding:'utf8',windowsHide:true});assert.equal(host.status,0);const data=JSON.parse(host.stdout);assert.equal(data.workspace.name,'Native smoke project');assert.ok(data.memory.totalPhysical>0);assert.ok(data.snapshots.some(s=>s.adapter==='context'));assert.equal(errors.length,0,errors.join('\n'));
  const result={checks:['Actual Tauri/WebView2 application rendered','Native memory and Windows discovery reached React through IPC','Project name/path saved through UI','Context snapshot persisted through UI and survived reload','Privacy-safe diagnostics exported through the real Tauri command without project name/path leakage','Interrupted durable work surfaces a recovery warning; acknowledgement hides only that known interruption and survives reload','No JavaScript runtime errors or visible native-command error'],limitations:['Screenshot includes real local window titles; keep it private','This smoke test does not close user applications']};await writeFile(join(root,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}catch(error){if(browser){const p=browser.contexts()[0]?.pages()[0];if(p){await p.screenshot({path:join(root,'failure.png'),fullPage:true}).catch(()=>{});console.error(await p.getByRole('alert').allTextContents());}}throw error;}finally{if(browser)await browser.close();child.kill();}
