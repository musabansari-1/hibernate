import {useState} from 'react';
import {Icon} from './ui';

export type ServiceCommand = {executable: string; arguments: string[]; cwd: string};

export function ServiceLaunch({service, cwd, busy, configure, start}: {service: {id: string; launch?: ServiceCommand; launchRun?: {state: string; detail?: string; verified?: boolean}}; cwd: string; busy: boolean; configure: (definition: ServiceCommand) => Promise<void>; start: (definition: ServiceCommand) => Promise<void>}) {
  const [executable, setExecutable] = useState(service.launch?.executable ?? '');
  const [args, setArgs] = useState(JSON.stringify(service.launch?.arguments ?? ['server.js']));
  const [directory, setDirectory] = useState(service.launch?.cwd ?? cwd);
  const [error, setError] = useState('');
  const [approved, setApproved] = useState(false);
  return <details className="panel-details">
    <summary>
      <span>Startup command</span>
      <small>{service.launch ? 'Saved - review before running' : 'Not configured'}</small>
    </summary>
    <div className="panel-details-body">
      <p className="muted-line">Direct Node or Python services only. Enter the runtime and literal arguments; shell commands and package-manager wrappers are not supported. Saving does not run anything.</p>
      <form className="field-grid" onSubmit={e => {
        e.preventDefault(); setError(''); setApproved(false);
        try {
          const argumentsValue: unknown = JSON.parse(args);
          if (!Array.isArray(argumentsValue) || argumentsValue.some(v => typeof v !== 'string')) throw Error('Arguments must be a JSON array of strings.');
          void configure({executable, arguments: argumentsValue as string[], cwd: directory});
        } catch (e) {setError(String(e));}
      }}>
        <label>Runtime executable<input required value={executable} onChange={e => setExecutable(e.target.value)} placeholder="C:\Program Files\nodejs\node.exe" /></label>
        <label>Literal arguments <em>JSON array</em><input required value={args} onChange={e => setArgs(e.target.value)} /></label>
        <label>Working directory<input required value={directory} onChange={e => setDirectory(e.target.value)} /></label>
        <button type="submit" className="btn btn-quiet" disabled={busy}>Save startup command</button>
      </form>
      {error && <p className="inline-alert is-bad" role="alert"><Icon name="alert" />{error}</p>}
      {service.launch && <>
        <p className="muted-line">Saved command:</p>
        <pre>{JSON.stringify(service.launch, null, 2)}</pre>
        <p className="muted-line">This runs project code in its own console with your account and inherited environment. Approve only a development service you trust. Startup counts as verified only when this runtime uniquely answers the healthy endpoint. A later ContextSleep run may ask this exact verified process to stop gracefully; it never force-terminates an unverified service.</p>
        <label className="consent">
          <input type="checkbox" disabled={busy} checked={approved} onChange={e => setApproved(e.target.checked)} />
          <span>I reviewed this saved command and approve running it now.</span>
        </label>
        <button type="button" className="btn btn-quiet" disabled={busy || !approved} onClick={() => {setApproved(false); void start(service.launch!);}}>Start and verify</button>
      </>}
      {service.launchRun && <p className="muted-line"><strong>{service.launchRun.state}</strong> - {service.launchRun.detail ?? 'Launch intent saved; inspect the process before retrying.'}</p>}
    </div>
  </details>;
}
