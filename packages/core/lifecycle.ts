import type { Adapter } from './model';
export interface HibernateJournal<T> {
    persist(state: T): Promise<{
        id: string;
    }>;
    transition(id: string, state: 'blocked' | 'closing' | 'closed' | 'partial', result: unknown): Promise<void>;
    settleResources(): Promise<void>;
}
/** Persistence is an acknowledged barrier, not a background task. Any uncertainty stops the close path. */
export async function deepHibernate<T>(adapter: Adapter<T>, target: string, journal: HibernateJournal<T>): Promise<{
    id: string;
    detail: string;
}> {
    const captured = await adapter.capture(target);
    const snapshot = await journal.persist(captured);
    const safety = await adapter.safety(captured);
    if (!safety.safe) {
        await journal.transition(snapshot.id, 'blocked', safety.reasons);
        throw new Error(safety.reasons.join(' '));
    }
    // Record intent before the side effect: a crash must never look like an untouched capture.
    await journal.transition(snapshot.id, 'closing', 'Capture durable; close pending. Inspect live state if interrupted.');
    try {
        await adapter.close(captured);
        await journal.settleResources();
        const detail = 'Window closure verified. Memory sampled after settling; system-wide changes include unrelated activity.';
        await journal.transition(snapshot.id, 'closed', detail);
        return { id: snapshot.id, detail };
    }
    catch (error) {
        await journal.transition(snapshot.id, 'partial', `Close or measurement incomplete. Inspect live windows before retrying. ${String(error)}`).catch(() => { });
        throw error;
    }
}
