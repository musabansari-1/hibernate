import {useEffect, useState} from 'react';
import type {ProjectCheckpoint, HibernationRun} from '../../../../packages/core/model';
import {VerificationResult} from './VerificationResult';
import {ProjectCloseReview} from './ProjectCloseReview';
import {ReadyBanner} from './ReadyBanner';
import {MemoryDelta} from './MemoryGauge';
import {Badge, Evidence, Icon} from './ui';
import {captureStateLabel, closeStateLabel, dateTime, plural, relativeTime, stepStateLabel} from '../lib/format';

type Props = {
  checkpoint: ProjectCheckpoint;
  busy: boolean;
  restore(selected: string[], acknowledgedServices: boolean): void;
  reviewClose(): Promise<HibernationRun | undefined>;
  closeReviewed(reviewId: string, selected: string[]): void;
  exportBackup(): void;
};

const closeSummary: Record<string, string> = {
  'closed-selected': 'Every selected window was checked, closed and observed to disappear. Anything ContextSleep could not verify was left open.',
  blocked: 'The final safety check stopped this run. No selected window was closed.',
  interrupted: 'This run was interrupted. ContextSleep did not replay it - check the applications before retrying.',
  partial: 'A close could not be verified, so ContextSleep stopped before the remaining windows.',
  expired: 'This approval was not used in time, so it expired. Nothing was closed. Start a fresh review when you are ready.',
  review: 'An approval is open and waiting for you. Nothing closes until you confirm it.',
};

