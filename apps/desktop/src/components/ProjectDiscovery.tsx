import {useEffect, useRef, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {ProjectDiscovery as Discovery} from '../../../../packages/core/model';
import {Evidence, Icon} from './ui';

export function ProjectDiscovery({enabled, busy, accept}: {enabled: boolean; busy: boolean; accept: (candidate: Discovery['candidates'][number]) => Promise<void>}) {
  const [result, setResult] = useState<Discovery>();
  const [error, setError] = useState('');
  const [scanning, setScanning] = useState(false);
  const pending = useRef(false);
  async function discover() {
    if (!enabled || pending.current) return;
    pending.current = true; setScanning(true);
    try {setResult(await invoke<Discovery>('discover_projects')); setError('');}
    catch (e) {setError(String(e));}
    finally {pending.current = false; setScanning(false);}
  }
  useEffect(() => {
    void discover();
    const timer = setInterval(() => {if (document.visibilityState === 'visible') void discover();}, 30000);
    return () => clearInterval(timer);
  }, [enabled]);

  return <div className="discovery">
    <div className="row-between">
      <h3>Suggested workspaces</h3>
      <button type="button" className="btn btn-quiet" disabled={!enabled || scanning || busy} onClick={() => void discover()}>
        <Icon name="refresh" />{scanning ? 'Checking folders' : 'Scan again'}
      </button>
    </div>
    <p className="muted-line">Suggestions come from folders your connected apps already report. Pick one to add it, then assign its windows.</p>
    {error && <p className="muted-line" role="status">Scanning unavailable: {error}</p>}
    {result && !result.candidates.length && <p className="muted-line">No suggestion available from the folders currently reported.</p>}
    <ul className="candidate-list">
      {result?.candidates.map(c => <li key={c.projectPath}>
        <div className="candidate-copy">
          <strong>{c.name}{c.repository.branch ? <em>{c.repository.branch}</em> : null}</strong>
          <small>{c.projectPath}</small>
          <small>Reported by {c.evidence.map(e => e.source).join(', ')}</small>
          <Evidence label="Why this folder?">{c.evidence.map(e => <p key={e.source + e.path}>{e.label}<small>{e.path}</small></p>)}</Evidence>
        </div>
        <button type="button" className="btn btn-quiet" disabled={busy || scanning} onClick={() => void accept(c).then(discover)}>
          {c.existingProjectId ? 'Open' : 'Add'}
        </button>
      </li>)}
    </ul>
    {result?.explorerError && <small className="muted-line">Explorer folders unavailable: {result.explorerError}</small>}
    {result && <small className="muted-line">Checked {new Date(result.checkedAt).toLocaleTimeString()}. A localhost tab on its own is not enough to link a folder.</small>}
  </div>;
}
