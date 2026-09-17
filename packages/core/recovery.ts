import type { Dashboard } from './model';

export function interruptedRecoveryKey(data: Pick<Dashboard, 'jobs' | 'checkpoints'>): string {
    const ids = new Set<string>();
    for (const job of data.jobs) {
        if (job.state === 'interrupted') ids.add(`job:${job.id}`);
    }
    for (const checkpoint of data.checkpoints) {
        if (checkpoint.state === 'interrupted') ids.add(`checkpoint:${checkpoint.id}`);
        for (const run of checkpoint.runs) {
            if (run.state === 'interrupted') ids.add(`restore:${run.id}`);
        }
        for (const run of checkpoint.hibernations) {
            if (run.state === 'interrupted') ids.add(`hibernate:${run.id}`);
        }
    }
    return [...ids].sort().join('|');
}