export function CheckpointCard({checkpoint: c, busy, restore, reviewClose, closeReviewed, exportBackup}: Props) {
  const entries = c.body.entries ?? [];
  const services = c.body.services ?? [];
  // Restore only opens things. Parts are pre-selected for convenience; restarting a
  // saved service command is never pre-selected and still needs explicit approval.
  const [selected, setSelected] = useState<string[]>(() => entries.map(e => e.id));
  const [approvedServices, setApprovedServices] = useState(false);
  const [review, setReview] = useState<HibernationRun>();
  useEffect(() => {
    setSelected(current => {
      const ids = new Set(entries.map(e => e.id));
      const kept = current.filter(id => ids.has(id) || id === 'services');
      return kept.length ? kept : entries.map(e => e.id);
    });
  }, [c.id, entries.length]);

  const lastClose = c.hibernations?.[0];
  const latest = c.runs[0];
  const captured = c.body.captures.filter(v => v.state === 'captured').length;
  const failedCaptures = c.body.captures.filter(v => v.state !== 'captured');
  const asleep = lastClose?.state === 'closed-selected' && (!latest || latest.created < lastClose.created);
  const readiness = latest?.body.readiness;
  const isReady = !!readiness?.verified && latest?.state !== 'interrupted' && (!lastClose || latest.created > lastClose.created);
  const restorable = [...entries.map(e => e.id), ...(services.length ? ['services'] : [])];
  const partsChosen = selected.filter(id => id !== 'services' || approvedServices).length;

  function toggle(id: string, checked: boolean) {
    if (id === 'services' && !checked) setApprovedServices(false);
    setSelected(s => checked ? [...new Set([...s, id])] : s.filter(k => k !== id));
  }

  return <article className={`checkpoint card ${asleep ? 'is-asleep' : ''} ${isReady ? 'is-ready' : ''}`}>
    <header className="checkpoint-head">
      <span className="checkpoint-icon" aria-hidden="true"><Icon name="layers" /></span>
      <div className="checkpoint-title">
        <strong>{c.body.project?.name ?? 'Workspace checkpoint'}</strong>
        <small>Captured {relativeTime(c.created)} - {captured} of {c.body.captures.length} contexts saved</small>
      </div>
      <Badge tone={asleep ? 'info' : isReady ? 'ready' : c.state === 'capturing' ? 'info' : 'good'} dot>
        {c.state === 'capturing' ? 'Capturing' : asleep ? 'Asleep' : isReady ? 'Ready' : 'Saved'}
      </Badge>
    </header>

    {c.body.error && <p className="inline-alert is-bad" role="alert"><Icon name="alert" />{c.body.error}</p>}
    {c.body.importWarnings?.map((warning, i) => <p key={`iw-${i}`} className="inline-alert is-warn" role="status"><Icon name="info" />Imported checkpoint: {warning}</p>)}
    {c.body.services?.filter(s => s.importWarning).map(s => <p key={`sw-${s.id}`} className="inline-alert is-warn" role="status"><Icon name="info" />{s.name}: {s.importWarning}</p>)}
    {failedCaptures.map((v, i) => <p key={`fc-${i}`} className="inline-alert is-warn" role="status"><Icon name="info" />{captureStateLabel(v.state).label}: {v.detail ?? 'No completed capture recorded. Individual saved contexts may still be available in History.'}</p>)}

    {isReady && readiness && <ReadyBanner detail={readiness.detail} facts={readiness.facts} at={latest?.created} />}

    <div className="checkpoint-lanes">
      <section className={`lane lane-sleep ${asleep ? '' : 'is-primary'}`}>
        <h4><Icon name="moon" />Put the workspace to sleep</h4>
        <p>ContextSleep rechecks every selected window immediately before closing it. If ownership or state cannot be established, that window stays open.</p>
        <div className="lane-actions">
          <button type="button" className={`btn ${asleep ? 'btn-quiet' : 'btn-primary'}`} disabled={busy || c.state === 'capturing'} onClick={() => void reviewClose().then(setReview)}>
            <Icon name="shield" />Review and sleep
          </button>
          <button type="button" className="btn btn-ghost" disabled={busy || c.state === 'capturing'} onClick={exportBackup}>Export encrypted copy</button>
        </div>
      </section>

      <section className={`lane lane-restore ${asleep ? 'is-primary' : ''}`}>
        <h4><Icon name="refresh" />Bring it back</h4>
        <p>Chrome restores tabs lazily. Terminals open a fresh shell without replaying commands. Offline VS Code workspaces are reopened so their integration can reconnect.</p>
        <ul className="part-list">
          {entries.map(e => <li key={e.id}>
            <label>
              <input type="checkbox" disabled={busy} checked={selected.includes(e.id)} onChange={v => toggle(e.id, v.target.checked)} />
              <span>{e.label}</span>
            </label>
          </li>)}
          {!!services.length && <li className="part-service">
            <label>
              <input type="checkbox" disabled={busy} checked={selected.includes('services')} onChange={v => toggle('services', v.target.checked)} />
              <span>Restart saved services: {services.map(s => s.name).join(', ')}</span>
            </label>
          </li>}
        </ul>
        {selected.includes('services') && <div className="service-approval">
          <strong>These commands run project code under your Windows account.</strong>
          <p>Review the exact saved definitions before approving a restart.</p>
          {services.map(s => <Evidence key={s.id} label={s.name}><pre>{s.launch ? JSON.stringify(s.launch, null, 2) : 'No saved startup definition'}</pre></Evidence>)}
          <label className="consent">
            <input type="checkbox" disabled={busy || services.some(s => !s.launch)} checked={approvedServices} onChange={e => setApprovedServices(e.target.checked)} />
            <span>I reviewed these saved commands and approve restarting them.</span>
          </label>
        </div>}
        <div className="lane-actions">
          <button type="button" className={`btn ${asleep ? 'btn-primary' : 'btn-quiet'}`} disabled={busy || !selected.length || c.state === 'capturing' || (selected.includes('services') && !approvedServices)} onClick={() => restore(selected, approvedServices)}>
            <Icon name="arrow" />Restore {plural(partsChosen, 'part')}
          </button>
          <button type="button" className="btn btn-ghost" disabled={busy || !restorable.length} onClick={() => setSelected(restorable)}>Select everything</button>
        </div>
      </section>
    </div>

    {review && <ProjectCloseReview review={review} onCancel={() => setReview(undefined)} onApprove={parts => {setReview(undefined); closeReviewed(review.id, parts);}} />}

    {lastClose && <section className="run-report">
      <div className="run-report-head">
        <h4>{['expired', 'review'].includes(lastClose.state) ? 'Last review' : 'Sleep report'}</h4>
        <Badge tone={closeStateLabel(lastClose.state).tone}>{closeStateLabel(lastClose.state).label}</Badge>
        <small>{dateTime(lastClose.created)}</small>
      </div>
      <p className="run-summary">{closeSummary[lastClose.state] ?? 'This run finished in an unexpected state. The observations below are what ContextSleep recorded.'}</p>
      {lastClose.body.detail && <p className="muted-line">{lastClose.body.detail}</p>}
      {lastClose.body.before && lastClose.body.after && <MemoryDelta before={lastClose.body.before} after={lastClose.body.after} />}
      <ul className="step-list">
        {lastClose.body.steps.map(s => {
          const raw = lastClose.state === 'interrupted' && ['checking', 'closing', 'ready', 'pending'].includes(s.state) ? 'interrupted' : s.state;
          const state = stepStateLabel(raw);
          return <li key={s.id}>
            <div className="step-main"><strong>{s.label}</strong><Badge tone={state.tone}>{state.label}</Badge></div>
            {s.detail && <small>{s.detail}</small>}
            {s.result !== undefined && <VerificationResult value={s.result} />}
          </li>;
        })}
      </ul>
    </section>}

    {latest && <section className="run-report">
      <div className="run-report-head">
        <h4>Restore report</h4>
        <Badge tone={isReady ? 'ready' : latest.state === 'interrupted' ? 'warn' : latest.state === 'running' ? 'info' : 'neutral'}>
          {isReady ? 'Verified ready' : latest.state === 'interrupted' ? 'Interrupted' : latest.state === 'running' ? 'Restoring' : 'Finished - review evidence'}
        </Badge>
        <small>{dateTime(latest.created)}</small>
      </div>
      <p className="run-summary">{latest.state === 'interrupted'
        ? 'Restore was interrupted. Check the applications before retrying; completed side effects were not replayed.'
        : latest.state === 'running' ? 'Reconstructing the workspace.'
        : 'Processing finished. The observations below show what actually came back.'}</p>
      {readiness && !isReady && <div className="readiness-block">
        <div className="run-report-head"><h5>Workspace readiness</h5><Badge tone={readiness.status === 'ready' ? 'ready' : 'warn'}>{readiness.status}</Badge></div>
        <p className="muted-line">{readiness.detail}</p>
        <ul className="fact-list">{readiness.facts.map((fact, i) => {
          const tone = stepStateLabel(fact.status);
          return <li key={i}><span className="fact-name">{fact.capability}</span><span className="fact-value"><Badge tone={tone.tone}>{tone.label}</Badge></span></li>;
        })}</ul>
      </div>}
      <ul className="step-list">
        {latest.body.steps.map(step => {
          const raw = latest.state === 'interrupted' && ['pending', 'running'].includes(step.state) ? 'interrupted' : step.state;
          const state = stepStateLabel(raw);
          return <li key={step.id}>
            <div className="step-main"><strong>{step.label}</strong><Badge tone={state.tone}>{state.label}</Badge></div>
            {step.detail && <small>{step.detail}</small>}
            {step.result !== undefined && <VerificationResult value={step.result} />}
          </li>;
        })}
      </ul>
    </section>}

    <Evidence label="Captured workspace evidence and earlier restores">
      <pre>{JSON.stringify({repository: c.body.repository, services, previousRuns: c.runs.slice(1)}, null, 2)}</pre>
    </Evidence>
  </article>;
}
