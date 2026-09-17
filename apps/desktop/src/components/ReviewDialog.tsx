import {useEffect,useRef,useState} from 'react';
import type {Snapshot} from '../../../../packages/core/model';
export function ReviewDialog({snapshot,onCancel,onApprove}:{snapshot:Snapshot;onCancel():void;onApprove(quick:boolean):void}){
 const dialog=useRef<HTMLDialogElement>(null);const [saved,setSaved]=useState(false);const [quick,setQuick]=useState(false);
 useEffect(()=>{dialog.current?.showModal();return()=>dialog.current?.close();},[]);
 const body=snapshot.body as {tabs?:{title:string;url:string}[];dirty?:boolean;terminals?:unknown[];groups?:unknown[]};
 const browser=snapshot.adapter==='chrome';const blocked=!browser&&(body.dirty||!!body.terminals?.length);
 return <dialog ref={dialog} onCancel={onCancel} aria-labelledby="review-title">
  <h2 id="review-title">Review {browser?'Chrome':'VS Code'} sleep</h2>
  <p>{browser?`${body.tabs?.length??0} tabs are saved with their URLs, order, pins and groups. Background tabs will restore lazily.`:'Project identity and saved editor metadata have been captured.'}</p>
  <p>{browser?'Web forms, page contents and login state are not stored. Save work in the websites before continuing. Chrome may ask you to confirm unsaved work; canceling keeps the window open.':'Unsaved editors, active tasks, debugging, custom editors and existing terminal sessions block closing. Save or finish them in VS Code and capture again.'}</p>
  {!!body.tabs?.length&&<details><summary>Review captured destinations ({body.tabs.length})</summary><ul>{body.tabs.map((t,i)=><li key={i}><strong>{t.title||'Untitled tab'}</strong><small>{t.url}</small></li>)}</ul></details>}
  {blocked&&<p role="alert">This capture contains unsaved work or terminal sessions. Closing is blocked.</p>}
  {browser&&<label className="item"><input type="checkbox" checked={quick} onChange={e=>setQuick(e.target.checked)}/><span>Quick Sleep instead: discard eligible background pages. Active, pinned and protected tabs stay loaded; selecting sleeping tabs reloads them.</span></label>}
  <label className="item"><input type="checkbox" checked={saved} onChange={e=>setSaved(e.target.checked)}/><span>I have saved the work in this window and reviewed what can be reconstructed.</span></label>
  <small>Approval applies once to this captured window. Changed tabs, downloads or an expired capture require a new review. Resource changes will be measured after the operation.</small>
  <div className="actions"><button onClick={onCancel}>Keep working</button><button className="primary" disabled={!saved||!!blocked} onClick={()=>onApprove(quick)}>{quick?'Quick Sleep reviewed tabs':'Deep sleep reviewed window'}</button></div>
 </dialog>;
}
