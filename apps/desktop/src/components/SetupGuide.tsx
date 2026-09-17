import type {Dashboard} from '../../../../packages/core/model';

type Props={
 data:Dashboard;
 onClose():void;
 onGo(id:string):void;
};

type Step={
 title:string;
 detail:string;
 done:boolean;
 target:string;
 optional?:boolean;
};

export function SetupGuide({data,onClose,onGo}:Props){
 const projectId=data.workspace.id;
 const connectedReports=data.reports.filter(r=>Date.now()-r.at<15000);
 const chrome=connectedReports.find(r=>r.adapter==='chrome');
 const assigned=data.memberships.some(m=>m.projectId===projectId);
 const chromeBound=!chrome||data.chromeLaunches.some(v=>v.client===chrome.client);
 const steps:Step[]=[
  {title:'Choose this project folder',detail:data.workspace.projectPath?'Project folder is saved.':'Save the local project folder ContextSleep should reconstruct.',done:!!data.workspace.projectPath.trim(),target:'projects'},
  {title:'Connect an integration',detail:connectedReports.length?`${connectedReports.length} integration${connectedReports.length===1?' is':'s are'} reporting now.`:'Open the bundled Chrome or VS Code integration so ContextSleep can capture semantic state.',done:connectedReports.length>0,target:'applications'},
  {title:'Assign an application window',detail:assigned?'At least one reported window belongs to this project.':'Assign a Chrome or VS Code window to this project before capture.',done:assigned,target:'applications'},
  {title:'Bind Chrome for cold restore',detail:!chrome?'No connected Chrome profile; this is only needed when Chrome is part of the project.':chromeBound?'The connected Chrome profile has an explicit cold-start binding.':'Save the profile directory so ContextSleep can relaunch this exact Chrome profile after a restart.',done:chromeBound,target:'applications',optional:!chrome},
  {title:'Save the first checkpoint',detail:data.checkpoints.length?'A durable project checkpoint exists.':'Select assigned applications and capture a checkpoint before putting the workspace to sleep.',done:data.checkpoints.length>0,target:'project-checkpoints'},
 ];
 const required=steps.filter(s=>!s.optional);
 const complete=required.every(s=>s.done);
 return <section className="setup-guide" aria-labelledby="setup-guide-title">
  <div className="section-title"><div><p className="eyebrow">GET STARTED</p><h2 id="setup-guide-title">Make this workspace sleep-ready</h2></div><div className="actions"><span className="badge">{required.filter(s=>s.done).length}/{required.length} required</span><button onClick={onClose}>Hide guide</button></div></div>
  <p>{complete?'Core setup is complete. Capture a fresh checkpoint whenever the workspace changes, then review it before putting the workspace to sleep.':'Finish these live checks once to make the current project reconstructable. A step turns green only when ContextSleep can observe the required setup.'}</p>
  <div className="setup-steps">{steps.map((step,i)=><button key={step.title} className={`setup-step ${step.done?'done':''}`} onClick={()=>onGo(step.target)}>
   <span className="setup-index" aria-hidden="true">{step.done?'OK':i+1}</span>
   <span><strong>{step.title}{step.optional?'  |  optional':''}</strong><small>{step.detail}</small></span>
  </button>)}</div>
  <small>Developer services are optional. If you add one, ContextSleep will require explicit command review before starting or restarting it.</small>
 </section>;
}
