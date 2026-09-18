import type {Memory, Snapshot} from '../../../../packages/core/model';
import {useState} from 'react';
import {ReviewDialog} from './ReviewDialog';
import {VerificationResult} from './VerificationResult';
import {Badge, Evidence, Icon} from './ui';
import {appLabel, dateTime, gbExact, relativeTime, signedGb, snapshotStateLabel} from '../lib/format';

type Props = {
  snapshot: Snapshot;
  busy: boolean;
  connected: boolean;
  resume(): void;
  openVSCode(): void;
  restoreContext(kind: string, index: number): void;
  closeReviewed(quick: boolean): void;
  archive(): void;
  exportBackup(): void;
  deleteSnapshot(confirmation: string): void;
};

function Measurements({before, after, chrome}: {before: Memory; after: Memory; chrome: boolean}) {
  const attributed = before.sourceBrowserPid === after.sourceBrowserPid
    && before.sourceBrowserWorkingSet != null && after.sourceBrowserWorkingSet != null;
  const change = after.availablePhysical - before.availablePhysical;
  return <div className="measurement">
    <p className="measurement-headline">
      <strong className={change >= 0 ? 'is-up' : 'is-down'}>{signedGb(change)}</strong>
      system available memory across this operation
    </p>
    {attributed && <div className="measurement-rows">
      <span>Source browser working sets <b>{gbExact(before.sourceBrowserWorkingSet! - after.sourceBrowserWorkingSet!)} lower</b></span>
      <span>Source browser private commit <b>{gbExact(before.sourceBrowserPrivateCommit! - after.sourceBrowserPrivateCommit!)} lower</b></span>
      <small>That process tree can include other windows and profiles. Unreadable source processes: {before.sourceBrowserUnreadable} before, {after.sourceBrowserUnreadable} after.</small>
    </div>}
    {chrome && <Evidence label="All Chrome process observations">
      <p>{gbExact(before.chromeWorkingSet - after.chromeWorkingSet)} working-set decrease</p>
      <p>{gbExact(before.chromePrivateCommit - after.chromePrivateCommit)} private-commit decrease</p>
      <small>Unreadable Chrome processes: {before.unreadableProcesses} before, {after.unreadableProcesses} after.</small>
    </Evidence>}
    <small className="measurement-note">Signed observations, not exclusive workspace savings. Working sets can count shared pages more than once, and private commit is not physical RAM.</small>
  </div>;
}

export function SnapshotCard({snapshot: s, busy, connected, resume, openVSCode, restoreContext, closeReviewed, archive, exportBackup, deleteSnapshot}: Props) {
  const [review, setReview] = useState(false);
  const [confirmation, setConfirmation] = useState('');
  if (s.adapter === 'unreadable') return <article className="card snapshot is-unreadable">
    <h3>This saved context could not be read</h3>
    <p className="muted-line">Your other saved contexts are unaffected. Restore is blocked for this record.</p>
    <Evidence label="Record"><pre>{JSON.stringify(s.body, null, 2)}</pre></Evidence>
  </article>;

  const body = s.body as {
    tabs?: unknown[];
    project?: {name: string};
    explorer?: {items: {url: string}[]};
    terminal?: {cwd?: string; items?: {cwd: string; source?: string; verifiedPrompt?: boolean}[]};
  };
  const label = appLabel(s.adapter);
  const state = s.adapter === 'chrome' && s.state === 'verified'
    ? {label: 'Organization verified', tone: 'good' as const}
    : snapshotStateLabel(s.state);

  return <article className="card snapshot">
    <header className="snapshot-head">
      <div className="snapshot-title">
        <span className="app-chip" data-app={s.adapter} aria-hidden="true">{label.slice(0, 2)}</span>
        <div>
          <strong>{body.project?.name ?? 'Workspace'} <span className="snapshot-app">{label}</span></strong>
          <small>{relativeTime(s.created)} - {body.tabs ? `${body.tabs.length} tabs` : 'workspace metadata'} - {dateTime(s.created)}</small>
        </div>
      </div>
      <Badge tone={state.tone}>{state.label}</Badge>
    </header>

    {s.adapter !== 'context' && <div className="snapshot-actions">
      {!s.archived && ['captured', 'blocked'].includes(s.state) && connected &&
        <button type="button" className="btn btn-quiet" disabled={busy} onClick={() => setReview(true)}><Icon name="moon" />Review and sleep</button>}
      {s.adapter === 'vscode' && !connected &&
        <button type="button" className="btn btn-quiet" disabled={busy} onClick={openVSCode}>Open saved workspace</button>}
      <button type="button" className="btn btn-quiet" disabled={busy || !connected} onClick={resume}><Icon name="refresh" />Restore and verify</button>
    </div>}

    {s.adapter === 'context' && <div className="snapshot-actions">
      {body.explorer?.items.map((item, i) => <button key={i} type="button" className="btn btn-quiet" disabled={busy} title={item.url} onClick={() => restoreContext('explorer', i)}><Icon name="folder" />Open folder {i + 1}</button>)}
      {body.terminal?.items?.map((item, i) => <button key={`t-${i}`} type="button" className="btn btn-quiet" disabled={busy} title={item.source ?? item.cwd} onClick={() => restoreContext('terminal', i)}>Open terminal at {item.cwd}</button>)}
      {!body.terminal?.items?.length && body.terminal?.cwd &&
        <button type="button" className="btn btn-quiet" disabled={busy} onClick={() => restoreContext('terminal', 0)}>Open saved terminal folder</button>}
    </div>}

    {s.adapter !== 'context' && !connected && <p className="muted-line">Connect the original Chrome profile or VS Code workspace integration to restore and verify this.</p>}

    {s.after && <Measurements before={s.before} after={s.after} chrome={s.adapter === 'chrome'} />}
    {s.result != null && <VerificationResult value={s.result} />}
    {review && <ReviewDialog snapshot={s} onCancel={() => setReview(false)} onApprove={quick => {setReview(false); closeReviewed(quick);}} />}

    <div className="snapshot-footer">
      <Evidence label="Saved details"><pre>{JSON.stringify(s.body, null, 2)}</pre></Evidence>
      <Evidence label="Backup and retention">
        <p className="muted-line">Backups are encrypted for this Windows account. Importing a backup never launches or closes applications.</p>
        <div className="row-actions">
          <button type="button" className="btn btn-quiet" disabled={busy} onClick={exportBackup}><Icon name="download" />Export encrypted backup</button>
          <button type="button" className="btn btn-quiet" disabled={busy} onClick={archive}>{s.archived ? 'Return to history' : 'Archive'}</button>
        </div>
        {s.archived && <div className="danger-zone">
          <p>Permanent deletion removes this record from the local database. Exported backups are kept.</p>
          <label>Type the record ID to delete it: <code>{s.id}</code>
            <input aria-label="Deletion confirmation" value={confirmation} onChange={e => setConfirmation(e.target.value)} />
          </label>
          <button type="button" className="btn btn-danger" disabled={busy || confirmation !== s.id} onClick={() => deleteSnapshot(confirmation)}>Delete permanently</button>
        </div>}
      </Evidence>
    </div>
  </article>;
}
