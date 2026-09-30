import {useEffect, useRef, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {Dashboard, SleepReview} from '../../../../packages/core/model';
import {Badge, Card, Evidence, Meter, SectionHead} from './ui';
const bytes = (n:number) => n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(1)} GB` : `${(n / 1024 ** 2).toFixed(1)} MB`;
const identityKey = (p:{pid:number;processStarted:string}) => `${p.pid}:${p.processStarted}`;

export function AppMemory({data, onRefresh, onVerified}:{data:Dashboard;onRefresh:()=>Promise<unknown>;onVerified:()=>void}) {
  const [selected,setSelected]=useState<string[]>([]);
  const [review,setReview]=useState<SleepReview>();
  const [acknowledged,setAcknowledged]=useState(false);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [notice,setNotice]=useState('');
  const [now,setNow]=useState(Date.now());
  const dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{if(review&&!dialog.current?.open)dialog.current?.showModal();},[review]);
  useEffect(()=>{const timer=setInterval(()=>setNow(Date.now()),1000);return()=>clearInterval(timer);},[]);
  const memory=data.memory;
  const rows=data.sleepMemory?.apps??[];
  const sleeping=(data.sleepMemory?.operations??[]).filter(o=>o.canWake);
  const sleepingKeys=new Set(sleeping.flatMap(o=>o.body.targets.map(t=>identityKey(t.identity))));
  const running=rows.filter(r=>!sleepingKeys.has(identityKey(r.identity)));
  const chosen=running.filter(r=>selected.includes(r.id)&&r.strategy==='GenericDeepFreeze');
  const observed=chosen.reduce((sum,r)=>sum+(r.memory?.workingSet??0),0);
  const available=memory.availablePhysical/memory.totalPhysical;
  const pressure=available<.1?'High':available<.25?'Moderate':'Low';
  async function act(task:()=>Promise<void>){setBusy(true);setError('');setNotice('');try{await task();}catch(e){setError(String(e));}finally{setBusy(false);await onRefresh().catch(e=>setError(String(e)));}}
  function cancelReview(){const id=review?.id;setReview(undefined);if(id)void invoke('cancel_app_sleep_review',{reviewId:id}).catch(e=>setError(String(e)));}
  return <section className="block memory-home" aria-labelledby="memory-title">
    <SectionHead eyebrow="Make room for what matters" id="memory-title" title="Memory" description="Choose whole apps to put away. Wake brings paused apps back in the same Windows session." />
    <Card className="physical-memory">
      <div><strong>{bytes(memory.totalPhysical-memory.availablePhysical)} / {bytes(memory.totalPhysical)} used</strong><Badge tone={pressure==='High'?'warn':'neutral'}>Pressure: {pressure}</Badge></div>
      <Meter percent={(1-available)*100} label={`${pressure} memory pressure`} />
      <small>{bytes(memory.availablePhysical)} available physical memory</small>
    </Card>
    {error&&<p role="alert">{error}</p>}{data.sleepMemory?.error&&<p role="alert">Memory app discovery unavailable: {data.sleepMemory.error}</p>}
    {notice&&<p role="status">{notice}</p>}
    <h3>Running apps</h3>
    <p className="section-description">Supported apps can pause here without losing their local process state. This release supports SumatraPDF 3.6.1 on Windows 11 build 26200. Other apps stay open. Chrome and VS Code keep their reviewed workspace sleep path below.</p>
    <div className="memory-apps">
      {running.map(row=><Card key={row.id} className="memory-app" data-sleep-app={row.id}>
        <label><input type="checkbox" aria-label={`Select whole app ${row.label}`} disabled={busy||row.strategy!=='GenericDeepFreeze'} checked={chosen.some(c=>c.id===row.id)} onChange={e=>setSelected(s=>e.target.checked?[...s,row.id]:s.filter(id=>id!==row.id))}/><span><strong>{row.label}</strong><small>{row.title}</small><small>{row.strategy==='GenericDeepFreeze'?'Deep Freeze · all app windows':row.enhancement?'Use the reviewed workspace sleep below':'Sleep unavailable · keep open'}</small></span></label>
        <div><strong>{row.memory?bytes(row.memory.workingSet):'Unavailable'}</strong><small>Physical working set</small></div>
        <Evidence label="Why this sleep option?"><p>{row.assessment.safeToFreeze?'This exact app version can pause as a whole process. All of its visible and helper windows are included.':row.assessment.blockers.join('. ')}</p><p>Commit: {row.memory?bytes(row.memory.privateCommit):'unavailable'}. This row measures the window-owning process; child processes are not included. Shared memory can appear in several processes.</p></Evidence>
      </Card>)}
      {!running.length&&<p>No running app windows observed.</p>}
    </div>
    <div className="memory-actions"><span>{chosen.length} selected · {bytes(observed)} observed working set<small>Actual physical RAM reclaim varies. This is not a guaranteed saving.</small></span><button className="btn btn-primary" disabled={busy||!chosen.length} onClick={()=>void act(async()=>{setReview(await invoke<SleepReview>('review_app_sleep',{ids:chosen.map(r=>r.id)}));setAcknowledged(false);})}>Sleep selected</button></div>
    {data.reports.some(r=>['chrome','vscode'].includes(r.adapter)&&Date.now()-r.at<15000)&&<button className="btn btn-quiet" onClick={onVerified}>Review Chrome / VS Code sleep</button>}
    <h3>Sleeping</h3>
    {!sleeping.length&&<p>No apps are paused by ContextSleep.</p>}
    {sleeping.map(op=>{const before=op.body.before.reduce((sum,m)=>sum+m.workingSet,0);const after=op.body.after?.reduce((sum,m)=>sum+m.workingSet,0);return <Card className="memory-app sleeping-app" key={op.id}>
      <div><strong>{op.body.review?.apps.map(a=>a.label).join(', ')??'Sleeping apps'}</strong><small>{bytes(before)} before sleep{after!==undefined?` · ${bytes(after)} after trimming`:''}</small><small>Same processes paused. Windows remain visible. Quitting ContextSleep releases its pauses.</small></div>
      <button className="btn btn-primary" disabled={busy} onClick={()=>void act(async()=>{await invoke('wake_apps',{id:op.id});setNotice('Wake requested and owned pauses released. Check your apps before continuing.');})}>Wake</button>
      <Evidence label="Memory measurements"><pre>{JSON.stringify({before:op.body.before,after:op.body.after},null,2)}</pre><p>Working-set reduction and changes in system available RAM are different measurements. Commit remains allocated.</p></Evidence>
    </Card>;})}
    {(data.sleepMemory?.operations??[]).filter(o=>o.state==='owner-exited').slice(-1).map(o=><p role="status" key={o.id}>A previous ContextSleep session ended. Windows released its app pauses. App health is unverified; no app was relaunched.</p>)}
    {review&&<dialog ref={dialog} className="sheet sleep-review" aria-labelledby="sleep-review-title" onCancel={e=>{if(busy)e.preventDefault();else cancelReview();}}>
      <h2 id="sleep-review-title">Review whole-app sleep</h2><p>{review.warning}</p>
      {review.apps.map(app=><div key={identityKey(app.identity)}><strong>{app.label} · {bytes(app.memory.workingSet)}</strong><p>{app.windows.length} windows, including helpers, will pause together.</p><ul>{app.windows.filter(w=>w.visible).map((w,i)=><li key={i}>{w.title||'Untitled window'}</li>)}</ul></div>)}
      <label><input type="checkbox" checked={acknowledged} onChange={e=>setAcknowledged(e.target.checked)}/> I selected every window in these apps and finished printing and other external work.</label>
      <p>{now>=review.expiresAt?'Review expired. Keep working, then review again.':`Review valid for ${Math.ceil((review.expiresAt-now)/1000)} seconds.`}</p>
      <div className="memory-actions"><button className="btn btn-quiet" disabled={busy} onClick={cancelReview}>Keep working</button><button className="btn btn-primary" disabled={busy||!acknowledged||now>=review.expiresAt} onClick={()=>void act(async()=>{const current=review;setReview(undefined);await invoke('sleep_apps',{reviewId:current.id,acknowledged});setSelected([]);setNotice('Selected apps are sleeping. Use Wake to continue.');})}>Confirm sleep</button></div>
    </dialog>}
  </section>;
}
