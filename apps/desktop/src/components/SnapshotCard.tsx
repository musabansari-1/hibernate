import type { Memory, Snapshot } from '../../../../packages/core/model';
import {useState} from 'react';
import {ReviewDialog} from './ReviewDialog';
import {VerificationResult} from './VerificationResult';
const gib = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(2)} GiB`;
const describe = (value: unknown) => typeof value === 'string' ? value : JSON.stringify(value, null, 2);
type Props = {
    snapshot: Snapshot;
    busy: boolean;
    connected: boolean;
    resume(): void;
    openVSCode(): void;
    restoreContext(kind: string, index: number): void;
    closeReviewed(quick:boolean):void;
    archive():void;
    exportBackup():void;
    deleteSnapshot(confirmation:string):void;
};
function Measurements({ before, after, chrome }: {
    before: Memory;
    after: Memory;
    chrome: boolean;
}) {
    const attributed = before.sourceBrowserPid === after.sourceBrowserPid
        && before.sourceBrowserWorkingSet != null && after.sourceBrowserWorkingSet != null;
    return <div className="measurement">
    <p><strong>{gib(after.availablePhysical - before.availablePhysical)}</strong> change in available system RAM</p>
    {attributed && <>
      <p>{gib(before.sourceBrowserWorkingSet! - after.sourceBrowserWorkingSet!)} decrease in source browser working sets</p>
      <p>{gib(before.sourceBrowserPrivateCommit! - after.sourceBrowserPrivateCommit!)} decrease in source browser private commit</p>
      <small>This browser process tree may include other windows/profiles. Unreadable source processes: {before.sourceBrowserUnreadable} before / {after.sourceBrowserUnreadable} after.</small>
    </>}
    {chrome&&<details><summary>All Chrome process observations</summary>
      <p>{gib(before.chromeWorkingSet - after.chromeWorkingSet)} working-set decrease</p>
      <p>{gib(before.chromePrivateCommit - after.chromePrivateCommit)} private-commit decrease</p>
      <small>Unreadable Chrome processes: {before.unreadableProcesses} before / {after.unreadableProcesses} after.</small>
    </details>}
    <small>Signed observations, not exclusive workspace savings. Working sets can count shared pages more than once. Private commit is not physical RAM.</small>
  </div>;
}
export function SnapshotCard({ snapshot: s, busy, connected, resume, openVSCode, restoreContext,closeReviewed,archive,exportBackup,deleteSnapshot }: Props) {
    const [review,setReview]=useState(false);
    const [confirmation,setConfirmation]=useState('');
    if(s.adapter==='unreadable')return <article className="snapshot"><h3>Snapshot could not be read</h3><p>Other snapshots remain available. Restore is blocked for this record.</p><pre>{describe(s.body)}</pre><small>Snapshot {s.id}</small></article>;
    const body = s.body as {
        tabs?: unknown[];
        project?: {
            name: string;
        };
        explorer?: {
            items: {
                url: string;
            }[];
        };
        terminal?: {
            cwd?: string;
            items?: { cwd: string; source?: string; verifiedPrompt?: boolean }[];
        };
    };
    const label = s.adapter === 'chrome' ? 'Chrome' : s.adapter === 'vscode' ? 'VS Code' : 'Explorer / terminal';
    const status = s.adapter === 'chrome' && s.state === 'verified' ? 'Organization verified' : s.state;
    return <article className="snapshot">
    <div className="section-title">
      <div>
        <h3>{body.project?.name ?? 'Project'}  |  {label} <span className="badge">{status}</span></h3>
        <small>{new Date(s.created).toLocaleString()}  |  {body.tabs ? `${body.tabs.length} tabs` : 'Workspace metadata'}</small>
      </div>
      {s.adapter !== 'context' && <div className="actions">
        {!s.archived&&['captured','blocked'].includes(s.state)&&connected&&<button disabled={busy} onClick={()=>setReview(true)}>Review & sleep</button>}
        {s.adapter === 'vscode' && !connected && <button disabled={busy} onClick={openVSCode}>Open saved project</button>}
        <button disabled={busy || !connected} onClick={resume}>Resume / verify</button>
      </div>}
    </div>
    {s.adapter === 'context' ? <div className="actions">
      {body.explorer?.items.map((item, i) => <button key={i} disabled={busy} title={item.url} onClick={() => restoreContext('explorer', i)}>Restore folder {i + 1}</button>)}
      {body.terminal?.items?.map((item, i) => <button key={`terminal-${i}`} disabled={busy} title={item.source??item.cwd} onClick={() => restoreContext('terminal', i)}>Restore terminal {i + 1}  |  {item.cwd}</button>)}
      {!body.terminal?.items?.length && body.terminal?.cwd && <button disabled={busy} onClick={() => restoreContext('terminal', 0)}>Restore terminal definition</button>}
    </div> : !connected && <p>Connect the original browser profile or VS Code project integration to resume and verify.</p>}
    {s.after && <Measurements before={s.before} after={s.after} chrome={s.adapter==='chrome'}/>}
    {s.result != null && <VerificationResult value={s.result}/>}
    {review&&<ReviewDialog snapshot={s} onCancel={()=>setReview(false)} onApprove={quick=>{setReview(false);closeReviewed(quick);}}/>}
    <details><summary>Inspect captured metadata</summary><pre>{describe(s.body)}</pre></details>
    <details><summary>Backup and retention</summary><p>Backups are encrypted for this Windows account. Importing a backup never launches or closes applications.</p><div className="actions"><button disabled={busy} onClick={exportBackup}>Export encrypted backup</button><button disabled={busy} onClick={archive}>{s.archived?'Return to history':'Archive snapshot'}</button></div>{s.archived&&<div><p>Permanent deletion removes this snapshot from the database. Existing exported backups are retained.</p><label>Type the snapshot ID to delete it: <code>{s.id}</code><input aria-label="Snapshot deletion confirmation" value={confirmation} onChange={e=>setConfirmation(e.target.value)}/></label><button disabled={busy||confirmation!==s.id} onClick={()=>deleteSnapshot(confirmation)}>Permanently delete snapshot</button></div>}</details>
  </article>;
}
