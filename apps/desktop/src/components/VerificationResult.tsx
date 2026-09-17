import {ListenerObservation} from './ListenerObservation';
const object=(v:unknown):Record<string,unknown>|undefined=>v!==null&&typeof v==='object'&&!Array.isArray(v)?v as Record<string,unknown>:undefined;
/** Render explicit adapter observations; never derive a readiness percentage. */
export function VerificationResult({value}:{value:unknown}){
 const outer=object(value);const result=object(outer?.adapterResult)??outer;
 if(!result)return <p>{typeof value==='string'?value:'No verification evidence recorded.'}</p>;
 const facts=Array.isArray(result.facts)?result.facts.map(object).filter(f=>f&&typeof f.capability==='string'&&typeof f.status==='string'):[];
 const services=Array.isArray(result.services)?result.services.map(object).filter(Boolean):[];
 return <div className="verification">
  {!!facts.length&&<ul>{facts.map((f,i)=><li key={i}><strong>{String(f!.capability)}</strong><span>{Number.isInteger(f!.observed)&&Number.isInteger(f!.expected)?`${f!.observed}/${f!.expected}  |  `:''}{String(f!.status)}</span></li>)}</ul>}
  {services.map((s,i)=>{const o=object(s!.observation);return <div key={i}><p><strong>{String(s!.name??'Service')}</strong>  |  {String(o?.status??'unverified')}<small>{String(o?.url??'')}{typeof o?.httpStatus==='number'?`  |  HTTP ${o.httpStatus}`:''}</small><small>{String(o?.detail??'No endpoint evidence recorded.')}</small></p><ListenerObservation value={o?.listenerOwnership}/></div>;})}
  {typeof result.status==='string'&&<p><strong>{result.status}</strong>{Number.isInteger(result.expected)&&Number.isInteger(result.observed)?`  |  ${result.observed}/${result.expected}`:''}</p>}
  {typeof result.detail==='string'&&<p>{result.detail}</p>}
  {Array.isArray(result.details)&&result.details.filter(d=>typeof d==='string').map((d,i)=><small key={i}>{d}</small>)}
  <details><summary>Full observation</summary><pre>{JSON.stringify(value,null,2)}</pre></details>
 </div>;
}
