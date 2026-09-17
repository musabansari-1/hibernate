import {useState} from 'react';
import type {ProjectCheckpoint,HibernationRun} from '../../../../packages/core/model';
import {VerificationResult} from './VerificationResult';
import {ProjectCloseReview} from './ProjectCloseReview';
export function CheckpointCard({checkpoint:c,busy,restore,reviewClose,closeReviewed,exportBackup}:{checkpoint:ProjectCheckpoint;busy:boolean;restore(selected:string[],acknowledgedServices:boolean):void;reviewClose():Promise<HibernationRun|undefined>;closeReviewed(reviewId:string,selected:string[]):void;exportBackup():void}){
 const entries=c.body.entries??[];const services=c.body.services??[];
 const [selected,setSelected]=useState<string[]>([]);const [approvedServices,setApprovedServices]=useState(false);
 const [review,setReview]=useState<HibernationRun>();const lastClose=c.hibernations?.[0];
 const latest=c.runs[0];const captured=c.body.captures.filter(v=>v.state==='captured').length;
 const keys=[...entries.map(e=>e.id),...(services.length?['services']:[])];
 function toggle(id:string,checked:boolean){if(id==='services'&&!checked)setApprovedServices(false);setSelected(s=>checked?[...s,id]:s.filter(k=>k!==id));}
 return <article className="snapshot checkpoint">
  <div className="section-title"><div><h3>{c.body.project?.name??'Project checkpoint'}</h3><small>{new Date(c.created).toLocaleString()}  |  {captured}/{c.body.captures.length} contexts captured</small></div><span className="badge">{c.state}</span></div>
  {c.body.error&&<p role="alert">{c.body.error}</p>}
  {c.body.importWarnings?.map((warning,i)=><p key={`import-warning-${i}`} role="status" className="error">Imported checkpoint: {warning}</p>)}
  {c.body.services?.filter(s=>s.importWarning).map(s=><p key={`service-warning-${s.id}`} role="status" className="error">{s.name}: {s.importWarning}</p>)}
  {c.body.captures.filter(v=>v.state!=='captured').map((v,i)=><p key={i} className="error">Capture {v.state}: {v.detail??'No completed capture recorded. Individual snapshots may still be available in history.'}</p>)}
  <div className="actions"><button disabled={busy||c.state==='capturing'} onClick={()=>void reviewClose().then(setReview)}>Review workspace sleep</button><button disabled={busy||c.state==='capturing'} onClick={exportBackup}>Export encrypted checkpoint</button></div>
  {review&&<ProjectCloseReview review={review} onCancel={()=>setReview(undefined)} onApprove={parts=>{setReview(undefined);closeReviewed(review.id,parts);}}/>}
  {lastClose&&<div className="project-close-result"><h4>Sleep observations  |  {lastClose.state}</h4><p>{lastClose.state==='closed-selected'?'Selected window disappearance was verified. Review retained contexts and resource observations before considering the whole project put away.':lastClose.state==='blocked'?'Preflight blocked closing. No selected window was closed by this run.':lastClose.state==='interrupted'?'This run was interrupted. Inspect the applications; queued project closes were canceled.':lastClose.state==='partial'?'A close failed or could not be verified. Later windows were not closed.':`Phase: ${lastClose.body.phase}`}</p>
   {lastClose.body.detail&&<p>{lastClose.body.detail}</p>}
   {lastClose.body.steps.map(s=><div key={s.id} className="result"><strong>{s.label}</strong><span className="badge">{lastClose.state==='interrupted'&&['checking','closing','ready','pending'].includes(s.state)?'interrupted':s.state.replaceAll('_',' ')}</span><small>{s.detail}</small>{s.result!==undefined&&<VerificationResult value={s.result}/>}</div>)}
   {lastClose.body.before&&lastClose.body.after&&<div className="measurement"><p>System available memory: {(lastClose.body.before.availablePhysical/1024**3).toFixed(2)}  to  {(lastClose.body.after.availablePhysical/1024**3).toFixed(2)} GiB</p><small>Measured across this close sequence. Other applications and shared processes affect the change; this is not exclusive project RAM.</small></div>}
  </div>}
  <p>Select the context to reconstruct. Chrome uses lazy tabs; terminals open a new shell without replaying commands. Offline VS Code workspaces may be opened to reconnect their integration.</p>
  <div className="checkpoint-parts">{entries.map(e=><label key={e.id}><input type="checkbox" disabled={busy} checked={selected.includes(e.id)} onChange={v=>toggle(e.id,v.target.checked)}/>{e.label}</label>)}
   {!!services.length&&<label><input type="checkbox" disabled={busy} checked={selected.includes('services')} onChange={v=>toggle('services',v.target.checked)}/>Restart checkpoint services: {services.map(s=>`${s.name} (${s.url})`).join(', ')}</label>}
  </div>
  {selected.includes('services')&&<div className="result"><strong>Saved service commands</strong><p>These commands run project code with your Windows account. Review the exact checkpoint definitions before approving restart.</p>{services.map(s=><details key={s.id}><summary>{s.name}</summary><pre>{s.launch?JSON.stringify(s.launch,null,2):'No saved startup definition'}</pre></details>)}<label className="item"><input type="checkbox" disabled={busy||services.some(s=>!s.launch)} checked={approvedServices} onChange={e=>setApprovedServices(e.target.checked)}/><span>I reviewed these saved commands and approve restarting them from this checkpoint.</span></label></div>}
  <div className="actions"><button disabled={busy||!keys.length} onClick={()=>{setSelected(keys);setApprovedServices(false);}}>Select all parts</button><button className="primary" disabled={busy||!selected.length||c.state==='capturing'||(selected.includes('services')&&!approvedServices)} onClick={()=>restore(selected,approvedServices)}>Restore selected context</button></div>
  {latest&&<div className="reconstruction"><h4>Restore observations  |  {new Date(latest.created).toLocaleString()}</h4><p>{latest.state==='interrupted'?'Restore was interrupted. Inspect the applications before retrying; completed side effects were not replayed.':latest.state==='running'?'Reconstruction is in progress.':'Processing finished. Review the observed capabilities below to see what is ready.'}</p>
   {latest.body.readiness&&<div className="result"><strong>Project readiness</strong><span className="badge">{latest.body.readiness.status}</span><p>{latest.body.readiness.detail}</p>{latest.body.readiness.facts.map((fact,i)=><small key={i}>{fact.capability}  |  {fact.status}</small>)}</div>}
   {latest.body.steps.map(step=><div key={step.id} className="result"><strong>{step.label}</strong><span className="badge">{latest.state==='interrupted'&&['pending','running'].includes(step.state)?'interrupted':step.state}</span>{step.detail&&<p>{step.detail}</p>}{step.result!==undefined&&<VerificationResult value={step.result}/>}</div>)}
  </div>}
  <details><summary>Captured project evidence and previous restores</summary><pre>{JSON.stringify({repository:c.body.repository,services,previousRuns:c.runs.slice(1)},null,2)}</pre></details>
 </article>;
}
