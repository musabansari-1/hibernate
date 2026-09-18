import type {Dashboard} from '../../../../packages/core/model';
import {Icon} from './ui';

type Props = {
  data: Dashboard;
  onClose(): void;
  onGo(id: string): void;
};

type Step = {
  title: string;
  detail: string;
  done: boolean;
  target: string;
  optional?: boolean;
};

/**
 * First-run checklist. A step turns green only when ContextSleep can observe the
 * setup, never because the user clicked something.
 */
export function SetupGuide({data, onClose, onGo}: Props) {
  const projectId = data.workspace.id;
  const connectedReports = data.reports.filter(r => Date.now() - r.at < 15000);
  const chrome = connectedReports.find(r => r.adapter === 'chrome');
  const assigned = data.memberships.some(m => m.projectId === projectId);
  const chromeBound = !chrome || data.chromeLaunches.some(v => v.client === chrome.client);
  const steps: Step[] = [
    {title: 'Pick the workspace folder', detail: data.workspace.projectPath ? data.workspace.projectPath : 'Tell ContextSleep which local folder this workspace lives in.', done: !!data.workspace.projectPath.trim(), target: 'workspace-settings'},
    {title: 'Connect an app', detail: connectedReports.length ? `${connectedReports.length} app${connectedReports.length === 1 ? '' : 's'} reporting now.` : 'Open the bundled Chrome or VS Code integration so ContextSleep can read real workspace context.', done: connectedReports.length > 0, target: 'apps'},
    {title: 'Assign a window', detail: assigned ? 'At least one window belongs to this workspace.' : 'Assign a Chrome or VS Code window to this workspace before capturing.', done: assigned, target: 'apps'},
    {title: 'Bind the Chrome profile', detail: !chrome ? 'Only needed when Chrome is part of this workspace.' : chromeBound ? 'The connected Chrome profile can be relaunched exactly.' : 'Save the profile folder name so ContextSleep can relaunch this exact profile later.', done: chromeBound, target: 'apps', optional: !chrome},
    {title: 'Capture your first checkpoint', detail: data.checkpoints.length ? 'A durable checkpoint exists for this workspace.' : 'Select the assigned windows and capture. Nothing closes until you review it.', done: data.checkpoints.length > 0, target: 'checkpoint'},
  ];
  const required = steps.filter(s => !s.optional);
  const doneCount = required.filter(s => s.done).length;
  const complete = doneCount === required.length;

  return <section className="card setup-guide" aria-labelledby="setup-guide-title">
    <header className="setup-head">
      <div>
        <p className="eyebrow">Get set up</p>
        <h2 id="setup-guide-title">{complete ? 'This workspace is sleep-ready' : 'Make this workspace sleep-ready'}</h2>
        <p className="section-description">{complete
          ? 'Capture a fresh checkpoint whenever the workspace changes, then review it before sleeping.'
          : 'ContextSleep checks each item live. A step turns green only when it can actually observe the setup.'}</p>
      </div>
      <div className="setup-progress">
        <strong>{doneCount}<span>/{required.length}</span></strong>
        <button type="button" className="btn btn-ghost" onClick={onClose}>Hide</button>
      </div>
    </header>
    <ol className="setup-steps">
      {steps.map((step, i) => <li key={step.title}>
        <button type="button" className={`setup-step ${step.done ? 'is-done' : ''}`} onClick={() => onGo(step.target)}>
          <span className="setup-index" aria-hidden="true">{step.done ? <Icon name="check" /> : i + 1}</span>
          <span className="setup-copy">
            <strong>{step.title}{step.optional ? <em> optional</em> : ''}</strong>
            <small>{step.detail}</small>
          </span>
          <Icon name="chevron" className="setup-chevron" />
        </button>
      </li>)}
    </ol>
    <small className="setup-note">Local development services are optional. If you add one, ContextSleep requires explicit command review before it ever starts or restarts it.</small>
  </section>;
}
