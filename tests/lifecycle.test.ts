import { expect, it, vi } from 'vitest';
import { deepHibernate } from '../packages/core/lifecycle';
function setup() {
    const steps: string[] = [];
    const adapter = { capture: vi.fn(async () => { steps.push('capture'); return { context: 'project' }; }), safety: vi.fn(async () => { steps.push('safety'); return { safe: true, reasons: [] as string[] }; }), close: vi.fn(async () => { steps.push('close'); }), restore: vi.fn(), verify: vi.fn() };
    const journal = { persist: vi.fn(async () => { steps.push('persist'); return { id: 'saved' }; }), transition: vi.fn(async (_id: string, state: string) => { steps.push(state); }), settleResources: vi.fn(async () => { steps.push('measure'); }) };
    return { adapter, journal, steps };
}
it('persists before safety/close and only records closed after observation', async () => { const t = setup(); await deepHibernate(t.adapter, 'window', t.journal); expect(t.steps).toEqual(['capture', 'persist', 'safety', 'closing', 'close', 'measure', 'closed']); });
it('never closes when snapshot persistence fails', async () => { const t = setup(); t.journal.persist.mockRejectedValue(new Error('Disk full')); await expect(deepHibernate(t.adapter, 'window', t.journal)).rejects.toThrow('Disk full'); expect(t.adapter.close).not.toHaveBeenCalled(); });
it('records a safety block while keeping the snapshot', async () => { const t = setup(); t.adapter.safety.mockResolvedValue({ safe: false, reasons: ['Unsaved work'] }); await expect(deepHibernate(t.adapter, 'window', t.journal)).rejects.toThrow('Unsaved work'); expect(t.steps).toEqual(['capture', 'persist', 'blocked']); expect(t.adapter.close).not.toHaveBeenCalled(); });
it('never closes if the durable intent cannot be written', async () => { const t = setup(); t.journal.transition.mockRejectedValue(new Error('Read-only disk')); await expect(deepHibernate(t.adapter, 'window', t.journal)).rejects.toThrow(); expect(t.adapter.close).not.toHaveBeenCalled(); });
it('does not report closure after cancellation or timeout', async () => { const t = setup(); t.adapter.close.mockRejectedValue(new Error('Still open')); await expect(deepHibernate(t.adapter, 'window', t.journal)).rejects.toThrow(); expect(t.steps).not.toContain('closed'); expect(t.steps).toContain('partial'); expect(t.journal.settleResources).not.toHaveBeenCalled(); });
it('does not fabricate measurements when observation fails', async () => { const t = setup(); t.journal.settleResources.mockRejectedValue(new Error('No sample')); await expect(deepHibernate(t.adapter, 'window', t.journal)).rejects.toThrow(); expect(t.steps).not.toContain('closed'); expect(t.steps).toContain('partial'); });
