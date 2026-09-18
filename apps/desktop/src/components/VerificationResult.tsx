import {ListenerObservation} from './ListenerObservation';
import {Badge, Evidence, Icon} from './ui';
import {stepStateLabel} from '../lib/format';

const object = (v: unknown): Record<string, unknown> | undefined =>
  v !== null && typeof v === 'object' && !Array.isArray(v) ? v as Record<string, unknown> : undefined;

function factTone(status: string) {
  if (status === 'verified') return {label: 'Verified', tone: 'good' as const};
  if (status === 'missing') return {label: 'Not found', tone: 'bad' as const};
  if (status === 'unsupported') return {label: 'Not supported', tone: 'warn' as const};
  if (status === 'deferred') return {label: 'Deferred', tone: 'info' as const};
  return stepStateLabel(status);
}

/** Render explicit adapter observations; never derive a readiness percentage. */
export function VerificationResult({value}: {value: unknown}) {
  const outer = object(value);
  const result = object(outer?.adapterResult) ?? outer;
  if (!result) return <p className="muted-line">{typeof value === 'string' ? value : 'No verification evidence recorded.'}</p>;
  const facts = Array.isArray(result.facts) ? result.facts.map(object).filter(f => f && typeof f.capability === 'string' && typeof f.status === 'string') : [];
  const services = Array.isArray(result.services) ? result.services.map(object).filter(Boolean) : [];
  return <div className="verification">
    {!!facts.length && <ul className="fact-list">
      {facts.map((f, i) => {
        const state = factTone(String(f!.status));
        return <li key={i}>
          <span className="fact-name"><Icon name={state.tone === 'good' ? 'check' : 'info'} />{String(f!.capability)}</span>
          <span className="fact-value">
            {Number.isInteger(f!.observed) && Number.isInteger(f!.expected) && <em>{String(f!.observed)} of {String(f!.expected)}</em>}
            <Badge tone={state.tone}>{state.label}</Badge>
          </span>
        </li>;
      })}
    </ul>}
    {services.map((s, i) => {
      const o = object(s!.observation);
      return <div key={i} className="service-observation">
        <div className="service-observation-head">
          <strong>{String(s!.name ?? 'Service')}</strong>
          <Badge tone={String(o?.status) === 'healthy' ? 'good' : 'warn'}>{String(o?.status ?? 'unverified')}</Badge>
        </div>
        <small>{String(o?.url ?? '')}{typeof o?.httpStatus === 'number' ? ` - HTTP ${o.httpStatus}` : ''}</small>
        <small>{String(o?.detail ?? 'No endpoint evidence recorded.')}</small>
        <ListenerObservation value={o?.listenerOwnership} />
      </div>;
    })}
    {typeof result.status === 'string' && <p className="muted-line"><strong>{result.status}</strong>{Number.isInteger(result.expected) && Number.isInteger(result.observed) ? ` - ${result.observed} of ${result.expected}` : ''}</p>}
    {typeof result.detail === 'string' && <p className="muted-line">{result.detail}</p>}
    {Array.isArray(result.details) && result.details.filter(d => typeof d === 'string').map((d, i) => <small key={i} className="muted-line">{d}</small>)}
    <Evidence label="Full observation"><pre>{JSON.stringify(value, null, 2)}</pre></Evidence>
  </div>;
}
