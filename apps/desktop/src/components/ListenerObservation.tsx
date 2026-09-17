const object=(v:unknown):Record<string,unknown>|undefined=>v!==null&&typeof v==='object'&&!Array.isArray(v)?v as Record<string,unknown>:undefined;
export function ListenerObservation({value}:{value:unknown}){
 const data=object(value);if(!data)return null;
 const listeners=Array.isArray(data.listeners)?data.listeners.map(object).filter(Boolean):[];
 return <div className="listener-observation">
  {!listeners.length&&<small>Listening process {data.status==='unavailable'?'unavailable':'not observed'}.</small>}
  {listeners.slice(0,8).map((p,i)=>{const identity=object(p!.identity);const path=typeof identity?.executable==='string'?identity.executable:undefined;return <small key={i}>Listener: {path?.split(/[\\/]/).pop()??'executable unavailable'}{typeof p!.pid==='number'?`  |  PID ${p!.pid}`:''}</small>;})}
  <small>Project ownership remains unverified.</small>
  <details><summary>Listener evidence</summary><p>{String(data.detail??'No listener details recorded.')}</p><pre>{JSON.stringify(data,null,2)}</pre></details>
 </div>;
}
