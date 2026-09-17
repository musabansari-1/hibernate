import { describe, expect, it } from 'vitest';
import { chromeSafety, compareTabs, restorableUrl, ChromeStateSchema, type ChromeState, type Report } from '../packages/core/model';
import { associate, normalizeWindowsPath, pruneSelectedTargets, readyToContinue } from '../packages/core/projects';
import { interruptedRecoveryKey } from '../packages/core/recovery';
const prefix = 'chrome-extension://abcdefghijklmnop/fixture.html?tab=';
function state(): ChromeState { return { version: 1, adapter: 'chrome', profile: 'd1149ba0-4f30-416c-a643-fbabc058c8d1', sourceWindow: 3, capturedAt: 1, tabs: [{ url: prefix + '1', title: 'Test', index: 0, active: true, pinned: false, group: -1, fixture: true }], groups: [], geometry: { left: 0, top: 0, width: 1000, height: 800, state: 'normal' } }; }
describe('close safety', () => {
    it('permits only owned validation pages with no active downloads', () => { expect(chromeSafety(state(), prefix, 0).safe).toBe(true); expect(chromeSafety(state(), prefix, 1).safe).toBe(false); });
    it('blocks live pages even when a stale fixture flag says safe', () => { const s = state(); s.tabs[0].url = 'https://example.com'; expect(chromeSafety(s, prefix, 0).safe).toBe(false); });
    it('rejects executable and credential-bearing URLs', () => { for (const u of ['javascript:alert(1)', 'data:text/html,hello', 'file:///C:/secret', 'https://user:password@example.com'])
        expect(restorableUrl(u)).toBe(false); expect(restorableUrl('https://example.com/path')).toBe(true); });
    it('rejects another extension origin', () => expect(restorableUrl(prefix + '1', 'chrome-extension://other/')).toBe(false));
    it('rejects future snapshot schemas', () => expect(ChromeStateSchema.safeParse({ ...state(), version: 2 }).success).toBe(false));
});
describe('verification', () => {
    it('detects count, order, pin and active differences', () => { const s = state(); const actual = structuredClone(s.tabs); actual[0].pinned = true; expect(compareTabs(s.tabs, actual)).toHaveLength(1); expect(compareTabs(s.tabs, []).length).toBeGreaterThan(0); expect(compareTabs(s.tabs, s.tabs)).toEqual([]); });
    it('does not consider unsupported or deferred capabilities ready', () => { expect(readyToContinue([])).toBe(false); expect(readyToContinue([{ capability: 'tabs', expected: 10, observed: 10, status: 'deferred', detail: 'Only placeholders' }])).toBe(false); });
});
describe('project understanding', () => {
    const report = (session: string, ids: string[], at = 20000): Report => ({ client: 'browser', adapter: 'chrome', at, body: { label: 'Chrome', items: ids.map(id => ({ id, membershipKey: `${session}:${id}`, title: id, detail: '', compatibility: 'Context Restore' })) } });
    const key = (id: string) => JSON.stringify(['browser', id]);
    it('drops closed and disconnected capture targets while retaining live selections', () => {
        const previous = [report('original', ['1', '2'])];
        expect(pruneSelectedTargets([key('1'), key('2')], previous, [report('original', ['2'])], 21000)).toEqual([key('2')]);
        expect(pruneSelectedTargets([key('2')], previous, [report('original', ['2'])], 35000)).toEqual([]);
    });
    it('does not transfer selection to a reused browser window ID after restart', () => {
        expect(pruneSelectedTargets([key('1')], [report('original', ['1'])], [report('restarted', ['1'])], 21000)).toEqual([]);
    });
    it('normalizes local file URIs and Windows case', () => expect(normalizeWindowsPath('file:///D:/Projects/My%20App')).toBe('d:\\projects\\my app'));
    it('respects folder boundaries', () => { const result = associate('D:\\projects\\app', [{ source: 'terminal', label: 'shell', path: 'D:\\projects\\app\\src' }, { source: 'vscode', label: 'other', path: 'D:\\projects\\apple' }]); expect(result.map(r => r.strength)).toEqual(['exact', 'unassigned']); });
    it('does not infer ownership from localhost alone', () => expect(associate('D:\\app', [{ source: 'browser', label: 'dev', url: 'http://localhost:3000' }])[0].strength).toBe('suggestion'));
    it('rejects ambiguous traversal and network paths', () => { expect(normalizeWindowsPath('D:\\app\\..\\other')).toBeNull(); expect(normalizeWindowsPath('file://server/share')).toBeNull(); });
});
describe('recovery visibility', () => {
    const base = { jobs: [], checkpoints: [] } as Pick<import('../packages/core/model').Dashboard, 'jobs' | 'checkpoints'>;
    it('stays quiet when no durable operation was interrupted', () => expect(interruptedRecoveryKey(base)).toBe(''));
    it('changes only for durable interrupted work', () => {
        const data = structuredClone(base);
        data.jobs.push({ id: 'j2', state: 'done', action: 'capture', result: null }, { id: 'j1', state: 'interrupted', action: 'close', result: null });
        data.checkpoints.push({ id: 'c1', created: 1, state: 'interrupted', body: { entries: [], captures: [] }, runs: [{ id: 'r1', created: 1, state: 'interrupted', body: { steps: [] } }], hibernations: [] } as never);
        expect(interruptedRecoveryKey(data)).toBe('checkpoint:c1|job:j1|restore:r1');
    });
});
