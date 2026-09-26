// Display helpers. Every string produced here is user-facing, so it stays plain ASCII
// and never invents precision the underlying observation does not have.

/** Windows Task Manager convention: binary units labelled GB. */
export const gb = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(1)} GB`;
export const gbExact = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(2)} GB`;

export const signedGb = (bytes: number) => `${bytes >= 0 ? '+' : '-'}${gbExact(Math.abs(bytes))}`;

export function relativeTime(at: number) {
  const seconds = Math.round((Date.now() - at) / 1000);
  if (seconds < 10) return 'just now';
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return new Date(at).toLocaleDateString();
}

export const dateTime = (at: number) => new Date(at).toLocaleString();

export const describe = (value: unknown) =>
  typeof value === 'string' ? value : JSON.stringify(value, null, 2);

export const plural = (count: number, one: string, many = `${one}s`) =>
  `${count} ${count === 1 ? one : many}`;

/** Tone drives colour only. It never changes what the underlying evidence says. */
export type Tone = 'neutral' | 'good' | 'ready' | 'warn' | 'bad' | 'info';

/**
 * Lifecycle states are stored as engine identifiers. The UI shows plain language,
 * but never softens a blocked or unverified outcome into a successful-looking one.
 */
export function closeStateLabel(state: string): { label: string; tone: Tone } {
  switch (state) {
    case 'closed-selected': return { label: 'Selected windows closed', tone: 'good' };
    case 'blocked': return { label: 'Blocked - nothing closed', tone: 'warn' };
    case 'partial': return { label: 'Stopped partway', tone: 'warn' };
    case 'interrupted': return { label: 'Interrupted', tone: 'warn' };
    case 'failed': return { label: 'Failed', tone: 'bad' };
    case 'expired': return { label: 'Expired - nothing closed', tone: 'neutral' };
    case 'review': return { label: 'Waiting for approval', tone: 'info' };
    default: return { label: state.replaceAll('-', ' ').replaceAll('_', ' '), tone: 'neutral' };
  }
}

export function stepStateLabel(state: string): { label: string; tone: Tone } {
  switch (state) {
    case 'closed': case 'done': case 'restored': case 'verified':
      return { label: 'Verified', tone: 'good' };
    case 'ready': return { label: 'Safe to sleep', tone: 'good' };
    case 'pending': return { label: 'Waiting', tone: 'neutral' };
    case 'checking': case 'running': case 'closing':
      return { label: 'Working', tone: 'info' };
    case 'retained': case 'skipped': case 'not_eligible': case 'not-eligible':
      return { label: 'Kept open', tone: 'warn' };
    case 'blocked': return { label: 'Needs review', tone: 'warn' };
    case 'interrupted': return { label: 'Interrupted', tone: 'warn' };
    case 'failed': case 'unverified':
      return { label: 'Not verified', tone: 'bad' };
    default: return { label: state.replaceAll('_', ' '), tone: 'neutral' };
  }
}

export function captureStateLabel(state: string): { label: string; tone: Tone } {
  switch (state) {
    case 'captured': return { label: 'Captured', tone: 'good' };
    case 'blocked': return { label: 'Needs review', tone: 'warn' };
    case 'failed': return { label: 'Not captured', tone: 'bad' };
    default: return { label: state.replaceAll('_', ' '), tone: 'neutral' };
  }
}

export function snapshotStateLabel(state: string): { label: string; tone: Tone } {
  switch (state) {
    case 'verified': return { label: 'Restore verified', tone: 'good' };
    case 'captured': return { label: 'Captured', tone: 'good' };
    case 'blocked': return { label: 'Needs review', tone: 'warn' };
    case 'closed': return { label: 'Asleep', tone: 'info' };
    case 'failed': return { label: 'Failed', tone: 'bad' };
    default: return { label: state.replaceAll('_', ' '), tone: 'neutral' };
  }
}

/** Friendly application names; the engine identifiers stay untouched. */
export function appLabel(adapter: string) {
  switch (adapter) {
    case 'chrome': return 'Chrome';
    case 'vscode': return 'VS Code';
    case 'windows-generic': return 'Windows app context';
    case 'context': return 'Folders and terminals';
    default: return adapter;
  }
}
