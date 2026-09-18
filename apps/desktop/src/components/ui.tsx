import type {ReactNode} from 'react';
import type {Tone} from '../lib/format';

export function Badge({tone = 'neutral', children, dot}: {tone?: Tone; children: ReactNode; dot?: boolean}) {
  return <span className={`badge tone-${tone}`}>{dot && <i className="badge-dot" aria-hidden="true" />}{children}</span>;
}

export function Card({className = '', children, ...rest}: {className?: string; children: ReactNode} & React.HTMLAttributes<HTMLDivElement>) {
  return <div className={`card ${className}`} {...rest}>{children}</div>;
}

export function SectionHead({eyebrow, title, description, action, id}: {eyebrow?: string; title: string; description?: string; action?: ReactNode; id?: string}) {
  return <div className="section-head">
    <div className="section-head-copy">
      {eyebrow && <p className="eyebrow">{eyebrow}</p>}
      <h2 id={id}>{title}</h2>
      {description && <p className="section-description">{description}</p>}
    </div>
    {action && <div className="section-head-action">{action}</div>}
  </div>;
}

export function Empty({title, body, action}: {title: string; body: string; action?: ReactNode}) {
  return <div className="empty">
    <Icon name="moon" className="empty-icon" />
    <strong>{title}</strong>
    <p>{body}</p>
    {action}
  </div>;
}

/** Technical evidence stays available, one click away, never in the default view. */
export function Evidence({label = 'Technical evidence', children}: {label?: string; children: ReactNode}) {
  return <details className="evidence-details"><summary>{label}</summary><div className="evidence-body">{children}</div></details>;
}

export function Meter({percent, tone = 'good', label}: {percent: number; tone?: Tone; label?: string}) {
  const clamped = Math.max(0, Math.min(100, percent));
  return <div className="meter" role="img" aria-label={label ?? `${Math.round(clamped)} percent`}>
    <div className={`meter-fill tone-${tone}`} style={{width: `${clamped}%`}} />
  </div>;
}

const paths: Record<string, ReactNode> = {
  check: <path d="M4 8.6 6.8 11.4 12 5.4" />,
  moon: <path d="M13.2 9.6A5.2 5.2 0 0 1 6.4 2.8a5.6 5.6 0 1 0 6.8 6.8Z" />,
  arrow: <><path d="M3 8h9.5" /><path d="M9 4.5 12.5 8 9 11.5" /></>,
  chevron: <path d="M6 4l4 4-4 4" />,
  spark: <><path d="M8 2v3" /><path d="M8 11v3" /><path d="M2 8h3" /><path d="M11 8h3" /><path d="m4.3 4.3 2 2" /><path d="m9.7 9.7 2 2" /><path d="m11.7 4.3-2 2" /><path d="m6.3 9.7-2 2" /></>,
  shield: <path d="M8 2 3.5 3.8v3.9c0 2.7 1.8 5.2 4.5 6.3 2.7-1.1 4.5-3.6 4.5-6.3V3.8Z" />,
  lock: <><rect x="3.5" y="7" width="9" height="6.5" rx="1.6" /><path d="M5.8 7V5.3a2.2 2.2 0 0 1 4.4 0V7" /></>,
  layers: <><path d="M8 2.2 14 5.5 8 8.8 2 5.5Z" /><path d="m2.6 8.6 5.4 3 5.4-3" /></>,
  window: <><rect x="2.5" y="3" width="11" height="10" rx="2" /><path d="M2.5 6h11" /></>,
  plus: <><path d="M8 3.5v9" /><path d="M3.5 8h9" /></>,
  refresh: <><path d="M13 8a5 5 0 1 1-1.6-3.7" /><path d="M13.4 2.6v2.6h-2.6" /></>,
  history: <><circle cx="8" cy="8" r="5.6" /><path d="M8 4.8V8l2.2 1.4" /></>,
  settings: <><circle cx="8" cy="8" r="2.2" /><path d="M8 1.8v1.6M8 12.6v1.6M14.2 8h-1.6M3.4 8H1.8m10.4-4.4-1.1 1.1M5.3 10.7l-1.1 1.1m0-7.6 1.1 1.1m5.4 5.4 1.1 1.1" /></>,
  alert: <><path d="M8 2.6 14 13H2Z" /><path d="M8 6.6v3M8 11.2v.1" /></>,
  info: <><circle cx="8" cy="8" r="5.8" /><path d="M8 7.2v3.4M8 5.2v.1" /></>,
  search: <><circle cx="7.2" cy="7.2" r="4.4" /><path d="m10.6 10.6 2.7 2.7" /></>,
  folder: <path d="M2.5 12.4V4.2a1 1 0 0 1 1-1h2.6l1.4 1.6h5a1 1 0 0 1 1 1v6.6a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1Z" />,
  download: <><path d="M8 2.6v7.2" /><path d="m5 7.2 3 3 3-3" /><path d="M3 12.6h10" /></>,
  close: <><path d="m4.4 4.4 7.2 7.2" /><path d="m11.6 4.4-7.2 7.2" /></>,
};

export function Icon({name, className = ''}: {name: keyof typeof paths | string; className?: string}) {
  return <svg className={`icon ${className}`} viewBox="0 0 16 16" fill="none" stroke="currentColor"
    strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
    {paths[name] ?? paths.info}
  </svg>;
}
