/**
 * DEV ONLY. Design harness for apps/desktop/preview/index.html.
 * Never bundled into the product: vite builds apps/desktop/index.html only, and
 * nothing in src/ imports this file. It exists so the interface can be reviewed
 * in every lifecycle state without touching a real workspace.
 */
import type {Dashboard, Memory, ProjectCheckpoint, Report, Snapshot} from '../../../packages/core/model';

const GB = 1024 ** 3;
const now = Date.now();

const mem = (available: number): Memory => ({
  at: now, totalPhysical: 15.8 * GB, availablePhysical: available * GB,
  chromeWorkingSet: 3.1 * GB, chromePrivateCommit: 2.4 * GB, unreadableProcesses: 2,
  sourceBrowserPid: 9120, sourceBrowserWorkingSet: 2.2 * GB, sourceBrowserPrivateCommit: 1.8 * GB, sourceBrowserUnreadable: 1,
});

const reports: Report[] = [
  {
    client: 'vscode-a', adapter: 'vscode', at: now - 1200,
    body: {
      label: 'VS Code', signals: [{source: 'vscode', label: 'Workspace folder', path: 'D:\\projects\\hibernate'}],
      items: [
        {id: 'w1', title: 'hibernate', detail: 'D:\\projects\\hibernate - 14 editors open', compatibility: 'Full sleep and restore when the window is unambiguous', membershipKey: 'vscode:hibernate'},
        {id: 'w2', title: 'design-system', detail: 'D:\\projects\\design-system - 3 editors open', compatibility: 'Full sleep and restore when the window is unambiguous', membershipKey: 'vscode:design-system'},
      ],
    },
  },
  {
    client: 'chrome-a', adapter: 'chrome', at: now - 900,
    body: {
      label: 'Chrome', signals: [{source: 'browser', label: 'Localhost tab', url: 'http://localhost:3000/'}],
      items: [
        {id: '41', title: 'Docs, dashboards and 18 more tabs', detail: '21 tabs - 3 groups - 2 pinned', compatibility: 'Tabs, order, pins and groups restore lazily', membershipKey: 'chrome:41'},
        {id: '42', title: 'Personal window', detail: '6 tabs - no group', compatibility: 'Tabs, order, pins and groups restore lazily', membershipKey: 'chrome:42'},
      ],
    },
  },
];

const snapshot = (id: string, adapter: string, state: string, body: unknown, after: Memory | null = null): Snapshot => ({
  id, client: adapter === 'chrome' ? 'chrome-a' : 'vscode-a', adapter, state, created: now - 900000,
  body, before: mem(5.4), after, result: null,
});

const closeRun = {
  id: 'h1', created: now - 300000, state: 'closed-selected',
  body: {
    project: {name: 'hibernate', projectPath: 'D:\\projects\\hibernate'}, phase: 'complete', expiresAt: now + 60000,
    detail: 'Two windows closed and observed to disappear. One VS Code window was kept open because two workbenches share one process.',
    before: mem(5.4), after: mem(7.3),
    steps: [
      {id: 'e1', kind: 'chrome', label: 'Chrome - 21 tabs', eligible: true, state: 'closed', detail: 'Window handle disappeared 0.8s after close.'},
      {id: 'e2', kind: 'vscode', label: 'VS Code - hibernate', eligible: true, state: 'closed', detail: 'GUI process exited cleanly.'},
      {id: 'e3', kind: 'vscode', label: 'VS Code - design-system', eligible: false, state: 'retained', detail: 'Two workbench windows share one process, so this window cannot be closed safely. Left open.'},
    ],
  },
};

const restoreRun = {
  id: 'r1', created: now - 60000, state: 'done',
  body: {
    readiness: {
      status: 'ready', verified: true,
      detail: 'Chrome relaunched into the enrolled profile, VS Code reconnected to the same workspace identity, and the reviewed service answered its health endpoint.',
      facts: [
        {capability: 'Chrome tabs and groups', kind: 'chrome', status: 'verified'},
        {capability: 'Chrome window organization', kind: 'chrome', status: 'verified'},
        {capability: 'VS Code workspace identity', kind: 'vscode', status: 'verified'},
        {capability: 'Frontend service health', kind: 'service', status: 'verified'},
      ],
    },
    steps: [
      {id: 'e1', kind: 'chrome', label: 'Chrome - 21 tabs', state: 'verified', detail: 'All 21 destinations, both pins and 3 groups matched.', result: {facts: [{capability: 'Tabs', expected: 21, observed: 21, status: 'verified'}, {capability: 'Groups', expected: 3, observed: 3, status: 'verified'}]}},
      {id: 'e2', kind: 'vscode', label: 'VS Code - hibernate', state: 'verified', detail: 'Same integration identity reconnected after 6.2s.'},
    ],
  },
};

