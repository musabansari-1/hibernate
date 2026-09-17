import React,{useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {ProjectDiscovery as Discovery} from '../../../../packages/core/model';

export function ProjectDiscovery({enabled,busy,accept}:{enabled:boolean;busy:boolean;accept:(candidate:Discovery['candidates'][number])=>Promise<void>}){
 const [result,setResult]=useState<Discovery>();const [error,setError]=useState('');const [scanning,setScanning]=useState(false);const pending=useRef(false);
 async function discover(){if(!enabled||pending.current)return;pending.current=true;setScanning(true);try{setResult(await invoke<Discovery>('discover_projects'));setError('');}catch(e){setError(String(e));}finally{pending.current=false;setScanning(false);}}
 useEffect(()=>{void discover();const timer=setInterval(()=>{if(document.visibilityState==='visible')void discover();},30000);return()=>clearInterval(timer);},[enabled]);
 return <div className="project-discovery"><div className="section-title"><h3>Discovered repositories</h3><button disabled={!enabled||scanning||busy} onClick={()=>void discover()}>{scanning?'Checking folders...':'Discover projects'}</button></div>
  <p>Suggestions come from local folders reported by VS Code, instrumented terminals and Explorer. Choose a repository to add it, then assign its application windows.</p>
  {error&&<p role="status">Discovery unavailable: {error}</p>}
  {result&&!result.candidates.length&&<p>No repository suggestion is available from the currently reported folders.</p>}
  {result?.candidates.map(c=><div className="evidence" key={c.projectPath}><div className="grow"><strong>{c.name}{c.repository.branch?`  |  ${c.repository.branch}`:''}</strong><small>{c.projectPath}</small><small>{c.evidence.map(e=>e.source).join('  |  ')}</small><details><summary>Why this project?</summary>{c.evidence.map(e=><p key={e.source+e.path}>{e.label}<small>{e.path}</small></p>)}</details></div><button disabled={busy||scanning} onClick={()=>void accept(c).then(discover)}>{c.existingProjectId?'Open project':'Add project'}</button></div>)}
  {result?.explorerError&&<small>Explorer folders unavailable: {result.explorerError}</small>}
  {result&&<small>Checked {new Date(result.checkedAt).toLocaleTimeString()}. Localhost tabs alone are not assigned to a repository.</small>}
 </div>;
}
