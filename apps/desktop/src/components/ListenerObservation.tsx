import {Evidence} from './ui';

const object = (v: unknown): Record<string, unknown> | undefined =>
  v !== null && typeof v === 'object' && !Array.isArray(v) ? v as Record<string, unknown> : undefined;

/**
 * Which process is answering on the port. ContextSleep shows this without
 * claiming the workspace owns it, because a healthy endpoint is not proof of ownership.
 */
export function ListenerObservation({value}: {value: unknown}) {
  const data = object(value);
  if (!data) return null;
  const listeners = Array.isArray(data.listeners) ? data.listeners.map(object).filter(Boolean) : [];
  return <div className="listener-observation">
    {!listeners.length && <small>Answering process {data.status === 'unavailable' ? 'unavailable' : 'not observed'}.</small>}
    {listeners.slice(0, 8).map((p, i) => {
      const identity = object(p!.identity);
      const path = typeof identity?.executable === 'string' ? identity.executable : undefined;
      return <small key={i}>Answering: {path?.split(/[\/]/).pop() ?? 'executable unavailable'}{typeof p!.pid === 'number' ? ` - PID ${p!.pid}` : ''}</small>;
    })}
    <small>ContextSleep has not proven this workspace owns the process.</small>
    <Evidence label="Port evidence">
      <p>{String(data.detail ?? 'No details recorded.')}</p>
      <pre>{JSON.stringify(data, null, 2)}</pre>
    </Evidence>
  </div>;
}
