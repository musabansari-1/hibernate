import {useEffect,useRef,useState} from 'react';
import type {HibernationRun} from '../../../../packages/core/model';
export function ProjectCloseReview({review,onCancel,onApprove}:{review:HibernationRun;onCancel():void;onApprove(selected:string[]):void}){
 const dialog=useRef<HTMLDialogElement>(null);const [selected,setSelected]=useState<string[]>([]);const [saved,setSaved]=useState(false);
 useEffect(()=>{dialog.current?.showModal();return()=>dialog.current?.close();},[]);
 return <dialog className="review-dialog" ref={dialog} onCancel={onCancel} aria-labelledby="project-close-title">
  <p className="eyebrow">FINAL SAFETY CHECK</p><h2 id="project-close-title">Review workspace sleep</h2><h3>{review.body.project.name}</h3><small>{review.body.project.projectPath}</small>
  <div className="review-intro"><strong>Choose exactly what ContextSleep may close.</strong><p>Every selected window is checked again immediately before closing. If anything changed, looks unsafe, or cannot be verified, ContextSleep stops the sequence instead of guessing.</p></div>
  <p>Save your work first. Browser form contents, page application state and sign-in state are not reconstructed; any native unsaved-work prompt remains under the application's control.</p>
  {review.body.steps.map(s=>{const b=s.snapshot?.body as {tabs?:{title:string;url:string}[];groups?:{tabs:{label:string;uri?:string|null}[]}[]}|undefined;return <div key={s.id} className="result">
   <label className="context-choice"><input type="checkbox" disabled={!s.eligible} checked={selected.includes(s.id)} onChange={e=>setSelected(v=>e.target.checked?[...v,s.id]:v.filter(id=>id!==s.id))}/><strong>{s.label}</strong></label>
   <small>{s.eligible?'Available for review':'Retained / cannot close'}  |  {s.detail}</small>
   {s.snapshot&&<small>Captured {new Date(s.snapshot.created).toLocaleString()}</small>}
   {s.snapshot?.adapter==='chrome'&&b?.tabs&&<details><summary>Captured destinations ({b.tabs.length})</summary><ul>{b.tabs.map((t,i)=><li key={i}>{t.title||'Untitled tab'}<small>{t.url}</small></li>)}</ul></details>}
   {s.snapshot?.adapter==='vscode'&&b?.groups&&<details><summary>Captured editors</summary><ul>{b.groups.flatMap(g=>g.tabs).map((t,i)=><li key={i}>{t.label}<small>{t.uri??'Unsupported editor'}</small></li>)}</ul></details>}
  </div>;})}
  <label className="item"><input type="checkbox" checked={saved} onChange={e=>setSaved(e.target.checked)}/><span>I have saved the work in every selected window and reviewed what can be reconstructed.</span></label>
  <small>Approval is single-use. Expired or changed captures require a new capture. Retained contexts may continue consuming resources.</small>
  <div className="actions"><button onClick={onCancel}>Keep working</button><button onClick={()=>setSelected(review.body.steps.filter(s=>s.eligible).map(s=>s.id))}>Select supported windows</button><button className="primary" disabled={!saved||!selected.length} onClick={()=>onApprove(selected)}>Sleep selected windows</button></div>
 </dialog>;
}
