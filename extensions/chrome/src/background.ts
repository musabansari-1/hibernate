import { ChromeStateSchema, type Job, type Snapshot } from '../../../packages/core/model';
import { adapter, fixture, identity, wake, closeReviewed, quickSleep, browserSession, revalidateReviewed } from './adapter';
import { deepHibernate } from '../../../packages/core/lifecycle';
const HOST = 'local.projecthibernate.bridge';
let port: chrome.runtime.Port | undefined;
let busy = false;
let connecting = false;
let profile = '';
const pending = new Map<string, {
    resolve: (v: any) => void;
    reject: (e: Error) => void;
    timer: ReturnType<typeof setTimeout>;
}>();
function rpc(op: string, body: Record<string, unknown> = {}): Promise<any> {
    if (!port)
        return Promise.reject(new Error('Native host disconnected. Install the host and reconnect.'));
    const requestId = crypto.randomUUID();
    return new Promise((resolve, reject) => { const timer = setTimeout(() => { pending.delete(requestId); reject(new Error('Native host timed out. Nothing will be closed without a durable acknowledgement.')); }, 15000); pending.set(requestId, { resolve, reject, timer }); port!.postMessage({ requestId, client: profile, op, ...body }); });
}
async function connect() {
    if (port || connecting)
        return;
    connecting = true;
    try {
        profile = await identity();
        port = chrome.runtime.connectNative(HOST);
        port.onMessage.addListener(msg => { const p = pending.get(msg.requestId); if (!p)
            return; clearTimeout(p.timer); pending.delete(msg.requestId); msg.ok ? p.resolve(msg.value) : p.reject(new Error(msg.error)); });
        port.onDisconnect.addListener(() => { const message = chrome.runtime.lastError?.message ?? 'Native host disconnected'; port = undefined; for (const p of pending.values()) {
            clearTimeout(p.timer);
            p.reject(new Error(message));
        } pending.clear(); void chrome.action.setBadgeText({ text: '!' }); });
        await tick();
    }
    finally {
        connecting = false;
    }
}
async function run(job: Job) {
    let snapshot: Snapshot | undefined;
    try {
        let result: unknown;
        if (job.action === 'fixture')
            result = await fixture();
        else if(job.action==='preflight_close'){
            if(!job.snapshot)throw new Error('Snapshot missing');
            await revalidateReviewed(ChromeStateSchema.parse(job.snapshot.body));
            result={checked:true,snapshotId:job.snapshot.id,detail:'Current browser organization, session, downloads and navigation checked. Page work still requires user review.'};
        }
        else if (job.action === 'close' || job.action === 'quick_sleep') {
            snapshot=job.snapshot;
            if(!snapshot||snapshot.state!=='approved')throw new Error('A reviewed snapshot is required.');
            const state=ChromeStateSchema.parse(snapshot.body);
            await rpc('finishSnapshot',{id:snapshot.id,state:'closing',result:'Revalidating the reviewed window. Chrome may ask about unsaved work; respond in Chrome.'});
            result=job.action==='close'?await closeReviewed(state):await quickSleep(state);
            await new Promise(r=>setTimeout(r,2500));
            await rpc('finishSnapshot',{id:snapshot.id,state:job.action==='close'?'closed':'parked',result});
        }
        else if (job.action === 'resume') {
            snapshot = job.snapshot;
            if (!snapshot)
                throw new Error('Snapshot missing');
            const state = ChromeStateSchema.parse(snapshot.body);
            await rpc('finishSnapshot', { id: snapshot.id, state: 'restoring', result: 'Reconstructing browser organization; not yet verified.' });
            const receipt = await adapter.restore(state, snapshot.id);
            result = await adapter.verify(state, receipt);
            await rpc('finishSnapshot', { id: snapshot.id, state: (result as {
                    verified: boolean;
                }).verified ? 'verified' : 'incomplete', result });
        }
        else if (job.action === 'hibernate') {
            result = await deepHibernate(adapter, job.target ?? '', {
                persist: body => rpc('save', { body, jobId: job.id }),
                transition: async (id, state, result) => { await rpc('finishSnapshot', { id, state, result }); },
                settleResources: () => new Promise(r => setTimeout(r, 2500))
            });
        }
        else {
            const state = await adapter.capture(job.target ?? '');
            snapshot = await rpc('save', { body: state, jobId: job.id });
            result = 'Captured to local SQLite. No windows closed.';
        }
        await rpc('complete', { id: job.id, ok: true, result });
    }
    catch (e) {
        if(snapshot && (job.action==='close'||job.action==='quick_sleep'))await rpc('finishSnapshot',{id:snapshot.id,state:'partial',result:String(e)}).catch(()=>{});
        await rpc('complete', { id: job.id, ok: false, result: String(e) }).catch(() => { });
    }
}
async function tick() {
    if (!port || busy)
        return;
    busy = true;
    try {
        const windows = await chrome.windows.getAll({ populate: true, windowTypes: ['normal'] });
        const signals = windows.filter(w => !w.incognito).flatMap(w => w.tabs ?? []).flatMap(t => { try {
            const u = new URL(t.url ?? '');
            return ['github.com', 'localhost', '127.0.0.1', '[::1]'].includes(u.hostname) ? [{ source: 'browser', label: t.title ?? 'Project tab', url: u.origin + u.pathname }] : [];
        }
        catch {
            return [];
        } });
        const session=await browserSession();
        const body = { label: `Chrome profile ${profile.slice(0, 8)}`, extensionId:chrome.runtime.id, signals, items: windows.filter(w => !w.incognito).map(w => ({ id: String(w.id), membershipKey:`${session}:${w.id}`, focused:!!w.focused, title: w.tabs?.find(t => t.active)?.title ?? 'Chrome window', detail: `${w.tabs?.length ?? 0} tabs`, compatibility: 'Context Restore · reviewed close · lazy resume' })) };
        const job = await rpc('poll', { adapter: 'chrome', body });
        await chrome.action.setBadgeText({ text: '' });
        if (job)
            await run(job);
    }
    finally {
        busy = false;
    }
}
chrome.runtime.onMessage.addListener((msg, sender, reply) => {
    if (sender.id !== chrome.runtime.id)
        return;
    const action = msg.type === 'connect' ? connect() : msg.type === 'wake' && sender.tab?.id !== undefined ? wake(sender.tab.id, msg.token) : Promise.reject(new Error('Unknown request'));
    void action.then(() => reply({ ok: true }), e => reply({ error: String(e) }));
    return true;
});
chrome.alarms.onAlarm.addListener(() => { void connect().catch(() => { }); });
void chrome.alarms.create('reconnect', { periodInMinutes: 0.5 });
setInterval(() => { void tick().catch(() => { }); }, 2000);
void connect().catch(() => { });
