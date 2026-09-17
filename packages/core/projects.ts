import type { Report } from './model';

// Numeric browser window IDs can be reused after a browser restart. A selection
// remains valid only while the same live session still reports that window.
export function pruneSelectedTargets(selected: string[], previous: Report[], current: Report[], now: number): string[] {
    const identities = (reports: Report[]) => new Map(reports.flatMap(report => report.body.items.map(item =>
        [JSON.stringify([report.client, item.id]), item.membershipKey ?? item.id] as const)));
    const before = identities(previous);
    const live = identities(current.filter(report => now - report.at < 15000));
    return [...new Set(selected)].filter(key => live.has(key) && before.get(key) === live.get(key));
}

export type Compatibility = 'Full Hibernate' | 'Context Restore' | 'Window Restore' | 'Experimental' | 'Unsupported';
export type ProjectSignal = {
    source: 'vscode' | 'terminal' | 'explorer' | 'browser';
    label: string;
    path?: string;
    url?: string;
    repository?: string;
};
export type Association = {
    signal: ProjectSignal;
    strength: 'exact' | 'suggestion' | 'unassigned';
    reason: string;
};
export function normalizeWindowsPath(raw: string): string | null {
    let path = raw;
    if (path.startsWith('file:')) {
        try {
            const u = new URL(path);
            if (u.hostname)
                return null;
            path = decodeURIComponent(u.pathname).replace(/^\/([a-z]:)/i, '$1');
        }
        catch {
            return null;
        }
    }
    path = path.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
    if (!/^[a-z]:\\/.test(path) || path.split('\\').some(p => p === '..' || p === '.'))
        return null;
    return path;
}
export function associate(projectPath: string, signals: ProjectSignal[], repository?: string): Association[] {
    const root = normalizeWindowsPath(projectPath);
    return signals.map(signal => {
        const path = signal.path ? normalizeWindowsPath(signal.path) : null;
        if (root && path && (path === root || path.startsWith(root + '\\')))
            return { signal, strength: 'exact', reason: 'Application reports a folder inside this project.' };
        if (repository && signal.url) {
            try {
                const u = new URL(signal.url);
                if (u.hostname === 'github.com' && u.pathname.toLowerCase().replace(/^\//, '').startsWith(repository.toLowerCase() + '/'))
                    return { signal, strength: 'suggestion', reason: 'GitHub repository matches; confirm browser membership.' };
                if (u.hostname === 'github.com' && u.pathname.replace(/^\/|\/$/g, '').toLowerCase() === repository.toLowerCase())
                    return { signal, strength: 'suggestion', reason: 'GitHub repository matches; confirm browser membership.' };
            }
            catch { /* An invalid URL is not evidence. */ }
        }
        if (signal.url) {
            try {
                const u = new URL(signal.url);
                if (['localhost', '127.0.0.1', '[::1]'].includes(u.hostname))
                    return { signal, strength: 'suggestion', reason: `Local service ${u.port || (u.protocol === 'https:' ? '443' : '80')} detected. Port ownership is not yet linked to this project.` };
            }
            catch { }
        }
        return { signal, strength: 'unassigned', reason: 'No reliable project relationship found. User assignment is required.' };
    });
}
export type VerificationFact = {
    capability: string;
    expected: number | null;
    observed: number | null;
    status: 'verified' | 'missing' | 'unsupported' | 'deferred';
    detail: string;
};
// Readiness is a collection of evidence, never a fabricated completeness percentage.
export function readyToContinue(facts: VerificationFact[]): boolean { return facts.length > 0 && facts.every(f => f.status === 'verified'); }
