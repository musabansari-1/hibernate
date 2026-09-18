import {useEffect, useMemo, useRef, useState} from 'react';
import {invoke, isTauri} from '@tauri-apps/api/core';
import type {Dashboard, Snapshot, HibernationRun} from '../../../packages/core/model';
import {associate, normalizeWindowsPath, pruneSelectedTargets, type ProjectSignal} from '../../../packages/core/projects';
import {interruptedRecoveryKey} from '../../../packages/core/recovery';
import {SnapshotCard} from './components/SnapshotCard';
import {CheckpointCard} from './components/CheckpointCard';
import {ProjectDiscovery} from './components/ProjectDiscovery';
import {ListenerObservation} from './components/ListenerObservation';
import {ServiceLaunch} from './components/ServiceLaunch';
import {ChromeColdStart} from './components/ChromeColdStart';
import {SetupGuide} from './components/SetupGuide';
import {ProductTour} from './components/ProductTour';
import {StageRail, type Stage, type StageKey} from './components/StageRail';
import {MemoryGauge} from './components/MemoryGauge';
import {Wordmark} from './components/Logo';
import {Badge, Card, Empty, Evidence, Icon, SectionHead} from './components/ui';
import {appLabel, describe, gb, gbExact, plural, relativeTime} from './lib/format';

type View = 'workspace' | 'history' | 'settings';
type AutoAdapterStatus = {
  id: string;
  label: string;
  detected: boolean;
  prepared: boolean;
  connectionMode: string;
  userAction: string | null;
  error?: string | null;
};
type AutoIntegrationState = {
  catalogVersion: number;
  chromeExtensionPath?: string;
  adapters: AutoAdapterStatus[];
};

const LIVE_WINDOW_MS = 15000;

const lastActive = (at: number | null) =>
  !at ? 'No activity observed'
    : Date.now() - at < LIVE_WINDOW_MS ? 'Active now'
    : `Last seen ${relativeTime(at)}`;

const projectOf = (s: Snapshot) => (s.body as {project?: {id: string}}).project?.id;