const checkpoint = (opts: {close?: boolean; restore?: boolean}): ProjectCheckpoint => ({
  id: 'c1', created: now - 600000, state: 'saved',
  hibernations: opts.close ? [closeRun as never] : [],
  runs: opts.restore ? [restoreRun as never] : [],
  body: {
    project: {name: 'hibernate', projectPath: 'D:\\projects\\hibernate'},
    repository: {branch: 'master', repository: 'local/hibernate'},
    services: [{id: 's1', name: 'Frontend', url: 'http://localhost:3000/health', launch: {executable: 'C:\\Program Files\\nodejs\\node.exe', arguments: ['server.js'], cwd: 'D:\\projects\\hibernate'}}],
    entries: [
      {id: 'e1', kind: 'chrome', label: 'Chrome - 21 tabs, 3 groups'},
      {id: 'e2', kind: 'vscode', label: 'VS Code - hibernate'},
      {id: 'e4', kind: 'context', label: 'Explorer folder - D:\\projects\\hibernate'},
    ],
    captures: [{state: 'captured'}, {state: 'captured'}, {state: 'captured'}],
  },
});

const base: Dashboard = {
  checkpoints: [],
  chromeLaunches: [{client: 'chrome-a', profileDirectory: 'Default', executable: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe', userDataDir: null, extensionId: 'abcdefghijklmnopabcdefghijklmnop', configuredAt: now}],
  services: [{id: 's1', name: 'Frontend', url: 'http://localhost:3000/health', launch: {executable: 'C:\\Program Files\\nodejs\\node.exe', arguments: ['server.js'], cwd: 'D:\\projects\\hibernate'}}],
  serviceHealth: {checkedAt: now - 30000, services: [{id: 's1', name: 'Frontend', observation: {status: 'healthy', detail: 'Responded in 24ms.', httpStatus: 200, url: 'http://localhost:3000/health', listenerOwnership: {status: 'observed', detail: 'One process is listening.', listeners: [{pid: 22188, identity: {executable: 'C:\\Program Files\\nodejs\\node.exe'}}]}}}]},
  projects: [
    {id: 'p1', name: 'hibernate', projectPath: 'D:\\projects\\hibernate', snapshotCount: 12, lastObservedActive: now - 2000},
    {id: 'p2', name: 'design-system', projectPath: 'D:\\projects\\design-system', snapshotCount: 3, lastObservedActive: now - 86400000},
    {id: 'p3', name: 'Research reading', projectPath: '', snapshotCount: 0, lastObservedActive: null},
  ],
  memberships: [
    {client: 'vscode-a', target: 'vscode:hibernate', projectId: 'p1'},
    {client: 'chrome-a', target: 'chrome:41', projectId: 'p1'},
  ],
  memory: mem(5.4),
  windows: [
    {handle: '1', pid: 9120, title: 'hibernate - Visual Studio Code', executable: 'Code.exe', app: 'VS Code', foreground: true, left: 0, top: 0, width: 1600, height: 900, workingSet: 0.72 * GB, privateCommit: 0.6 * GB},
    {handle: '2', pid: 4412, title: 'Docs and 18 more - Google Chrome', executable: 'chrome.exe', app: 'Chrome', foreground: false, left: 0, top: 0, width: 1400, height: 800, workingSet: 1.24 * GB, privateCommit: 1.1 * GB},
    {handle: '3', pid: 771, title: 'hibernate - File Explorer', executable: 'explorer.exe', app: 'Explorer', foreground: false, left: 0, top: 0, width: 900, height: 600, workingSet: null, privateCommit: null},
  ],
  explorer: {items: [{handle: '3', url: 'D:\\projects\\hibernate', limitation: 'Context restore only'}], error: null},
  reports,
  snapshots: [
    snapshot('s-1', 'chrome', 'verified', {project: {id: 'p1', name: 'hibernate'}, tabs: new Array(21).fill({url: 'https://example.com', title: 'Tab'})}, mem(7.3)),
    snapshot('s-2', 'vscode', 'captured', {project: {id: 'p1', name: 'hibernate'}, groups: []}),
    snapshot('s-3', 'context', 'captured', {project: {id: 'p1', name: 'hibernate'}, explorer: {items: [{url: 'D:\\projects\\hibernate'}]}, terminal: {items: [{cwd: 'D:\\projects\\hibernate', verifiedPrompt: true}]}}),
  ],
  jobs: [{id: 'j1', state: 'done', action: 'capture', result: 'Captured 3 contexts.'}],
  workspace: {id: 'p1', name: 'hibernate', projectPath: 'D:\\projects\\hibernate'},
};

export function mockDashboard(stage: string): Dashboard {
  const d: Dashboard = JSON.parse(JSON.stringify(base));
  d.memory = mem(5.4);
  if (stage === 'connect') {
    d.reports = []; d.memberships = []; d.snapshots = []; d.services = []; d.serviceHealth = null; d.jobs = [];
    return d;
  }
  if (stage === 'capture') return d;
  if (stage === 'sleep') {d.checkpoints = [checkpoint({})]; return d;}
  if (stage === 'asleep') {d.checkpoints = [checkpoint({close: true})]; d.memory = mem(7.3); return d;}
  d.checkpoints = [checkpoint({close: true, restore: true})];
  d.memory = mem(6.8);
  return d;
}

export function installMockBridge() {
  const stage = new URLSearchParams(location.search).get('stage') ?? 'ready';
  const w = window as unknown as Record<string, unknown>;
  w.isTauri = true;
  w.__TAURI_INTERNALS__ = {
    async invoke(cmd: string) {
      if (cmd === 'dashboard') return mockDashboard(stage);
      if (cmd === 'discover_projects') return {checkedAt: Date.now(), detail: '', candidates: [{name: 'api-gateway', projectPath: 'D:\\projects\\api-gateway', existingProjectId: null, repository: {branch: 'main'}, evidence: [{source: 'terminal', label: 'Instrumented prompt folder', path: 'D:\\projects\\api-gateway'}]}]};
      if (cmd === 'snapshot_history') return mockDashboard(stage).snapshots;
      if (cmd === 'review_checkpoint_close') return {
        id: 'review-1', created: Date.now(), state: 'ready',
        body: {
          project: {name: 'hibernate', projectPath: 'D:\projects\hibernate'}, phase: 'review', expiresAt: Date.now() + 120000, detail: '',
          steps: [
            {id: 'e1', kind: 'chrome', label: 'Chrome - 21 tabs, 3 groups', eligible: true, state: 'ready', detail: 'No download is running and every tab has a saved destination.', snapshot: {id: 's-1', client: 'chrome-a', adapter: 'chrome', state: 'captured', created: Date.now() - 600000, before: mem(5.4), after: null, result: null, body: {tabs: [{title: 'ContextSleep on Product Hunt', url: 'https://www.producthunt.com/posts/contextsleep'}, {title: 'Windows memory APIs', url: 'https://learn.microsoft.com/windows/win32/memory'}, {title: 'localhost:3000', url: 'http://localhost:3000/'}]}}},
            {id: 'e2', kind: 'vscode', label: 'VS Code - hibernate', eligible: true, state: 'ready', detail: 'No unsaved editor, task, debug session or terminal is open.', snapshot: {id: 's-2', client: 'vscode-a', adapter: 'vscode', state: 'captured', created: Date.now() - 600000, before: mem(5.4), after: null, result: null, body: {groups: [{tabs: [{label: 'App.tsx', uri: 'file:///D:/projects/hibernate/apps/desktop/src/App.tsx'}, {label: 'style.css', uri: 'file:///D:/projects/hibernate/apps/desktop/src/style.css'}]}]}}},
            {id: 'e3', kind: 'vscode', label: 'VS Code - design-system', eligible: false, state: 'retained', detail: 'Two workbench windows share one process, so ContextSleep cannot prove which window belongs to this workspace.'},
            {id: 'e4', kind: 'context', label: 'Explorer folder - D:\projects\hibernate', eligible: false, state: 'retained', detail: 'Explorer windows are context restore only. ContextSleep never closes them.'},
          ],
        },
      };
      return 'Preview mode: no engine is connected.';
    },
    convertFileSrc: (p: string) => p,
  };
}
