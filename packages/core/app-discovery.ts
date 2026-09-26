import type {Dashboard, Report} from './model';

/** Native observations enhance the chooser, never fabricate an integration/HWND binding. */
export function withNativeDiscovery(data: Dashboard): Dashboard {
    const reports: Report[] = data.reports.filter(r => r.adapter !== 'windows-generic');
    const apps = data.apps;
    if (apps?.items.length) reports.push({
        client:'windows-discovery', adapter:'windows-generic', at:apps.at,
        body:{label:'Open Windows apps — context only',items:apps.items.map(app => ({
            id:app.id, title:`App context: ${app.window.title}`, membershipKey:app.id,
            detail:`${app.label}${app.window.sameOwnerWindows > 1 ? ` · ${app.window.sameOwnerWindows} top-level windows share this process (including hidden windows)` : ''}`,
            compatibility:`${app.capabilities.reconstruction?.summary ?? app.capabilities.limitation}${app.enhancement ? ' Use its specialized entry for deeper capture and restore.' : ''}`,
            capturable:app.capabilities.contextCapture,nativeGroup:app.group,
        }))},
    });
    return {...data,reports};
}
