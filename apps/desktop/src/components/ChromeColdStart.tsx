import {useState} from 'react';
import {Icon} from './ui';

/**
 * ContextSleep never reads Chrome's profile folders or command line. The user
 * names the enrolled profile once so the exact same profile can be relaunched.
 */
export function ChromeColdStart({binding, busy, save}: {binding?: {profileDirectory: string; executable: string; userDataDir?: string | null}; busy: boolean; save(profileDirectory: string, executable: string, userDataDir: string): Promise<void>}) {
  const [profileDirectory, setProfileDirectory] = useState(binding?.profileDirectory ?? '');
  const [executable, setExecutable] = useState(binding?.executable ?? '');
  const [userDataDir, setUserDataDir] = useState(binding?.userDataDir ?? '');
  return <details className="panel-details">
    <summary>
      <span>Chrome profile binding</span>
      <small>{binding ? `Saved: ${binding.profileDirectory}` : 'Needed to reopen this exact profile'}</small>
    </summary>
    <div className="panel-details-body">
      <p className="muted-line">ContextSleep does not inspect Chrome's private profile folders or command line. Name the profile once so this exact enrolled profile can be relaunched after a restart.</p>
      <div className="field-grid">
        <label>Chrome profile folder<input value={profileDirectory} onChange={e => setProfileDirectory(e.target.value)} placeholder="Default or Profile 1" /></label>
        <label>chrome.exe path <em>optional</em><input value={executable} onChange={e => setExecutable(e.target.value)} placeholder="C:\Program Files\Google\Chrome\Application\chrome.exe" /></label>
        <label>User-data root <em>portable Chrome only</em><input value={userDataDir} onChange={e => setUserDataDir(e.target.value)} placeholder="D:\ChromeData" /></label>
      </div>
      <button type="button" className="btn btn-quiet" disabled={busy || !profileDirectory.trim()} onClick={() => void save(profileDirectory, executable, userDataDir)}><Icon name="check" />Save profile binding</button>
      {binding && <small className="muted-line">Saved for cold start: {binding.profileDirectory} - {binding.executable}{binding.userDataDir ? ` - data ${binding.userDataDir}` : ''}</small>}
    </div>
  </details>;
}
