import {useEffect, useRef, useState} from 'react';
import type {Snapshot} from '../../../../packages/core/model';
import {Badge, Evidence, Icon} from './ui';

/** Single-window review. Same fail-closed rules as the workspace review. */
export function ReviewDialog({snapshot, onCancel, onApprove}: {snapshot: Snapshot; onCancel(): void; onApprove(quick: boolean): void}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [saved, setSaved] = useState(false);
  const [quick, setQuick] = useState(false);
  useEffect(() => {dialog.current?.showModal(); return () => dialog.current?.close();}, []);
  const body = snapshot.body as {tabs?: {title: string; url: string}[]; dirty?: boolean; terminals?: unknown[]; groups?: unknown[]};
  const browser = snapshot.adapter === 'chrome';
  const blocked = !browser && (body.dirty || !!body.terminals?.length);
  return <dialog className="sheet review-sheet" ref={dialog} onCancel={onCancel} aria-labelledby="review-title">
    <header className="sheet-head">
      <div>
        <p className="eyebrow">Final safety check</p>
        <h2 id="review-title">Sleep this {browser ? 'Chrome' : 'VS Code'} window</h2>
        <small>{browser ? `${body.tabs?.length ?? 0} tabs saved with their URLs, order, pins and groups` : 'Workspace identity and editor context saved'}</small>
      </div>
      <button type="button" className="btn btn-icon" onClick={onCancel} aria-label="Close review"><Icon name="close" /></button>
    </header>

    <div className="sheet-body">
      <p className="sheet-lede">{browser
        ? 'Background tabs come back lazily when you select them. Page contents, forms and sign-in state are not stored, so save your work in those sites first.'
        : 'Unsaved editors, running tasks, debugging, custom editors and existing terminal sessions block closing. Finish them in VS Code and capture again.'}</p>

      {blocked && <p className="inline-alert is-bad" role="alert"><Icon name="alert" />This capture contains unsaved work or terminal sessions. Closing is blocked.</p>}

      {!!body.tabs?.length && <Evidence label={`Saved destinations (${body.tabs.length})`}>
        <ul className="destination-list">{body.tabs.map((t, i) => <li key={i}><strong>{t.title || 'Untitled tab'}</strong><small>{t.url}</small></li>)}</ul>
      </Evidence>}

      {browser && <label className="option-row">
        <input type="checkbox" checked={quick} onChange={e => setQuick(e.target.checked)} />
        <span>
          <strong>Quick Sleep instead<Badge tone="info">keeps the window open</Badge></strong>
          <small>Discards eligible background pages only. Active, pinned and protected tabs stay loaded; selecting a sleeping tab reloads it.</small>
        </span>
      </label>}
    </div>

    <footer className="sheet-foot">
      <label className="consent">
        <input type="checkbox" checked={saved} onChange={e => setSaved(e.target.checked)} />
        <span>I saved the work in this window and reviewed what can be reconstructed.</span>
      </label>
      <small className="sheet-fineprint">Approval applies once to this capture. Changed tabs, downloads or an expired capture need a fresh review.</small>
      <div className="sheet-actions">
        <button type="button" className="btn btn-ghost" onClick={onCancel}>Keep working</button>
        <button type="button" className="btn btn-primary" disabled={!saved || !!blocked} onClick={() => onApprove(quick)}>
          <Icon name="moon" />{quick ? 'Quick Sleep tabs' : 'Sleep this window'}
        </button>
      </div>
    </footer>
  </dialog>;
}
