import {Icon} from './ui';

/**
 * The end of the loop. READY is shown only when the engine recorded verified
 * readiness; every other outcome falls back to the plain observation panel.
 */
export function ReadyBanner({detail, facts, at}: {detail: string; facts: {capability: string; status: string}[]; at?: number}) {
  const verified = facts.filter(f => f.status === 'verified').length;
  return <div className="ready-banner">
    <div className="ready-seal" aria-hidden="true">
      <svg viewBox="0 0 64 64">
        <circle className="ready-ring" cx="32" cy="32" r="27" />
        <path className="ready-check" d="M21 33.5 28.5 41 44 24" />
      </svg>
    </div>
    <div className="ready-copy">
      <p className="ready-kicker">Workspace restored</p>
      <h3>Ready</h3>
      <p className="ready-detail">{detail}</p>
      {!!facts.length && <>
        <p className="ready-count">{verified} of {facts.length} capabilities verified by observation{at ? ` at ${new Date(at).toLocaleTimeString()}` : ''}.</p>
        <ul className="ready-facts" aria-label="Verified restore capabilities">
          {facts.map((fact, i) => <li key={`${fact.capability}-${i}`} className={fact.status === 'verified' ? 'is-verified' : ''}>
            <span className="ready-fact-mark" aria-hidden="true">{fact.status === 'verified' ? '✓' : '•'}</span>
            <span>{fact.capability}</span>
          </li>)}
        </ul>
      </>}
    </div>
    <Icon name="spark" className="ready-spark" />
  </div>;
}
