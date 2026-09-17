import React,{useState} from 'react';
export type ServiceCommand={executable:string;arguments:string[];cwd:string};
export function ServiceLaunch({service,cwd,busy,configure,start}:{service:{id:string;launch?:ServiceCommand;launchRun?:{state:string;detail?:string;verified?:boolean}};cwd:string;busy:boolean;configure:(definition:ServiceCommand)=>Promise<void>;start:(definition:ServiceCommand)=>Promise<void>}){
 const [executable,setExecutable]=useState(service.launch?.executable??'');
 const [args,setArgs]=useState(JSON.stringify(service.launch?.arguments??['server.js']));
 const [directory,setDirectory]=useState(service.launch?.cwd??cwd);
 const [error,setError]=useState('');const [approved,setApproved]=useState(false);
 return <details><summary>Approved service startup</summary><p>Direct Node or Python services only. Enter the runtime and literal arguments; shell commands and package-manager wrappers are not supported. Saving does not run anything.</p>
 <form className="fields" onSubmit={e=>{e.preventDefault();setError('');setApproved(false);try{const argumentsValue:unknown=JSON.parse(args);if(!Array.isArray(argumentsValue)||argumentsValue.some(v=>typeof v!=='string'))throw Error('Arguments must be a JSON array of strings.');void configure({executable,arguments:argumentsValue as string[],cwd:directory});}catch(e){setError(String(e));}}}>
 <label>Runtime executable<input required value={executable} onChange={e=>setExecutable(e.target.value)} placeholder="C:\Program Files\nodejs\node.exe"/></label>
 <label>Literal arguments (JSON array)<input required value={args} onChange={e=>setArgs(e.target.value)}/></label>
 <label>Working directory<input required value={directory} onChange={e=>setDirectory(e.target.value)}/></label><button disabled={busy}>Save startup definition</button></form>
 {error&&<p role="alert">{error}</p>}
 {service.launch&&<><p>Saved command to run:</p><pre>{JSON.stringify(service.launch,null,2)}</pre><p>This executes project code in a dedicated console with your account and inherited environment. Approve only a development service you trust. Startup is verified only when this runtime uniquely owns the healthy endpoint. A later ContextSleep run may request graceful shutdown only for this exact verified process; it never force-terminates an unverified service.</p>
 <label><input type="checkbox" disabled={busy} checked={approved} onChange={e=>setApproved(e.target.checked)}/>I reviewed this saved command and approve running it now.</label><button disabled={busy||!approved} onClick={()=>{setApproved(false);void start(service.launch!);}}>Start approved service and verify</button></>}
 {service.launchRun&&<p><strong>{service.launchRun.state}</strong>  |  {service.launchRun.detail??'Launch intent saved; inspect the process before retrying.'}</p>}
 </details>;
}
