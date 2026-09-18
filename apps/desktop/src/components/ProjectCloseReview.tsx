import {useEffect, useRef, useState} from 'react';
import type {HibernationRun} from '../../../../packages/core/model';
import {Badge, Evidence, Icon} from './ui';
import {dateTime, plural} from '../lib/format';

/**
 * The last thing a user sees before anything closes. It states exactly what will
 * close, what will be kept open, and what cannot be reconstructed.
 */
export function ProjectCloseReview({review, onCancel, onApprove}: {review: HibernationRun; onCancel(): void; onApprove(selected: string[]): void}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const eligible = review.body.steps.filter(s => s.eligible);
  const retained = review.body.steps.filter(s => !s.eligible);
  // Fail closed: nothing is pre-selected for closing. The user opts each window in.
  const [selected, setSelected] = useState<string[]>([]);
  const [saved, setSaved] = useState(false);
  useEffect(() => {dialog.current?.showModal(); return () => dialog.current?.close();}, []);

  return <dialog className="sheet review-sheet" ref={dialog} onCancel={onCancel} aria-labelledby="project-close-title">
    <header className="sheet-head">
      <div>
        <p className="eyebrow">Final safety check</p>
        <h2 id="project-close-title">Review what may close</h2>
        <small>{review.body.project.name} - {review.body.project.projectPath || 'No folder saved'}</small>
      </div>
      <button type="button" className="btn btn-icon" onClick={onCancel} aria-label="Close review"><Icon name="close" /></button>
    </header>

    <div className="sheet-body">
      <p className="sheet-lede">
        Each selected window is checked again immediately before it closes. If anything changed, looks unsafe, or cannot be verified,
        ContextSleep stops the sequence instead of guessing.
      </p>
      <p className="sheet-warning">
        <Icon name="alert" />
        Save your work first. Form contents, in-page application state and sign-in state are not reconstructed, and any native
        unsaved-work prompt stays under the application's control.
      </p>

      {!!eligible.length && <section className="review-group">
        <h3><Badge tone="good" dot>Safe to sleep</Badge>{plural(eligible.length, 'window')}</h3>
        <ul className="review-list">
          {eligible.map(s => {
            const body = s.snapshot?.body as {tabs?: {title: string; url: string}[]; groups?: {tabs: {label: string; uri?: string | null}[]}[]} | undefined;
            return <li key={s.id}>
              <label className="review-row">
                <input type="checkbox" checked={selected.includes(s.id)} onChange={e => setSelected(v => e.target.checked ? [...v, s.id] : v.filter(id => id !== s.id))} />
                <span className="review-row-copy">
                  <strong>{s.label}</strong>
                  <small>{s.detail}</small>
                  {s.snapshot && <small>Captured {dateTime(s.snapshot.created)}</small>}
                </span>
              </label>
              {s.snapshot?.adapter === 'chrome' && body?.tabs && <Evidence label={`Saved destinations (${body.tabs.length})`}>
                <ul className="destination-list">{body.tabs.map((t, i) => <li key={i}><strong>{t.title || 'Untitled tab'}</strong><small>{t.url}</small></li>)}</ul>
              </Evidence>}
              {s.snapshot?.adapter === 'vscode' && body?.groups && <Evidence label="Saved editors">
                <ul className="destination-list">{body.groups.flatMap(g => g.tabs).map((t, i) => <li key={i}><strong>{t.label}</strong><small>{t.uri ?? 'Unsupported editor'}</small></li>)}</ul>
              </Evidence>}
            </li>;
          })}
        </ul>
      </section>}

      {!!retained.length && <section className="review-group is-retained">
        <h3><Badge tone="warn" dot>Kept open</Badge>{plural(retained.length, 'item')}</h3>
        <p className="group-note">ContextSleep cannot safely identify or close these, so it leaves them alone. They may keep using memory.</p>
        <ul className="review-list">
          {retained.map(s => <li key={s.id}><div className="review-row is-static">
            <span className="review-row-copy"><strong>{s.label}</strong><small>{s.detail}</small></span>
          </div></li>)}
        </ul>
      </section>}

      {!eligible.length && <p className="inline-alert is-warn"><Icon name="info" />Nothing in this checkpoint can be closed safely right now. Everything stays open.</p>}
    </div>

    <footer className="sheet-foot">
      <label className="consent">
        <input type="checkbox" checked={saved} onChange={e => setSaved(e.target.checked)} />
        <span>I saved the work in every selected window and reviewed what can be reconstructed.</span>
      </label>
      <small className="sheet-fineprint">Approval is single use. Expired or changed captures need a fresh capture.</small>
      <div className="sheet-actions">
        <button type="button" className="btn btn-ghost" onClick={onCancel}>Keep working</button>
        <button type="button" className="btn btn-quiet" disabled={!eligible.length} onClick={() => setSelected(eligible.map(s => s.id))}>Select all safe</button>
        <button type="button" className="btn btn-primary" disabled={!saved || !selected.length} onClick={() => onApprove(selected)}>
          <Icon name="moon" />{selected.length ? `Sleep ${plural(selected.length, 'window')}` : 'Sleep selected windows'}
        </button>
      </div>
    </footer>
  </dialog>;
}