export function App() {
  const [data, setData] = useState<Dashboard>();
  const dataRef = useRef<Dashboard | undefined>(undefined);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [operation, setOperation] = useState('');
  const [view, setView] = useState<View>('workspace');
  const [selected, setSelected] = useState<string[]>([]);
  const [name, setName] = useState('My workspace');
  const [path, setPath] = useState('');
  const [repository, setRepository] = useState<{root?: string; branch?: string; repository?: string; detail?: string}>();
  const [serviceName, setServiceName] = useState('Frontend');
  const [serviceUrl, setServiceUrl] = useState('http://localhost:3000/');
  const [filter, setFilter] = useState('');
  const [theme, setTheme] = useState(() => localStorage.getItem('hibernate-theme') ?? 'dark');
  const [setupOpen, setSetupOpen] = useState(() => localStorage.getItem('hibernate-setup-hidden') !== '1');
  const setupInitialized = useRef(false);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [newPath, setNewPath] = useState('');
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const [history, setHistory] = useState<Snapshot[] | null>(null);
  const [archived, setArchived] = useState(false);
  const [historyLimit, setHistoryLimit] = useState(30);
  const historyOptions = useRef({archived: false, limit: 30});
  const [backupPath, setBackupPath] = useState('');
  const [checkpointBackupPath, setCheckpointBackupPath] = useState('');
  const [dismissedRecovery, setDismissedRecovery] = useState(() => localStorage.getItem('hibernate-recovery-dismissed') ?? '');
  const [includeContext, setIncludeContext] = useState(false);

  const [chromePreparedPath, setChromePreparedPath] = useState('');
  const [autoIntegrations, setAutoIntegrations] = useState<AutoIntegrationState>();
  const autoIntegrationStarted = useRef(false);
  const [connectionsOpen, setConnectionsOpen] = useState(false);
  const [tourOpen, setTourOpen] = useState(false);

  async function refresh() {
    if (!isTauri()) return;
    const d = await invoke<Dashboard>('dashboard');
    if (dataRef.current?.workspace.id !== d.workspace.id) {
      setName(d.workspace.name); setPath(d.workspace.projectPath); setSelected([]); setRepository(undefined);
      setHistory(null); setArchived(false); setHistoryLimit(30); historyOptions.current = {archived: false, limit: 30};
    } else {
      const previousReports = dataRef.current.reports;
      setSelected(s => pruneSelectedTargets(s, previousReports, d.reports, Date.now()));
    }
    if (historyOptions.current.archived || historyOptions.current.limit > 30)
      setHistory(await invoke<Snapshot[]>('snapshot_history', {projectId: d.workspace.id, limit: historyOptions.current.limit, archived: historyOptions.current.archived}));
    else setHistory(null);
    dataRef.current = d; setData(d); return d;
  }

  async function discoverAndPrepareApps(showNotice = false) {
    if (!isTauri()) return;
    try {
      const result = await invoke<AutoIntegrationState>('auto_prepare_integrations');
      setAutoIntegrations(result);
      if (result.chromeExtensionPath) setChromePreparedPath(result.chromeExtensionPath);
      if (showNotice) {
        const detected = result.adapters.filter(a => a.detected).map(a => a.label);
        setNotice(detected.length
          ? `App discovery refreshed. ${detected.join(', ')} ${detected.length === 1 ? 'is' : 'are'} available to ContextSleep.`
          : 'App discovery refreshed. Open a supported app and ContextSleep will pick it up automatically.');
      }
    } catch (e) {
      if (showNotice) setError(String(e));
    }
  }

  useEffect(() => {
    void refresh().catch(e => setError(String(e)));
    const timer = setInterval(() => {if (document.visibilityState === 'visible') void refresh().catch(e => setError(String(e)));}, 3000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!isTauri() || autoIntegrationStarted.current) return;
    autoIntegrationStarted.current = true;
    void discoverAndPrepareApps(false);
  }, []);

  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: light)');
    const update = () => {document.documentElement.dataset.theme = theme === 'system' ? (media.matches ? 'light' : 'dark') : theme;};
    update(); localStorage.setItem('hibernate-theme', theme);
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, [theme]);

  useEffect(() => {
    if (!data || setupInitialized.current) return;
    setupInitialized.current = true;
    if (localStorage.getItem('hibernate-setup-hidden') === null && data.checkpoints.length > 0) setSetupOpen(false);
  }, [data]);

  useEffect(() => {
    if (!switcherOpen) return;
    const close = () => setSwitcherOpen(false);
    window.addEventListener('click', close);
    return () => window.removeEventListener('click', close);
  }, [switcherOpen]);

  async function act(f: () => Promise<unknown>) {
    setBusy(true); setError('');
    try {await f(); await refresh();}
    catch (e) {setError(String(e));}
    finally {setBusy(false); setOperation('');}
  }
  async function queue(client: string, action: string, target: string | null = null, snapshotId: string | null = null) {
    return invoke<string>('enqueue', {client, action, target, snapshotId});
  }
  async function waitForJob(id: string) {
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      await new Promise(r => setTimeout(r, 700));
      const d = await refresh();
      const job = d?.jobs.find(j => j.id === id);
      if (job && ['done', 'failed', 'expired', 'interrupted'].includes(job.state)) {
        if (job.state !== 'done') throw new Error(describe(job.result));
        return d;
      }
    }
    throw new Error('This operation is still running in the connected app. Check its result before retrying.');
  }

  const integrations = data?.reports ?? [];
  const projectId = data?.workspace.id;
  const connected = (client: string) => integrations.some(r => r.client === client && Date.now() - r.at < LIVE_WINDOW_MS);
  const memberKey = (report: {client: string}, item: {id: string; membershipKey?: string}) => item.membershipKey ?? item.id;
  const assignedKeys = useMemo(() => integrations.flatMap(r =>
    r.body.items.filter(i => data?.memberships.some(m => m.projectId === projectId && m.client === r.client && m.target === memberKey(r, i)))
      .map(i => JSON.stringify([r.client, i.id]))), [integrations, data?.memberships, projectId]);

  async function capture(targetKeys: string[]) {
    const previous = dataRef.current;
    const current = await refresh();
    if (!previous || !current || previous.workspace.id !== current.workspace.id)
      throw new Error('The active workspace changed. Select its windows again.');
    const targets = pruneSelectedTargets(targetKeys, previous.reports, current.reports, Date.now());
    setSelected(targets);
    if (!targets.length && !includeContext)
      throw new Error('Those windows are no longer connected. Select the windows that are open right now, then capture.');
    await invoke('save_workspace', {name, projectPath: path});
    setOperation('Capturing workspace context. Each app is saved as it finishes.');
    await invoke('capture_checkpoint', {projectId: current.workspace.id, targets: targets.map(key => JSON.parse(key) as [string, string]), includeContext});
    setNotice('Checkpoint saved. Review what may close before putting the workspace to sleep.');
    goTo('checkpoint');
  }

  async function prepareChrome() {
    await act(async () => {
      const info = await invoke<{chromeExtensionPath: string; detail?: string}>('prepare_chrome_integration');
      setChromePreparedPath(info.chromeExtensionPath);
      setNotice(info.detail ?? 'Chrome is prepared. Click Load unpacked once and select the opened folder; ContextSleep will connect automatically.');
    });
  }

  async function installVSCode() {
    await act(async () => {
      const result = await invoke<{detail: string}>('install_vscode_integration');
      setNotice(result.detail);
    });
  }

  async function assignDetectedWindows() {
    await act(async () => {
      const current = dataRef.current ?? await refresh();
      if (!current) throw new Error('ContextSleep is still loading. Try again in a moment.');
      let added = 0;
      let alreadyAssignedElsewhere = 0;
      for (const report of current.reports.filter(r => Date.now() - r.at < LIVE_WINDOW_MS)) {
        for (const item of report.body.items) {
          const target = item.membershipKey ?? item.id;
          const existing = current.memberships.find(m => m.client === report.client && m.target === target);
          if (existing) {
            if (existing.projectId !== current.workspace.id) alreadyAssignedElsewhere += 1;
            continue;
          }
          await invoke('assign_project', {client: report.client, target, projectId: current.workspace.id});
          added += 1;
        }
      }
      if (!added) {
        setNotice(alreadyAssignedElsewhere
          ? 'No unassigned windows were available. Some connected windows already belong to another workspace; review them below if you want to move one.'
          : 'No new connected windows were found yet. Open Chrome or VS Code and wait a few seconds.');
        return;
      }
      setNotice(`${added} ${added === 1 ? 'window' : 'windows'} added to this workspace.${alreadyAssignedElsewhere ? ` ${alreadyAssignedElsewhere} already belonged to another workspace and ${alreadyAssignedElsewhere === 1 ? 'was' : 'were'} left unchanged.` : ''}`);
    });
    goTo('apps');
  }

  async function startSafeDemo() {
    await act(async () => {
      const current = dataRef.current ?? await refresh();
      if (!current) throw new Error('ContextSleep is still loading. Try the demo again in a moment.');
      const chrome = current.reports.find(r => r.adapter === 'chrome' && Date.now() - r.at < LIVE_WINDOW_MS);
      if (!chrome) {
        goTo('integrations-setup');
        throw new Error('Connect Chrome first. Then ContextSleep can create a synthetic demo window without touching your real tabs.');
      }
      const beforeIds = new Set(chrome.body.items.map(item => item.id));
      setOperation('Creating a safe 12-tab demo in Chrome. Your real windows are not touched.');
      const jobId = await queue(chrome.client, 'fixture');
      await waitForJob(jobId);

      const deadline = Date.now() + 12000;
      let demoReport = current.reports.find(r => r.client === chrome.client);
      let demoItem = demoReport?.body.items.find(item => !beforeIds.has(item.id) && item.detail === '12 tabs');
      while (!demoItem && Date.now() < deadline) {
        await new Promise(r => setTimeout(r, 400));
        const latest = await refresh();
        demoReport = latest?.reports.find(r => r.client === chrome.client);
        demoItem = demoReport?.body.items.find(item => !beforeIds.has(item.id) && item.detail === '12 tabs');
      }
      if (!demoReport || !demoItem) {
        goTo('apps');
        throw new Error('The safe Chrome window was created, but it has not appeared in ContextSleep yet. Wait a few seconds, then select the 12-tab validation window under Workspace apps.');
      }

      const target = memberKey(demoReport, demoItem);
      await invoke('assign_project', {client: demoReport.client, target, projectId: current.workspace.id});
      await refresh();
      const key = JSON.stringify([demoReport.client, demoItem.id]);
      setSelected([key]);
      await capture([key]);
      setNotice('Safe demo ready: 12 synthetic Chrome tabs are captured and assigned to this workspace. Review the checkpoint below, put it to sleep, then restore it to see the full loop.');
    });
  }

  async function openWorkspaceCandidate(id: string) {
    await act(() => invoke('select_project', {id}));
    goTo('apps');
  }

  function goTo(id: string, nextView: View = 'workspace') {
    setView(nextView);
    requestAnimationFrame(() => document.getElementById(id)?.scrollIntoView({behavior: 'smooth', block: 'start'}));
  }

  const signals: ProjectSignal[] = [
    ...integrations.filter(r => Date.now() - r.at < LIVE_WINDOW_MS).flatMap(r => r.body.signals ?? []),
    ...(data?.explorer.items ?? []).map(item => ({source: 'explorer' as const, label: 'Explorer folder', path: item.url})),
  ];
  const associations = associate(path, signals, repository?.repository);
  const snapshots = (history ?? data?.snapshots ?? [])
    .filter(s => s.adapter === 'unreadable' || projectOf(s) === projectId)
    .filter(s => !filter || describe(s.body).toLowerCase().includes(filter.toLowerCase()));
  const memory = data?.memory;
  const recoveryKey = data ? interruptedRecoveryKey(data) : '';
  const showRecovery = !!recoveryKey && recoveryKey !== dismissedRecovery;
  const liveReports = integrations.filter(r => connected(r.client));
  const liveIntegrations = liveReports.length;
  const chromeConnected = integrations.some(r => r.adapter === 'chrome' && connected(r.client));
  const vscodeConnected = integrations.some(r => r.adapter === 'vscode' && connected(r.client));
  const chromeKnown = integrations.some(r => r.adapter === 'chrome');
  const chromeAuto = autoIntegrations?.adapters.find(a => a.id === 'chrome');
  const vscodeAuto = autoIntegrations?.adapters.find(a => a.id === 'vscode');
  const detectedAutomaticApps = autoIntegrations?.adapters.filter(a => a.detected && ['chrome', 'vscode'].includes(a.id)) ?? [];
  const chromeNeedsOneTimeApproval = !!chromeAuto?.detected && !chromeConnected && !chromeKnown;
  const vscodeWaitingForWindow = !!vscodeAuto?.detected && !!vscodeAuto.prepared && !vscodeConnected;
  const connectedWindowCount = liveReports.reduce((sum, report) => sum + report.body.items.length, 0);
  const unassignedConnectedWindows = liveReports.reduce((sum, report) => sum + report.body.items.filter(item => {
    const target = memberKey(report, item);
    return !data?.memberships.some(m => m.client === report.client && m.target === target);
  }).length, 0);
  const assignedWindows = data?.memberships.filter(m => m.projectId === projectId).length ?? 0;
  const liveWindowCounts = new Map<string, number>();
  for (const report of integrations.filter(r => connected(r.client))) {
    for (const item of report.body.items) {
      const target = memberKey(report, item);
      const membership = data?.memberships.find(m => m.client === report.client && m.target === target);
      if (membership) liveWindowCounts.set(membership.projectId, (liveWindowCounts.get(membership.projectId) ?? 0) + 1);
    }
  }
  const memoryAvailableRatio = memory?.totalPhysical ? memory.availablePhysical / memory.totalPhysical : null;
  const memoryPressure = memoryAvailableRatio == null
    ? {label: 'Memory reading unavailable', detail: 'ContextSleep needs the desktop app to read Windows memory.', tone: 'neutral' as const}
    : memoryAvailableRatio <= 0.15
      ? {label: 'Memory is tight', detail: `${Math.round(memoryAvailableRatio * 100)}% of physical memory is available. Consider putting away work you are not using.`, tone: 'bad' as const}
      : memoryAvailableRatio <= 0.25
        ? {label: 'Memory pressure is rising', detail: `${Math.round(memoryAvailableRatio * 100)}% of physical memory is available. ContextSleep can help you free headroom safely.`, tone: 'warn' as const}
        : {label: 'Memory looks healthy', detail: `${Math.round(memoryAvailableRatio * 100)}% of physical memory is available. You can still put away a workspace whenever you want.`, tone: 'good' as const};
  const sleepCandidates = (data?.projects ?? [])
    .map(project => ({...project, liveWindows: liveWindowCounts.get(project.id) ?? 0}))
    .filter(project => project.liveWindows > 0)
    .sort((a, b) => {
      if (a.lastObservedActive == null && b.lastObservedActive != null) return 1;
      if (b.lastObservedActive == null && a.lastObservedActive != null) return -1;
      const aObserved = a.lastObservedActive ?? Number.MAX_SAFE_INTEGER;
      const bObserved = b.lastObservedActive ?? Number.MAX_SAFE_INTEGER;
      if (aObserved !== bObserved) return aObserved - bObserved;
      return b.liveWindows - a.liveWindows;
    })
    .slice(0, 4);
  const checkpoint = data?.checkpoints[0];
  const checkpointCount = data?.checkpoints.length ?? 0;
  const lastClose = checkpoint?.hibernations?.[0];
  const lastRun = checkpoint?.runs?.[0];
  const asleep = lastClose?.state === 'closed-selected' && (!lastRun || lastRun.created < lastClose.created);
  const isReady = !!lastRun?.body.readiness?.verified && lastRun.state !== 'interrupted' && (!lastClose || lastRun.created > lastClose.created);
  const sleepDelta = lastClose?.body.before && lastClose.body.after
    ? lastClose.body.after.availablePhysical - lastClose.body.before.availablePhysical : null;

  const stageKey: StageKey = isReady ? 'ready' : asleep ? 'restore' : checkpointCount ? 'sleep' : assignedWindows ? 'capture' : 'connect';
  const stages: Stage[] = [
    {key: 'connect', label: 'Connect', hint: assignedWindows ? `${plural(assignedWindows, 'window')} added` : 'Add your apps', state: assignedWindows ? 'done' : stageKey === 'connect' ? 'current' : 'todo'},
    {key: 'capture', label: 'Save', hint: checkpointCount ? 'Workspace saved' : 'Save before closing', state: checkpointCount ? 'done' : stageKey === 'capture' ? 'current' : 'todo'},
    {key: 'sleep', label: 'Sleep', hint: lastClose ? 'Reviewed and closed' : 'Free memory safely', state: lastClose && ['closed-selected', 'partial'].includes(lastClose.state) ? 'done' : stageKey === 'sleep' ? 'current' : 'todo'},
    {key: 'restore', label: 'Restore', hint: lastRun ? 'Rebuilt locally' : 'Bring it back', state: lastRun ? 'done' : stageKey === 'restore' ? 'current' : 'todo'},
    {key: 'ready', label: 'Verified', hint: isReady ? 'Observed and ready' : 'Check the restore', state: isReady ? 'done' : stageKey === 'ready' ? 'current' : 'todo'},
  ];

  const connectHero = liveIntegrations === 0
    ? chromeNeedsOneTimeApproval
      ? {
          title: 'Step 1: Chrome needs one browser approval',
          body: 'ContextSleep already discovered Chrome and pre-registered its local bridge. Because this private beta uses an unpacked extension, Chrome requires one Load unpacked approval. After that, Chrome reconnects automatically on every launch.',
          cta: 'Approve Chrome once', tone: 'setup',
          onCta: () => void prepareChrome(),
        }
      : vscodeWaitingForWindow
        ? {
            title: 'Step 1: Apps are prepared automatically',
            body: 'The VS Code adapter is already installed. Open or reload a trusted local project and ContextSleep will detect it automatically. No integration setup is required.',
            cta: 'Refresh app discovery', tone: 'setup',
            onCta: () => void discoverAndPrepareApps(true),
          }
        : {
            title: 'Step 1: ContextSleep discovers supported apps automatically',
            body: detectedAutomaticApps.length
              ? `${detectedAutomaticApps.map(a => a.label).join(' and ')} ${detectedAutomaticApps.length === 1 ? 'is' : 'are'} installed. ContextSleep prepares supported adapters automatically and waits for a live window to report.`
              : 'Open a supported app and ContextSleep will prepare its adapter automatically. Built-in folder and terminal context require no integration at all.',
            cta: 'Refresh app discovery', tone: 'setup',
            onCta: () => void discoverAndPrepareApps(true),
          }
    : unassignedConnectedWindows > 0
      ? {
          title: 'Step 2: Add the windows you want to put away',
          body: `ContextSleep can see ${plural(connectedWindowCount, 'window')} right now. Add the ${plural(unassignedConnectedWindows, 'unassigned window')} to this workspace with one click, or choose them manually below.`,
          cta: `Add ${plural(unassignedConnectedWindows, 'detected window')}`, tone: 'setup',
          onCta: () => void assignDetectedWindows(),
        }
      : {
          title: 'Step 2: Choose the windows for this workspace',
          body: connectedWindowCount
            ? 'Your connected windows are already assigned somewhere. Review the list below and choose which ones belong to this workspace.'
            : 'The integrations are connected, but no supported window is reporting yet. Open Chrome or a trusted local VS Code project and wait a few seconds.',
          cta: 'Choose workspace windows', tone: 'setup',
          onCta: () => goTo('apps'),
        };

  const heroCopy: Record<StageKey, {title: string; body: string; cta: string; onCta(): void; tone: string}> = {
    connect: connectHero,
    capture: {
      title: 'Step 3: Save this workspace before anything closes',
      body: `${plural(assignedWindows, 'window')} belong to this workspace. Saving creates the recovery checkpoint ContextSleep needs to rebuild them later. Nothing closes yet.`,
      cta: 'Save this workspace', tone: 'capture',
      onCta: () => void act(() => capture(assignedKeys)),
    },
    sleep: {
      title: 'Step 4: Review, then free the memory',
      body: 'The workspace is saved. ContextSleep will show exactly what may close, recheck it, and leave anything uncertain open.',
      cta: 'Review and sleep', tone: 'sleep',
      onCta: () => goTo('checkpoint'),
    },
    restore: {
      title: 'Workspace asleep. Bring it back when you need it.',
      body: 'The reviewed windows closed and the recovery checkpoint stayed on this PC. Restore it and ContextSleep will verify what actually returned.',
      cta: 'Bring workspace back', tone: 'asleep',
      onCta: () => goTo('checkpoint'),
    },
    ready: {
      title: 'Done: the restore was verified',
      body: lastRun?.body.readiness?.detail ?? 'ContextSleep observed the selected capabilities after restore instead of assuming that launching an app was enough.',
      cta: 'See restore proof', tone: 'ready',
      onCta: () => goTo('checkpoint'),
    },
  };
  const hero = heroCopy[stageKey];

  /* ------------------------------------------------------------------ chrome */

  const topBar = <header className="topbar">
    <div className="topbar-left">
      <Wordmark />
      <div className={`switcher ${switcherOpen ? 'is-open' : ''}`} onClick={e => e.stopPropagation()}>
        <button type="button" className="switcher-trigger" aria-expanded={switcherOpen} onClick={() => setSwitcherOpen(o => !o)}>
          <span className="switcher-label">
            <small>Workspace</small>
            <strong>{data?.workspace.name ?? 'Loading'}</strong>
          </span>
          <Icon name="chevron" className="switcher-caret" />
        </button>
        {switcherOpen && <div className="switcher-menu" role="menu">
          {data?.projects.map(p => <button key={p.id} type="button" role="menuitem" className={p.id === projectId ? 'is-active' : ''}
            disabled={busy} onClick={() => {setSwitcherOpen(false); void act(() => invoke('select_project', {id: p.id}));}}>
            <span className="switcher-name">{p.name}{p.id === projectId && <Icon name="check" />}</span>
            <small>{p.projectPath || 'No folder set'}</small>
            <small>{plural(p.snapshotCount, 'saved context')} - {lastActive(p.lastObservedActive)}</small>
          </button>)}
          <button type="button" role="menuitem" className="switcher-new" onClick={() => {setSwitcherOpen(false); setCreating(true); goTo('workspace-settings', 'settings');}}>
            <Icon name="plus" />New workspace
          </button>
        </div>}
      </div>
    </div>

    <nav className="view-tabs" aria-label="Sections">
      {([['workspace', 'Workspace', 'window'], ['history', 'History', 'history'], ['settings', 'Settings', 'settings']] as const).map(([key, label, icon]) =>
        <button key={key} type="button" className={view === key ? 'is-active' : ''} aria-current={view === key ? 'page' : undefined} onClick={() => setView(key)}>
          <Icon name={icon} />{label}
        </button>)}
    </nav>

    <div className="topbar-right">
      <span className="memory-chip" title="System memory reported by Windows">
        <span className="memory-chip-bar"><i style={{width: memory ? `${Math.round(100 * (1 - memory.availablePhysical / memory.totalPhysical))}%` : '0%'}} /></span>
        <span>{memory ? gb(memory.availablePhysical) : '--'} free</span>
      </span>
      <span className="local-chip" title="Workspace data never leaves this PC"><Icon name="lock" />Local</span>
      <label className="theme-control">
        <span className="sr-only">Appearance</span>
        <select aria-label="Appearance" value={theme} onChange={e => setTheme(e.target.value)}>
          <option value="system">System</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </label>
    </div>
  </header>;

  const banners = <div className="banners">
    {!isTauri() && <div className="banner is-info"><Icon name="info" /><p>This is a browser preview of the interface. Open the ContextSleep desktop app for live Windows readings and actions.</p></div>}
    {error && <div className="banner is-bad" role="alert"><Icon name="alert" /><p>{error}</p><button type="button" className="btn btn-icon" onClick={() => setError('')} aria-label="Dismiss"><Icon name="close" /></button></div>}
    {notice && <div className="banner is-good" role="status"><Icon name="check" /><p>{notice}</p><button type="button" className="btn btn-icon" onClick={() => setNotice('')} aria-label="Dismiss"><Icon name="close" /></button></div>}
    {showRecovery && <div className="banner is-warn" role="alert">
      <Icon name="alert" />
      <p><strong>Interrupted work needs a look.</strong> ContextSleep never replays uncertain close, restore or capture work. Check the live apps before capturing or retrying.</p>
      <div className="banner-actions">
        <button type="button" className="btn btn-quiet" onClick={() => goTo('checkpoint')}>Review checkpoint</button>
        {!!data?.jobs.length && <button type="button" className="btn btn-quiet" onClick={() => goTo('operation-history', 'history')}>Operation log</button>}
        <button type="button" className="btn btn-quiet" onClick={() => {localStorage.setItem('hibernate-recovery-dismissed', recoveryKey); setDismissedRecovery(recoveryKey);}}>I checked</button>
      </div>
    </div>}
  </div>;

  /* --------------------------------------------------------------- workspace */

  const workspaceView = <>
    {data && setupOpen && <div id="setup"><SetupGuide data={data} onClose={() => {localStorage.setItem('hibernate-setup-hidden', '1'); setSetupOpen(false);}} onGo={id => goTo(id, id === 'workspace-settings' ? 'settings' : 'workspace')} /></div>}

    <section className={`hero stage-${hero.tone}`} aria-labelledby="hero-title">
      <div className="hero-inner">
        <div className="hero-copy">
          <p className="eyebrow">{data?.workspace.name ?? 'Workspace'}</p>
          <h1 id="hero-title">{hero.title}</h1>
          <p className="hero-body">{hero.body}</p>
          <div className="hero-actions">
            <button type="button" className="btn btn-primary btn-lg" disabled={busy || !data} onClick={hero.onCta}>
              <Icon name={stageKey === 'sleep' ? 'moon' : stageKey === 'restore' ? 'refresh' : stageKey === 'ready' ? 'layers' : 'arrow'} />{hero.cta}
            </button>
            {stageKey !== 'connect' && <button type="button" className="btn btn-ghost btn-lg" onClick={() => goTo('apps')}>Workspace apps</button>}
          </div>
          <ul className="hero-facts">
            <li><strong>{liveIntegrations}</strong><span>{liveIntegrations === 1 ? 'app connected' : 'apps connected'}</span></li>
            <li><strong>{assignedWindows}</strong><span>{assignedWindows === 1 ? 'window assigned' : 'windows assigned'}</span></li>
            <li><strong>{checkpointCount}</strong><span>{checkpointCount === 1 ? 'checkpoint' : 'checkpoints'}</span></li>
          </ul>
        </div>
        <div className="hero-meter"><MemoryGauge memory={memory} delta={sleepDelta} /></div>
      </div>
      <StageRail stages={stages} onSelect={key => goTo(key === 'connect' || key === 'capture' ? 'apps' : 'checkpoint')} />
    </section>

    {data && !asleep && !isReady && <section className={`sleep-advisor advisor-${memoryPressure.tone}`} aria-labelledby="sleep-advisor-title">
      <div className="advisor-overview">
        <span className="advisor-icon" aria-hidden="true"><Icon name="spark" /></span>
        <div className="advisor-copy">
          <p className="eyebrow">Memory Rescue</p>
          <h2 id="sleep-advisor-title">What should I put away?</h2>
          <p>{memoryPressure.detail}</p>
        </div>
        <div className="advisor-memory">
          <Badge tone={memoryPressure.tone}>{memoryPressure.label}</Badge>
          <strong>{memory ? gb(memory.availablePhysical) : '--'}</strong>
          <small>Windows available memory</small>
        </div>
      </div>
      <div className="advisor-candidates">
        <div className="advisor-candidates-head">
          <strong>Workspaces worth reviewing</strong>
          <small>ContextSleep ranks only workspaces that currently have explicitly assigned windows reporting.</small>
        </div>
        {sleepCandidates.length ? <div className="advisor-list">
          {sleepCandidates.map((project, index) => <div key={project.id} className={`advisor-row ${project.id === projectId ? 'is-current' : ''}`}>
            <span className="advisor-rank" aria-hidden="true">{index + 1}</span>
            <div className="advisor-project">
              <div className="advisor-project-title">
                <strong>{project.name}</strong>
                {project.id === projectId && <Badge tone="info">Current</Badge>}
              </div>
              <small>{project.lastObservedActive ? `Foreground last observed ${relativeTime(project.lastObservedActive)}` : 'No reliable foreground observation recorded'}</small>
            </div>
            <div className="advisor-evidence">
              <strong>{project.liveWindows}</strong>
              <small>{project.liveWindows === 1 ? 'reporting window' : 'reporting windows'}</small>
            </div>
            {project.id === projectId
              ? <button type="button" className="btn btn-primary" disabled={busy || (!checkpointCount && !assignedKeys.length)} onClick={() => checkpointCount ? goTo('checkpoint') : void act(() => capture(assignedKeys))}>{checkpointCount ? 'Review sleep plan' : 'Prepare sleep plan'}</button>
              : <button type="button" className="btn btn-quiet" disabled={busy} onClick={() => void openWorkspaceCandidate(project.id)}>Review workspace</button>}
          </div>)}
        </div> : <div className="advisor-empty">
          <Icon name="info" />
          <span><strong>No sleep candidate is observable yet.</strong><small>Assign a connected Chrome or VS Code window to a workspace and ContextSleep will surface it here.</small></span>
        </div>}
      </div>
      <small className="advisor-note">Recommendation evidence is deliberately conservative: system memory, explicit window membership and last foreground observations. It does not prove continuous inactivity or promise an exact per-workspace RAM saving.</small>
    </section>}

    {checkpointCount === 0 && <section className="quickstart-card" aria-labelledby="quickstart-title">
      <div className="quickstart-copy">
        <span className="quickstart-icon" aria-hidden="true"><Icon name="spark" /></span>
        <div>
          <p className="eyebrow">New to ContextSleep?</p>
          <h2 id="quickstart-title">Try the whole idea safely in about two minutes</h2>
          <p>ContextSleep can create a separate 12-tab synthetic Chrome window, assign it to this workspace and capture it for you. Your real tabs are never touched.</p>
        </div>
      </div>
      <div className="quickstart-action">
        <button type="button" className="btn btn-primary btn-lg" disabled={busy || !data || !isTauri()} onClick={() => chromeConnected ? void startSafeDemo() : chromeNeedsOneTimeApproval ? void prepareChrome() : goTo('integrations-setup')}>
          <Icon name={chromeConnected ? 'spark' : 'arrow'} />{chromeConnected ? 'Try the safe live demo' : chromeNeedsOneTimeApproval ? 'Approve Chrome once' : 'Open Chrome for live demo'}
        </button>
        <button type="button" className="btn btn-ghost" onClick={() => setTourOpen(true)}><Icon name="play" />Watch full 60-second demo</button>
        <small>{chromeConnected ? 'The live demo uses only synthetic ContextSleep pages.' : chromeNeedsOneTimeApproval ? 'This one browser approval is the only Chrome setup step in the private beta.' : 'ContextSleep discovers Chrome automatically when it is available.'}</small>
      </div>
      <div className="scope-strip" aria-label="Current support">
        <div><span className="scope-dot is-good" /><strong>Sleep + restore</strong><small>Chrome, VS Code, approved Node/Python services</small></div>
        <div><span className="scope-dot is-context" /><strong>Restore as context</strong><small>Explorer folders and verified terminal working directories</small></div>
        <div><span className="scope-dot is-safe" /><strong>Fail closed</strong><small>If ContextSleep cannot prove a target is safe to close, it stays open</small></div>
      </div>
    </section>}

    {stageKey === 'connect' || connectionsOpen || integrations.length === 0 ? <section id="integrations-setup" className="block integrations-setup" aria-labelledby="connect-apps-title">
      <SectionHead
        eyebrow="App discovery"
        id="connect-apps-title"
        title="Supported apps connect automatically"
        description="ContextSleep discovers installed supported apps, prepares their adapters in the background, and reconnects them whenever they are available. You only see a manual step when Windows or the app itself requires approval."
      />
      <div className="integration-auto-setup">
        <div>
          <strong>{autoIntegrations ? `${autoIntegrations.adapters.filter(a => a.detected).length} supported adapters detected` : 'Discovering supported apps...'}</strong>
          <small>Explorer and terminal context are built in. VS Code installs automatically. Chrome reconnects automatically after its one-time browser approval.</small>
        </div>
        <button type="button" className="btn btn-quiet" disabled={busy || !isTauri()} onClick={() => void discoverAndPrepareApps(true)}>
          <Icon name="refresh" />Refresh discovery
        </button>
      </div>
      <div className="integration-setup-grid">
        <Card className={`integration-setup-card ${chromeConnected ? 'is-connected' : ''}`}>
          <div className="integration-setup-head">
            <span className="app-chip" data-app="chrome" aria-hidden="true">Ch</span>
            <div><h3>Chrome</h3><p>Tabs, order, pins, groups and reviewed close/restore.</p></div>
            <Badge tone={chromeConnected ? 'good' : chromeKnown ? 'info' : chromeAuto?.detected ? 'warn' : 'neutral'} dot>
              {chromeConnected ? 'Connected' : chromeKnown ? 'Ready automatically' : chromeAuto?.detected ? 'One approval needed' : 'Not installed'}
            </Badge>
          </div>
          {chromeConnected ? <p className="integration-success"><Icon name="check" />Chrome reconnects automatically whenever it is available.</p>
            : chromeKnown && chromeAuto?.detected ? <>
              <p className="muted-line"><strong>Chrome has connected before.</strong> Just open Chrome and the existing extension will reconnect automatically. There is no setup to repeat.</p>
              <div className="integration-actions">
                <button type="button" className="btn btn-quiet" disabled={busy || !isTauri()} onClick={() => void discoverAndPrepareApps(true)}><Icon name="refresh" />Refresh discovery</button>
              </div>
            </> : chromeAuto?.detected ? <>
              <p className="muted-line"><strong>Chrome was discovered automatically.</strong> ContextSleep has already prepared and registered the local bridge. The current private beta needs one Chrome approval because its extension is loaded unpacked.</p>
              <div className="integration-actions">
                <button type="button" className="btn btn-primary" disabled={busy || !isTauri()} onClick={() => void prepareChrome()}><Icon name="check" />Approve Chrome once</button>
                {chromePreparedPath && <small className="integration-path">ContextSleep opens the exact folder to choose.</small>}
              </div>
              <small className="muted-line">After that one approval there is no extension ID, registration, Connect, or refresh step on future launches.</small>
            </> : <p className="muted-line">Chrome is not installed on this PC. There is nothing to configure; ContextSleep will discover it automatically if it appears later.</p>}
        </Card>

        <Card className={`integration-setup-card ${vscodeConnected ? 'is-connected' : ''}`}>
          <div className="integration-setup-head">
            <span className="app-chip" data-app="vscode" aria-hidden="true">VS</span>
            <div><h3>VS Code</h3><p>Workspace, editors and verified working-directory context.</p></div>
            <Badge tone={vscodeConnected ? 'good' : vscodeAuto?.detected ? (vscodeAuto.prepared ? 'info' : 'warn') : 'neutral'} dot>
              {vscodeConnected ? 'Connected' : vscodeAuto?.detected ? (vscodeAuto.prepared ? 'Ready automatically' : 'Needs retry') : 'Not installed'}
            </Badge>
          </div>
          {vscodeConnected ? <p className="integration-success"><Icon name="check" />VS Code reconnects automatically whenever a trusted local project is open.</p>
            : vscodeAuto?.detected && vscodeAuto.prepared ? <>
              <p className="muted-line"><strong>The VS Code adapter was installed automatically.</strong> Open a trusted local project. If VS Code was already open when ContextSleep started, reload that window once and it will appear here.</p>
              <small className="muted-line">No VSIX installation or bridge-path configuration is required.</small>
            </> : vscodeAuto?.detected ? <>
              <p className="muted-line">ContextSleep found VS Code, but its adapter could not be prepared automatically{vscodeAuto.error ? `: ${vscodeAuto.error}` : '.'}</p>
              <div className="integration-actions">
                <button type="button" className="btn btn-quiet" disabled={busy || !isTauri()} onClick={() => void installVSCode()}><Icon name="refresh" />Retry adapter install</button>
              </div>
            </> : <p className="muted-line">VS Code is not installed on this PC. If it is installed later, ContextSleep will discover and prepare it automatically.</p>}
        </Card>
      </div>
      <div className="scope-strip" aria-label="Built-in adapters">
        <div><span className="scope-dot is-good" /><strong>Built in</strong><small>Explorer folder context</small></div>
        <div><span className="scope-dot is-good" /><strong>Built in</strong><small>Verified terminal working directories</small></div>
        <div><span className="scope-dot is-safe" /><strong>Scalable</strong><small>Future supported adapters are discovered automatically</small></div>
      </div>
      {!isTauri() && <p className="muted-line">Automatic app discovery runs in the ContextSleep desktop app, not this browser preview.</p>}
    </section> : <section id="integrations-setup" className="connection-summary" aria-label="App connections">
      <div>
        <p className="eyebrow">Automatic app discovery</p>
        <strong>{integrations.length} integration{integrations.length === 1 ? '' : 's'} known</strong>
        <small>{liveIntegrations ? `${liveIntegrations} reporting right now.` : 'Nothing reporting right now - supported apps reconnect automatically when available.'}</small>
      </div>
      <div className="connection-summary-badges">
        <Badge tone={chromeConnected ? 'good' : 'neutral'} dot>Chrome {chromeConnected ? 'live' : 'offline'}</Badge>
        <Badge tone={vscodeConnected ? 'good' : 'neutral'} dot>VS Code {vscodeConnected ? 'live' : 'offline'}</Badge>
      </div>
      <button type="button" className="btn btn-quiet" onClick={() => {setConnectionsOpen(true); requestAnimationFrame(() => document.getElementById('integrations-setup')?.scrollIntoView({behavior: 'smooth', block: 'start'}));}}>View app discovery</button>
    </section>}

    <section id="apps" className="block" aria-labelledby="apps-title">
      <SectionHead
        eyebrow="Choose what to put away"
        id="apps-title"
        title="Which windows belong to this workspace?"
        description="Add the windows you want ContextSleep to save and later rebuild. Adding a window does not close it."
        action={<div className="row-actions">
          {!!unassignedConnectedWindows && <button type="button" className="btn btn-primary" disabled={busy || !projectId} onClick={() => void assignDetectedWindows()}>Add detected windows</button>}
          <button type="button" className="btn btn-quiet" disabled={busy || !projectId || !assignedKeys.length} onClick={() => setSelected(assignedKeys)}>Select saved workspace windows</button>
        </div>}
      />

      {!integrations.length && <Empty
        title="Waiting for a supported app"
        body="ContextSleep is already watching for supported apps. Open Chrome or a trusted local VS Code project; built-in folder and terminal context require no integration setup."
        action={<button type="button" className="btn btn-primary" disabled={busy || !isTauri()} onClick={() => void discoverAndPrepareApps(true)}>Refresh app discovery</button>}
      />}

      <div className="app-grid">
        {integrations.map(report => {
          const live = connected(report.client);
          return <Card key={report.client} className={`app-card ${live ? '' : 'is-offline'}`}>
            <header className="app-card-head">
              <span className="app-chip" data-app={report.adapter} aria-hidden="true">{appLabel(report.adapter).slice(0, 2)}</span>
              <div>
                <strong>{report.body.label}</strong>
                <small>{plural(report.body.items.length, 'window')}</small>
              </div>
              <Badge tone={live ? 'good' : 'neutral'} dot>{live ? 'Connected' : 'Offline'}</Badge>
            </header>
            <ul className="window-list">
              {report.body.items.map(item => {
                const key = JSON.stringify([report.client, item.id]);
                const membershipTarget = memberKey(report, item);
                const assigned = data?.memberships.find(m => m.client === report.client && m.target === membershipTarget)?.projectId ?? '';
                const mine = assigned === projectId;
                return <li key={item.id} className={`window-row ${selected.includes(key) ? 'is-selected' : ''}`}>
                  <label className="window-pick">
                    <input type="checkbox" aria-label={`Select ${item.title}`} disabled={busy || !live}
                      checked={selected.includes(key)}
                      onChange={e => setSelected(s => e.target.checked ? [...s, key] : s.filter(k => k !== key))} />
                  </label>
                  <div className="window-copy">
                    <strong>{item.title}</strong>
                    <small>{item.detail}</small>
                    <small className="window-compat">{item.compatibility}</small>
                  </div>
                  <div className="window-assign">
                    {mine && <Badge tone="good">This workspace</Badge>}
                    <select aria-label={`Workspace for ${item.title}`} disabled={busy} value={assigned}
                      onChange={e => void act(() => invoke('assign_project', {client: report.client, target: membershipTarget, projectId: e.target.value || null}))}>
                      <option value="">Unassigned</option>
                      {data?.projects.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                    </select>
                  </div>
                </li>;
              })}
            </ul>
            {report.adapter === 'chrome' && <div className="app-card-foot">
              <ChromeColdStart binding={data?.chromeLaunches.find(v => v.client === report.client)} busy={busy}
                save={(profileDirectory, executable, userDataDir) => act(() => invoke('configure_chrome_launch', {client: report.client, profileDirectory, executable: executable.trim() || null, userDataDir: userDataDir.trim() || null}))} />
              <details className="panel-details">
                <summary><span>Test without touching real work</span><small>Opens a safe validation window</small></summary>
                <div className="panel-details-body">
                  <p className="muted-line">Creates a dedicated Chrome window of 12 synthetic pages so you can try the full capture, sleep and restore loop without risking anything you care about.</p>
                  <button type="button" className="btn btn-quiet" disabled={busy || !live} onClick={() => void act(() => queue(report.client, 'fixture'))}>Create validation window</button>
                </div>
              </details>
            </div>}
          </Card>;
        })}
      </div>

      {!!integrations.length && <div className="capture-bar">
        <label className="option-row">
          <input type="checkbox" disabled={busy || !path} checked={includeContext} onChange={e => setIncludeContext(e.target.checked)} />
          <span>
            <strong>Include folders and terminals</strong>
            <small>Adds this workspace's Explorer folders and verified terminal working directories. These restore as context only.</small>
          </span>
        </label>
        <div className="capture-actions">
          <small>{selected.length ? `${plural(selected.length, 'window')} selected` : 'No window selected'}</small>
          <button type="button" className="btn btn-primary btn-lg" disabled={busy || !data || (!selected.length && !includeContext)} onClick={() => void act(() => capture(selected))}>
            <Icon name="layers" />Save this workspace
          </button>
        </div>
      </div>}
    </section>

    <section id="checkpoint" className="block" aria-labelledby="checkpoint-title">
      <SectionHead
        eyebrow="Sleep, restore, verify"
        id="checkpoint-title"
        title="Sleep and restore"
        description="A checkpoint is a durable local plan for rebuilding this workspace. Review it to see exactly what is eligible to close; after restore, ContextSleep reports what actually came back."
      />
      {!checkpointCount && <Empty
        title="No checkpoint yet"
        body="Select your assigned windows above and capture. ContextSleep never closes a window before that saved state exists and you have reviewed it."
        action={<button type="button" className="btn btn-quiet" onClick={() => goTo('apps')}>Go to workspace apps</button>}
      />}
      {data?.checkpoints.map(c => <CheckpointCard key={c.id} checkpoint={c} busy={busy}
        reviewClose={async () => {
          let review: HibernationRun | undefined;
          await act(async () => {review = await invoke<HibernationRun>('review_checkpoint_close', {id: c.id});});
          return review;
        }}
        closeReviewed={(reviewId, parts) => void act(async () => {
          setOperation('Checking every selected window, then closing them in order. Answer any confirmation prompt in the app itself.');
          await invoke('close_checkpoint', {reviewId, selected: parts, acknowledged: true});
        })}
        restore={(parts, acknowledgedServices) => void act(async () => {
          setOperation('Rebuilding your workspace. Observations appear as each part finishes.');
          await invoke('restore_checkpoint', {id: c.id, selected: parts, acknowledgedServices});
        })}
        exportBackup={() => void act(async () => setNotice(`Encrypted checkpoint saved: ${await invoke<string>('export_checkpoint', {id: c.id})}`))} />)}
    </section>
  </>;

  /* ----------------------------------------------------------------- history */

  const historyView = <>
    <section className="block" aria-labelledby="history-title">
      <SectionHead
        eyebrow="History"
        id="history-title"
        title="Saved workspace context"
        description="Every capture stays on this PC so you can inspect, restore, archive, export or remove it."
        action={<label className="search"><Icon name="search" /><input value={filter} onChange={e => setFilter(e.target.value)} placeholder="Find a title, URL or folder" aria-label="Find a saved context" /></label>}
      />
      <div className="row-actions">
        <button type="button" className="btn btn-quiet" disabled={busy} onClick={() => void act(async () => {
          historyOptions.current.archived = !archived; setArchived(!archived);
          setHistory(await invoke('snapshot_history', {projectId, limit: historyLimit, archived: !archived}));
        })}>{archived ? 'Show recent' : 'Show archive'}</button>
        <button type="button" className="btn btn-quiet" disabled={busy || historyLimit >= 500} onClick={() => void act(async () => {
          const limit = Math.min(500, historyLimit + 30);
          historyOptions.current.limit = limit; setHistoryLimit(limit);
          setHistory(await invoke('snapshot_history', {projectId, limit, archived}));
        })}>Load older</button>
      </div>
      {!snapshots.length && <Empty title="Nothing saved here yet" body="Captures for this workspace appear here as soon as you make one." />}
      <div className="stack">
        {snapshots.map(s => <SnapshotCard key={s.id} snapshot={s} busy={busy} connected={connected(s.client)}
          archive={() => void act(async () => {await invoke('archive_snapshot', {id: s.id, archived: !s.archived}); setHistory(null);})}
          exportBackup={() => void act(async () => setNotice(`Encrypted backup saved: ${await invoke<string>('export_snapshot', {id: s.id})}`))}
          deleteSnapshot={confirmation => void act(() => invoke('delete_snapshot', {id: s.id, confirmation}))}
          openVSCode={() => void act(() => invoke('open_vscode_snapshot', {snapshotId: s.id}))}
          resume={() => void act(async () => {
            setOperation('Rebuilding saved context.');
            await waitForJob(await queue(s.client, 'resume', null, s.id));
            if (dataRef.current?.services.length) await invoke('verify_services', {projectId: projectOf(s)});
          })}
          closeReviewed={quick => void act(async () => {
            setOperation(quick ? 'Discarding eligible background tabs.' : 'Closing the reviewed window. Answer any confirmation prompt in the app itself.');
            await waitForJob(await invoke<string>('close_reviewed', {snapshotId: s.id, acknowledged: true, quickSleep: quick}));
          })}
          restoreContext={(kind, index) => void act(() => invoke('restore_context_item', {snapshotId: s.id, kind, index}))} />)}
      </div>
    </section>

    <section className="block" aria-labelledby="restore-backup-title">
      <SectionHead eyebrow="Backups" id="restore-backup-title" title="Import an encrypted backup"
        description="Backups are protected to the Windows account that exported them. Importing never launches or closes anything." />
      <div className="import-grid">
        <Card>
          <h3>Checkpoint backup</h3>
          <form className="field-grid" onSubmit={e => {e.preventDefault(); void act(async () => {
            const id = await invoke<string>('import_checkpoint', {path: checkpointBackupPath});
            setNotice(`Checkpoint imported: ${id}. Select its workspace to restore it.`); setCheckpointBackupPath('');
          });}}>
            <label>Backup file<input value={checkpointBackupPath} onChange={e => setCheckpointBackupPath(e.target.value)} placeholder="D:\backups\checkpoint.hibernate" required /></label>
            <button type="submit" className="btn btn-quiet" disabled={busy || !data}>Import checkpoint</button>
          </form>
          <small className="muted-line">Import recreates workspace and service definitions plus restore-only saved contexts with new IDs. It never imports pending jobs, close approvals, process identities or previous verification results.</small>
        </Card>
        <Card>
          <h3>Single capture backup</h3>
          <form className="field-grid" onSubmit={e => {e.preventDefault(); void act(async () => {
            const id = await invoke<string>('import_snapshot', {path: backupPath});
            setNotice(`Capture imported: ${id}. Select its original workspace to restore it.`); setBackupPath('');
          });}}>
            <label>Backup file<input value={backupPath} onChange={e => setBackupPath(e.target.value)} placeholder="D:\backups\snapshot.hibernate" required /></label>
            <button type="submit" className="btn btn-quiet" disabled={busy || !data}>Import capture</button>
          </form>
          <small className="muted-line">Use the Windows account that created the backup. No pending command or close approval is imported.</small>
        </Card>
      </div>
    </section>

    {!!data?.jobs.length && <section id="operation-history" className="block" aria-labelledby="ops-title">
      <SectionHead eyebrow="Diagnostics" id="ops-title" title="Operation log" description="Raw results from recent engine operations." />
      <div className="stack">
        {data.jobs.map(j => <Card key={j.id} className="op-row">
          <div className="row-between"><strong>{j.action.replaceAll('_', ' ')}</strong><Badge tone={j.state === 'done' ? 'good' : j.state === 'interrupted' ? 'warn' : j.state === 'failed' ? 'bad' : 'neutral'}>{j.state}</Badge></div>
          {j.result != null && <Evidence label="Result"><pre>{describe(j.result)}</pre></Evidence>}
        </Card>)}
      </div>
    </section>}
  </>;

  /* ---------------------------------------------------------------- settings */

  const settingsView = <>
    <section id="workspace-settings" className="block" aria-labelledby="ws-title">
      <SectionHead eyebrow="Workspaces" id="ws-title" title="Your workspaces"
        description="A workspace is a folder plus the app windows you assign to it."
        action={<button type="button" className="btn btn-quiet" disabled={busy || !data} onClick={() => setCreating(c => !c)}><Icon name="plus" />New workspace</button>} />

      {creating && <Card className="create-card">
        <form className="field-grid" onSubmit={e => {e.preventDefault(); void act(async () => {
          await invoke('create_project', {name: newName, projectPath: newPath});
          setCreating(false); setNewName(''); setNewPath('');
        });}}>
          <label>Name<input value={newName} onChange={e => setNewName(e.target.value)} required /></label>
          <label>Folder<input value={newPath} onChange={e => setNewPath(e.target.value)} placeholder="D:\projects\my-project" /></label>
          <button type="submit" className="btn btn-primary" disabled={busy}>Create</button>
        </form>
      </Card>}

      <div className="workspace-grid">
        {data?.projects.map(p => <button key={p.id} type="button" className={`workspace-tile ${p.id === projectId ? 'is-active' : ''}`}
          disabled={busy} onClick={() => void act(() => invoke('select_project', {id: p.id}))}>
          <span className="tile-head"><strong>{p.name}</strong>{p.id === projectId && <Badge tone="good">Active</Badge>}</span>
          <small>{p.projectPath || 'No folder set'}</small>
          <small>{plural(p.snapshotCount, 'saved context')}</small>
          <small className="tile-activity">{lastActive(p.lastObservedActive)}</small>
        </button>)}
      </div>
      <small className="muted-line">Activity is only observed while an assigned Chrome or VS Code window is reporting. A gap does not prove the workspace was idle, and reclaimable memory per workspace is not something ContextSleep can know.</small>

      <Card className="settings-card">
        <h3>Active workspace</h3>
        <div className="field-grid">
          <label>Name<input disabled={!data || busy} value={name} onChange={e => setName(e.target.value)} /></label>
          <label>Folder<input disabled={!data || busy} value={path} onChange={e => setPath(e.target.value)} placeholder="D:\projects\my-project" /></label>
          <button type="button" className="btn btn-quiet" disabled={busy || !data} onClick={() => void act(() => invoke('save_workspace', {name, projectPath: path}))}>Save</button>
        </div>
      </Card>

      <Card className="settings-card"><ProjectDiscovery enabled={!!data} busy={busy}
        accept={c => act(() => c.existingProjectId ? invoke('select_project', {id: c.existingProjectId}) : invoke('create_project', {name: c.name, projectPath: c.projectPath}))} /></Card>
    </section>

    <section className="block" aria-labelledby="context-title">
      <SectionHead eyebrow="Optional" id="context-title" title="Folders and terminals"
        description="Explorer folders and verified terminal working directories can be saved as context. They reopen on restore; commands, history and panes are never replayed."
        action={<button type="button" className="btn btn-quiet" disabled={busy || !path} onClick={() => void act(async () => setRepository(await invoke('inspect_project', {path})))}>Check folder</button>} />
      {repository && <Card className="settings-card">
        <strong>{repository.root ?? 'Not a repository'}</strong>
        <small>{repository.branch ? `branch ${repository.branch}` : ''}{repository.repository ? ` - ${repository.repository}` : ''}</small>
        <small className="muted-line">{repository.detail}</small>
      </Card>}
      {!signals.length && <p className="muted-line">Open a folder in VS Code or Explorer and its relationships appear here.</p>}
      {!!associations.length && <ul className="signal-list">
        {associations.slice(0, 20).map((a, i) => <li key={i}>
          <Badge tone={a.strength === 'exact' ? 'good' : a.strength === 'suggestion' ? 'info' : 'neutral'}>
            {a.strength === 'exact' ? 'Folder match' : a.strength === 'suggestion' ? 'Possible match' : 'Unassigned'}
          </Badge>
          <div className="signal-copy">
            <strong>{a.signal.label}</strong>
            <small>{a.signal.path ?? a.signal.url}</small>
            <small className="muted-line">{a.reason}</small>
          </div>
          {a.signal.path && <button type="button" className="btn btn-quiet" disabled={busy} onClick={() => setPath(normalizeWindowsPath(a.signal.path!) ?? '')}>Use as folder</button>}
        </li>)}
      </ul>}
      {data?.explorer.error && <p className="muted-line">Explorer discovery unavailable: {data.explorer.error}</p>}
      <div className="row-actions">
        <button type="button" className="btn btn-quiet" disabled={busy || !data || !path} onClick={() => void act(async () => {await invoke('save_workspace', {name, projectPath: path}); await invoke('capture_context');})}>Capture folders and terminals</button>
        <button type="button" className="btn btn-quiet" disabled={busy || !path} onClick={() => void act(async () => setNotice(describe(await invoke('restore_folder', {kind: 'terminal', path}))))}>Open terminal here</button>
        <button type="button" className="btn btn-quiet" disabled={busy || !path} onClick={() => void act(async () => setNotice(describe(await invoke('restore_folder', {kind: 'explorer', path}))))}>Open folder</button>
      </div>
      <small className="muted-line">Explorer windows stay open. Live instrumented terminal folders are saved when available; otherwise ContextSleep saves one folder definition. Restore always opens a fresh PowerShell prompt.</small>
    </section>

    <section className="block" aria-labelledby="services-title">
      <SectionHead eyebrow="Optional" id="services-title" title="Local development services"
        description="If this workspace depends on a local Node or Python service, ContextSleep can check it and include it in sleep and restore after you review the exact command."
        action={<button type="button" className="btn btn-quiet" disabled={busy || !data?.services.length} onClick={() => void act(() => invoke('verify_services', {projectId}))}>Check now</button>} />
      <Card className="settings-card">
        <form className="field-grid" onSubmit={e => {e.preventDefault(); void act(() => invoke('save_service', {projectId, name: serviceName, url: serviceUrl}));}}>
          <label>Service name<input value={serviceName} onChange={e => setServiceName(e.target.value)} required /></label>
          <label>Health endpoint<input value={serviceUrl} onChange={e => setServiceUrl(e.target.value)} required /></label>
          <button type="submit" className="btn btn-quiet" disabled={busy || !data}>Add service</button>
        </form>
      </Card>
      <div className="stack">
        {data?.services.map(s => {
          const observed = data.serviceHealth?.services.find(v => v.id === s.id)?.observation;
          return <Card key={s.id} className="settings-card">
            <div className="row-between">
              <div><strong>{s.name}</strong><small>{s.url}</small></div>
              <Badge tone={observed?.status === 'healthy' ? 'good' : observed ? 'warn' : 'neutral'}>
                {observed?.status ?? 'Not checked'}{observed?.httpStatus ? ` - HTTP ${observed.httpStatus}` : ''}
              </Badge>
            </div>
            <small className="muted-line">{observed?.detail ?? 'Not checked yet'}</small>
            <ListenerObservation value={observed?.listenerOwnership} />
            <ServiceLaunch key={`${projectId}:${s.id}:${JSON.stringify(s.launch)}`} service={s} cwd={data.workspace.projectPath} busy={busy}
              configure={definition => act(() => invoke('configure_service_launch', {id: s.id, definition}))}
              start={reviewed => act(() => invoke('start_service', {id: s.id, reviewed, acknowledged: true}))} />
          </Card>;
        })}
      </div>
      {data?.serviceHealth && <small className="muted-line">Last checked {new Date(data.serviceHealth.checkedAt).toLocaleString()}. A healthy endpoint does not prove the expected process owns it.</small>}
    </section>

    <section className="block" aria-labelledby="advanced-title">
      <SectionHead eyebrow="Advanced" id="advanced-title" title="Diagnostics"
        description="Raw observations for support. Working sets can be shared between windows and must never be summed as exclusive workspace memory." />
      <Card className="settings-card">
        <div className="row-actions">
          <button type="button" className="btn btn-quiet" disabled={busy} onClick={() => void act(async () => setNotice(`Privacy-safe diagnostics saved: ${await invoke<string>('export_diagnostics')}`))}>
            <Icon name="download" />Export privacy-safe report
          </button>
        </div>
        <small className="muted-line">The report contains aggregate capability and status counts only. It excludes folder paths, URLs, titles, terminal directories, service commands and saved payloads.</small>
        <Evidence label={`Observed windows (${data?.windows.length ?? 0})`}>
          <div className="table-scroll">
            <table>
              <thead><tr><th>Window</th><th>Process working set</th></tr></thead>
              <tbody>{data?.windows.map(w => <tr key={w.handle}>
                <td><strong>{w.app}{w.foreground ? ' - foreground' : ''}</strong><small>{w.title}</small></td>
                <td>{w.workingSet === null ? 'Unavailable' : gbExact(w.workingSet)}</td>
              </tr>)}</tbody>
            </table>
          </div>
        </Evidence>
      </Card>
    </section>
  </>;

  return <div className="app">
    {topBar}
    {operation && <div className="operation-strip" role="status" aria-live="polite">
      <span className="operation-bar" aria-hidden="true"><i /></span>
      <p>{operation}</p>
    </div>}
    <main className="app-main">
      {banners}
      {view === 'workspace' && workspaceView}
      {view === 'history' && historyView}
      {view === 'settings' && settingsView}
      <footer className="app-footer">
        <Wordmark tag="Windows private beta" />
        <div>
          <p>Verified sleep and restore for VS Code, Chrome and approved local development services. Explorer folders and instrumented terminals restore as context.</p>
          <p>ContextSleep never claims arbitrary application state or full process hibernation. If a close target is unsupported or ambiguous, it stays open.</p>
        </div>
      </footer>
    </main>
    {tourOpen && <ProductTour onClose={() => setTourOpen(false)} />}
  </div>;
}
