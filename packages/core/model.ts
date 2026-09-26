import { z } from 'zod';
export const TabSchema = z.object({ url: z.string().max(16384), title: z.string().max(2048), index: z.number().int().nonnegative(), active: z.boolean(), pinned: z.boolean(), group: z.number().int(), fixture: z.boolean() });
export const ChromeStateSchema = z.object({ version: z.literal(1), adapter: z.literal('chrome'), profile: z.string().uuid(), browserSession:z.string().uuid().optional(), sourceWindow: z.number().int(), tabs: z.array(TabSchema).min(1).max(500), groups: z.array(z.object({ id: z.number().int(), title: z.string(), color: z.enum(['grey', 'blue', 'red', 'yellow', 'green', 'pink', 'purple', 'cyan', 'orange']), collapsed: z.boolean() })), geometry: z.object({ left: z.number().int(), top: z.number().int(), width: z.number().int().positive(), height: z.number().int().positive(), state: z.enum(['normal', 'minimized', 'maximized', 'fullscreen', 'locked-fullscreen']) }), capturedAt: z.number() });
export type ChromeState = z.infer<typeof ChromeStateSchema>;
export type Memory = {
    at: number;
    totalPhysical: number;
    availablePhysical: number;
    chromeWorkingSet: number;
    chromePrivateCommit: number;
    unreadableProcesses: number;
    sourceBrowserPid?: number | null;
    sourceBrowserWorkingSet?: number | null;
    sourceBrowserPrivateCommit?: number | null;
    sourceBrowserUnreadable?: number | null;
};
export type WindowInfo = {
    handle: string;
    pid: number;
    title: string;
    executable: string;
    app: string;
    foreground: boolean;
    left: number;
    top: number;
    width: number;
    height: number;
    workingSet: number | null;
    privateCommit: number | null;
};
export type Report = {
    client: string;
    adapter: 'chrome' | 'vscode' | 'windows-generic';
    at: number;
    body: {
        label: string;
        signals?: import('./projects').ProjectSignal[];
        items: {
            id: string;
            title: string;
            detail: string;
            compatibility: string;
            membershipKey?:string;
            focused?:boolean;
            capturable?:boolean;
            nativeGroup?:string | null;
        }[];
    };
};
export type Snapshot = {
    archived?:boolean;
    id: string;
    client: string;
    adapter: string;
    state: string;
    created: number;
    body: unknown;
    before: Memory;
    after: Memory | null;
    result: unknown;
};
export type Job = {
    id: string;
    action: 'capture' | 'hibernate' | 'resume' | 'fixture' | 'close' | 'quick_sleep' | 'preflight_close';
    target?: string;
    snapshot?: Snapshot;
};
export type Dashboard = {
    apps?: {at:number;error:string|null;items:NativeApp[]};
    adapterCatalog?: {id:string;label:string;reviewedClose:boolean;semanticRestore:boolean;verification:string}[];
    checkpoints: ProjectCheckpoint[];
    chromeLaunches:{client:string;profileDirectory:string;executable:string;userDataDir?:string|null;extensionId:string;configuredAt:number}[];
    services:{id:string;name:string;url:string;launch?:{executable:string;arguments:string[];cwd:string};launchRun?:{state:string;detail?:string;verified?:boolean}}[];
    serviceHealth?:{checkedAt:number;services:{id:string;name:string;observation:{status:string;detail:string;httpStatus:number|null;url:string;listenerOwnership?:unknown}}[]}|null;
    projects: { id: string; name: string; projectPath: string; snapshotCount:number; lastObservedActive:number|null }[];
    memberships: { client: string; target: string; projectId: string }[];
    memory: Memory;
    windows: WindowInfo[];
    explorer: {
        items: {
            handle: string;
            url: string;
            limitation: string;
        }[];
        error: string | null;
    };
    reports: Report[];
    snapshots: Snapshot[];
    jobs: {
        id: string;
        state: string;
        action: string;
        result: unknown;
    }[];
    workspace: {
        id: string;
        name: string;
        projectPath: string;
    };
};
export type ReconstructionAssessment = {version:1;disposition:'restore-only'|'keep-open';summary:string;discovered:boolean;processIdentityProven:boolean;identityProven:boolean;relaunchRecipeAvailable:boolean;windowPlacementRestorable:boolean;windowPlacementRequestAvailable:boolean;documentContextRestorable:boolean;unsavedStateRestorable:boolean;safeCloseAllowed:boolean;restoreVerifiable:boolean;verificationScope:string;semanticAdapterRequired:boolean;blockers:string[]};
export type NativeApp = {id:string;label:string;group:string|null;enhancement:string|null;window:{title:string;handle:string;class:string;owner:{pid:number;processStarted:string;executable:string}|null;sameOwnerWindows:number;appUserModelId:string|null};capabilities:{contextCapture:boolean;reviewedClose:false;semanticRestore:false;limitation:string;reconstruction?:ReconstructionAssessment}};
export type CheckpointEntry={id:string;snapshotId?:string;kind:string;index?:number;label:string;detail?:string;executable?:string};
export type ProjectDiscovery={checkedAt:number;detail:string;explorerError?:string|null;candidates:{name:string;projectPath:string;existingProjectId:string|null;repository:{branch?:string|null;repository?:string|null};evidence:{source:string;label:string;path:string}[]}[]};
export type HibernationRun={id:string;created:number;state:string;body:{project:{name:string;projectPath:string};phase:string;expiresAt:number;detail:string;before?:Memory;after?:Memory|null;steps:(CheckpointEntry&{eligible:boolean;state:string;detail?:string;snapshot?:Snapshot;preflight?:unknown;result?:unknown})[]}};
export type ProjectCheckpoint={id:string;created:number;state:string;hibernations:HibernationRun[];body:{error?:string;importWarnings?:string[];project?:{name:string;projectPath:string};repository?:unknown;services?:{id:string;name:string;url:string;launch?:{executable:string;arguments:string[];cwd:string};launchRun?:unknown;importWarning?:string}[];entries:CheckpointEntry[];captures:{state:string;detail?:string;snapshotId?:string}[]};runs:{id:string;created:number;state:string;body:{readiness?:{status:string;verified:boolean;detail:string;facts:{capability:string;kind:string;status:string}[]};steps:(CheckpointEntry&{state:string;detail?:string;result?:unknown;checkedAt?:number})[]}}[]};
export type Safety = {
    safe: boolean;
    reasons: string[];
};
export interface Adapter<T> {
    capture(target: string): Promise<T>;
    safety(state: T): Promise<Safety>;
    close(state: T): Promise<void>;
    restore(state: T, snapshotId: string): Promise<unknown>;
    verify(state: T, receipt: unknown): Promise<{
        verified: boolean;
        details: string[];
    }>;
}
// Only semantic URLs are stored. Tokens embedded in a URL can still be sensitive.
export function restorableUrl(raw: string, extensionOrigin?: string): boolean {
    try {
        const u = new URL(raw);
        return !u.username && !u.password && (['https:', 'http:'].includes(u.protocol) || (extensionOrigin !== undefined && u.origin === new URL(extensionOrigin).origin && raw.startsWith(extensionOrigin + 'fixture.html?')));
    }
    catch {
        return false;
    }
}
export function chromeSafety(state: ChromeState, fixturePrefix: string, downloads: number): Safety {
    const reasons: string[] = [];
    if (downloads)
        reasons.push('A download is in progress in this browser profile.');
    if (state.tabs.some(t => !t.fixture || !t.url.startsWith(fixturePrefix)))
        reasons.push('Live page state cannot be proven safe. Automatic close is limited to the dedicated validation pages. Capture and lazy restore remain available.');
    return { safe: reasons.length === 0, reasons };
}
export function compareTabs(expected: ChromeState['tabs'], actual: ChromeState['tabs']): string[] {
    const errors: string[] = [];
    if (expected.length !== actual.length)
        errors.push(`Expected ${expected.length} tabs; found ${actual.length}.`);
    expected.forEach((tab, i) => { const a = actual[i]; if (!a || ['url', 'pinned', 'active'].some(k => tab[k as keyof typeof tab] !== a[k as keyof typeof a]))
        errors.push(`Tab ${i + 1}: URL, order, pin or active state differs.`); });
    return errors;
}
