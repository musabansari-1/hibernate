import {useState} from 'react';
export function ChromeColdStart({binding,busy,save}:{binding?:{profileDirectory:string;executable:string;userDataDir?:string|null};busy:boolean;save(profileDirectory:string,executable:string,userDataDir:string):Promise<void>}){
 const [profileDirectory,setProfileDirectory]=useState(binding?.profileDirectory??'');
 const [executable,setExecutable]=useState(binding?.executable??'');
 const [userDataDir,setUserDataDir]=useState(binding?.userDataDir??'');
 return <details><summary>Cold-start profile binding</summary>
  <p>ContextSleep does not inspect Chrome's private profile folders or command line. Save the profile directory name once so this exact enrolled profile can be launched after a restart.</p>
  <div className="fields">
   <label>Chrome profile directory<input value={profileDirectory} onChange={e=>setProfileDirectory(e.target.value)} placeholder="Default or Profile 1"/></label>
   <label>chrome.exe path (optional if Chrome Stable is installed normally)<input value={executable} onChange={e=>setExecutable(e.target.value)} placeholder="C:\Program Files\Google\Chrome\Application\chrome.exe"/></label>
   <label>User-data root (optional; only for portable/custom Chrome data)<input value={userDataDir} onChange={e=>setUserDataDir(e.target.value)} placeholder="D:\ChromeData"/></label>
   <button disabled={busy||!profileDirectory.trim()} onClick={()=>void save(profileDirectory,executable,userDataDir)}>Save profile binding</button>
  </div>
  {binding&&<small>Saved for cold start: {binding.profileDirectory}  |  {binding.executable}{binding.userDataDir?`  |  data ${binding.userDataDir}`:''}</small>}
 </details>;
}
