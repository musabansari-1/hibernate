import { ChromeStateSchema, chromeSafety, compareTabs, restorableUrl, type Adapter, type ChromeState } from '../../../packages/core/model';
const base = chrome.runtime.getURL('');
const fixturePrefix = base + 'fixture.html?tab=';
let restoring = false;
let profile = '';
export async function browserSession(){const data=await chrome.storage.session.get('browserSession');if(typeof data.browserSession==='string')return data.browserSession;const id=crypto.randomUUID();await chrome.storage.session.set({browserSession:id});return id;}
export async function identity() { if (!profile) {
    const data = await chrome.storage.local.get('profile');
    profile = typeof data.profile === 'string' ? data.profile : crypto.randomUUID();
    await chrome.storage.local.set({ profile });
} return profile; }
async function lazyItem(token: string): Promise<{
    url: string;
    title: string;
} | undefined> { const data = await chrome.storage.local.get(`lazy:${token}`); const item = data[`lazy:${token}`] as {
    url?: unknown;
    title?: unknown;
} | undefined; return item && typeof item.url === 'string' && typeof item.title === 'string' ? { url: item.url, title: item.title } : undefined; }
async function semantic(tab: chrome.tabs.Tab): Promise<ChromeState['tabs'][number]> {
    let url = tab.pendingUrl ?? tab.url ?? '';
    let title = tab.title ?? '';
    if (url.startsWith(base + 'placeholder.html#')) {
        const token = url.split('#')[1];
        const item = await lazyItem(token);
        if (!item)
            throw new Error('A sleeping tab has lost its local mapping. Capture aborted.');
        url = item.url;
        title = item.title;
    }
    return { url, title, index: tab.index, active: tab.active, pinned: tab.pinned, group: tab.groupId, fixture: url.startsWith(fixturePrefix) && /^\d+$/.test(url.slice(fixturePrefix.length)) };
}
export const adapter: Adapter<ChromeState> = {
    async capture(target) {
        const win = await chrome.windows.get(Number(target), { populate: true });
        if (win.incognito || win.type !== 'normal' || !win.tabs?.length)
            throw new Error('Only normal, non-incognito windows are supported.');
        const tabs = await Promise.all(win.tabs.map(semantic));
        if (tabs.some(t => !restorableUrl(t.url, base)))
            throw new Error('This window contains unsupported or credential-bearing URLs. Separate HTTP(S) tabs into a workspace window first.');
        const groups = await chrome.tabGroups.query({ windowId: win.id });
        return ChromeStateSchema.parse({ version: 1, adapter: 'chrome', profile: await identity(), browserSession:await browserSession(), sourceWindow: win.id, tabs, groups: groups.map(g => ({ id: g.id, title: g.title ?? '', color: g.color, collapsed: g.collapsed })), geometry: { left: win.left ?? 0, top: win.top ?? 0, width: win.width ?? 1000, height: win.height ?? 800, state: win.state ?? 'normal' }, capturedAt: Date.now() });
    },
    async safety(state) { return chromeSafety(state, fixturePrefix, (await chrome.downloads.search({ state: 'in_progress' })).length); },
    async close(state) {
        // A snapshot is insufficient authorization: re-read the exact window immediately before removal.
        const fresh = await this.capture(String(state.sourceWindow));
        const safety = await this.safety(fresh);
        if (!safety.safe || compareTabs(state.tabs, fresh.tabs).length || JSON.stringify(state.groups) !== JSON.stringify(fresh.groups))
            throw new Error('Window changed or is unsafe. Capture again.');
        await chrome.windows.remove(state.sourceWindow);
        if ((await chrome.windows.getAll()).some(w => w.id === state.sourceWindow))
            throw new Error('Window is still open. Close was not verified.');
    },
    async restore(state, id) {
        if (state.profile !== await identity())
            throw new Error('Reconnect the original Chrome profile. Cross-profile restoration is blocked.');
        const marker = `restore:${id}`;
        const sessionData = await chrome.storage.session.get('browserSession');
        const session = typeof sessionData.browserSession === 'string' ? sessionData.browserSession : crypto.randomUUID();
        await chrome.storage.session.set({ browserSession: session });
        const prior = (await chrome.storage.local.get(marker))[marker] as {
            windowId: number;
            complete: boolean;
            session: string;
        } | undefined;
        if (prior && prior.session === session) {
            try {
                await chrome.windows.get(prior.windowId);
                return prior;
            }
            catch {
                if (prior.complete === false)
                    throw new Error('An interrupted restore needs inspection. Existing tabs will not be duplicated automatically.');
            }
        }
        restoring = true;
        try {
            const urls: string[] = [];
            for (let i = 0; i < state.tabs.length; i++) {
                const tab = state.tabs[i];
                if (!restorableUrl(tab.url, base))
                    throw new Error('Snapshot contains an unsupported URL.');
                const token = `${id}:${i}`;
                await chrome.storage.local.set({ [`lazy:${token}`]: { url: tab.url, title: tab.title } });
                urls.push(tab.active ? tab.url : base + 'placeholder.html#' + token);
            }
            const win = await chrome.windows.create({ url: urls[0], focused: false, type: 'normal', width: Math.min(state.geometry.width, 1600), height: Math.min(state.geometry.height, 1100) });
            if (!win?.id || !win.tabs?.[0]?.id)
                throw new Error('Chrome did not return a new window.');
            const receipt = { windowId: win.id, complete: false, session };
            await chrome.storage.local.set({ [marker]: receipt });
            const ids = [win.tabs[0].id];
            await chrome.tabs.update(ids[0], { pinned: state.tabs[0].pinned });
            for (let i = 1; i < urls.length; i++) {
                const tab = await chrome.tabs.create({ windowId: win.id, url: urls[i], index: i, active: false, pinned: state.tabs[i].pinned });
                if (tab.id === undefined)
                    throw new Error('Tab creation failed.');
                ids.push(tab.id);
            }
            // Without windowId Chrome moves members to the current window, which need not be this unfocused restore window.
            for (const g of state.groups) {
                const members = ids.filter((_, i) => state.tabs[i].group === g.id);
                if (members.length) {
                    const gid = await chrome.tabs.group({ tabIds: members as [
                            number,
                            ...number[]
                        ], createProperties: { windowId: win.id } });
                    await chrome.tabGroups.update(gid, { title: g.title, color: g.color, collapsed: g.collapsed });
                }
            }
            const active = state.tabs.findIndex(t => t.active);
            await chrome.tabs.update(ids[Math.max(0, active)], { active: true });
            // Let Chrome clamp geometry to the current desktop. Monitor identity is not yet preserved.
            await chrome.windows.update(win.id, { state: 'normal', left: state.geometry.left, top: state.geometry.top, width: state.geometry.width, height: state.geometry.height });
            if (['maximized', 'fullscreen', 'minimized'].includes(state.geometry.state))
                await chrome.windows.update(win.id, { state: state.geometry.state });
            receipt.complete = true;
            await chrome.storage.local.set({ [marker]: receipt });
            return receipt;
        }
        finally {
            restoring = false;
        }
    },
    async verify(state, receipt) {
        const actual = await this.capture(String((receipt as {
            windowId: number;
        }).windowId));
        const details = compareTabs(state.tabs, actual.tabs);
        // IDs are transient. Compare group metadata and membership by tab position.
        const shape = (s: ChromeState) => s.groups.map(g => ({ title: g.title, color: g.color, collapsed: g.collapsed, members: s.tabs.map((t, i) => t.group === g.id ? i : -1).filter(i => i >= 0) })).sort((a, b) => (a.members[0] ?? 0) - (b.members[0] ?? 0));
        if (JSON.stringify(shape(state)) !== JSON.stringify(shape(actual)))
            details.push('Group metadata or membership differs.');
        const matched=state.tabs.filter((t,i)=>{const a=actual.tabs[i];return a&&t.url===a.url&&t.pinned===a.pinned&&t.active===a.active;}).length;
        const expectedGroups=shape(state),observedGroups=shape(actual);
        const groupsMatched=expectedGroups.filter(g=>observedGroups.some(a=>JSON.stringify(a)===JSON.stringify(g))).length;
        return { verified: details.length === 0, facts:[{capability:'Chrome tab URLs, order, pins and active state',expected:state.tabs.length,observed:matched,status:matched===state.tabs.length&&actual.tabs.length===state.tabs.length?'verified':'mismatch'},{capability:'Chrome groups and membership',expected:expectedGroups.length,observed:groupsMatched,status:JSON.stringify(expectedGroups)===JSON.stringify(observedGroups)?'verified':'mismatch'}], details: details.length ? details : ['Tab URLs/order, active tab, pinned tabs and groups verified. Background page content is deferred. Geometry and page content are not verified.'] };
    }
};
export async function revalidateReviewed(state:ChromeState){
    if(state.browserSession!==await browserSession())throw new Error('Browser restarted since capture. Capture the live window again before closing.');
    if(state.profile!==await identity())throw new Error('The browser profile changed. Capture again.');
    const fresh=await adapter.capture(String(state.sourceWindow));
    if(compareTabs(state.tabs,fresh.tabs).length||JSON.stringify(state.groups)!==JSON.stringify(fresh.groups))throw new Error('Tabs or groups changed since review. Capture again; nothing was closed.');
    if((await chrome.downloads.search({state:'in_progress'})).length)throw new Error('A download is in progress. Nothing was closed.');
    const tabs=await chrome.tabs.query({windowId:state.sourceWindow});
    if(tabs.some(t=>t.audible||t.status==='loading'||t.pendingUrl))throw new Error('A tab is playing audio or navigating. Wait until it is idle and capture again.');
    return tabs;
}
export async function closeReviewed(state:ChromeState){
    await revalidateReviewed(state);
    // Chrome's own close flow surfaces beforeunload dialogs. Never accept those dialogs for the user.
    await chrome.windows.remove(state.sourceWindow);
    const deadline=Date.now()+30000;
    do{if(!(await chrome.windows.getAll()).some(w=>w.id===state.sourceWindow))return {closed:true,detail:'Reviewed Chrome window closed and disappearance verified. Page contents and sign-in state were not captured.'};await new Promise(r=>setTimeout(r,250));}while(Date.now()<deadline);
    throw new Error('Chrome is still open: close was canceled or requires your response. The saved snapshot remains available.');
}
export async function quickSleep(state:ChromeState){
    const tabs=await revalidateReviewed(state);let parked=0;const skipped:string[]=[];
    for(const tab of tabs){
        if(tab.active||tab.pinned||tab.audible||!tab.autoDiscardable){skipped.push(`Tab ${tab.index+1}: active, pinned, playing audio or protected by Chrome`);continue;}
        if(tab.discarded){parked++;continue;}
        try{await chrome.tabs.discard(tab.id);if((await chrome.tabs.get(tab.id!)).discarded)parked++;else skipped.push(`Tab ${tab.index+1}: Chrome kept the page loaded`);}catch{skipped.push(`Tab ${tab.index+1}: Chrome refused to discard`);}
    }
    return {parked,total:tabs.length,skipped,detail:'Quick Sleep discards eligible background pages. Selecting them reloads them. Browser processes remain alive; this is not Deep Hibernate or a guaranteed RAM saving.'};
}
export async function fixture() {
    const w = await chrome.windows.create({ url: fixturePrefix + '1', focused: true });
    if (!w?.id)
        throw new Error('Could not create validation window');
    const ids: number[] = [];
    for (let i = 2; i <= 12; i++) {
        const tab = await chrome.tabs.create({ windowId: w.id, url: fixturePrefix + i, active: false });
        if (tab.id)
            ids.push(tab.id);
    }
    if (w.tabs?.[0]?.id)
        await chrome.tabs.update(w.tabs[0].id, { pinned: true });
    const g = await chrome.tabs.group({ tabIds: ids.slice(0, 5) as [
            number,
            ...number[]
        ], createProperties: { windowId: w.id } });
    await chrome.tabGroups.update(g, { title: 'Validation docs', color: 'green', collapsed: false });
    return 'Created 12 validation tabs (8 MiB explicit test buffer per page). Select that window in the dashboard.';
}
export async function wake(tabId: number, token: string) { const item = await lazyItem(token); if (!item || !restorableUrl(item.url, base))
    throw new Error('Original URL unavailable or unsupported.'); const current = await chrome.tabs.get(tabId); if (current.url !== base + 'placeholder.html#' + token)
    throw new Error('Tab changed.'); await chrome.tabs.update(tabId, { url: item.url }); }
chrome.tabs.onActivated.addListener(info => { if (restoring)
    return; void chrome.tabs.get(info.tabId).then(t => { if (t.url?.startsWith(base + 'placeholder.html#'))
    return wake(info.tabId, t.url.split('#')[1]); }).catch(() => { }); });
