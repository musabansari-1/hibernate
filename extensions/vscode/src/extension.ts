import * as vscode from 'vscode';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { isAbsolute, basename, join } from 'node:path';
import { existsSync } from 'node:fs';
import { randomUUID,createHash } from 'node:crypto';
import type { Job } from '../../../packages/core/model';
import {matchedCount,sameEditorGroups} from '../../../packages/core/verification';
let child: ChildProcessWithoutNullStreams | undefined;
let busy = false;
const terminalEvidence=new WeakMap<vscode.Terminal,{cwd:string;observedAt:number}>();
const pending = new Map<string, {
    resolve: (v: any) => void;
    reject: (e: Error) => void;
    timer: ReturnType<typeof setTimeout>;
}>();
function rpc(client: string, op: string, body: Record<string, unknown> = {}): Promise<any> {
    if (!child)
        return Promise.reject(new Error('Bridge disconnected'));
    const requestId = randomUUID();
    const data = Buffer.from(JSON.stringify({ requestId, client, op, ...body }));
    if (data.length > 900000)
        return Promise.reject(new Error('Snapshot exceeds native message limit'));
    const header = Buffer.alloc(4);
    header.writeUInt32LE(data.length);
    return new Promise((resolve, reject) => { const timer = setTimeout(() => { pending.delete(requestId); reject(new Error('Bridge timed out')); }, 15000); pending.set(requestId, { resolve, reject, timer }); child!.stdin.write(Buffer.concat([header, data])); });
}
function capture() {
    const groups = vscode.window.tabGroups.all.map(g => ({ viewColumn: g.viewColumn, active: g.isActive, tabs: g.tabs.map(t => ({ label: t.label, active: t.isActive, pinned: t.isPinned, dirty: t.isDirty, uri: t.input instanceof vscode.TabInputText ? t.input.uri.toString() : null, kind: t.input instanceof vscode.TabInputText ? 'text' : 'unsupported' })) }));
    const terminals = vscode.window.terminals.map(t => ({ name: t.name, cwd: t.shellIntegration?.cwd?.toString() ?? terminalEvidence.get(t)?.cwd ?? null, cwdSource:t.shellIntegration?.cwd?'VS Code shell integration':terminalEvidence.has(t)?'ContextSleep instrumented prompt':'unknown',observedAt:terminalEvidence.get(t)?.observedAt??null, shell: typeof t.creationOptions === 'object' && 'shellPath' in t.creationOptions ? t.creationOptions.shellPath ?? null : null }));
    const editors=vscode.window.visibleTextEditors.map(e=>({uri:e.document.uri.toString(),viewColumn:e.viewColumn,selections:e.selections.map(s=>({anchor:{line:s.anchor.line,character:s.anchor.character},active:{line:s.active.line,character:s.active.character}})),visibleRanges:e.visibleRanges.map(r=>({start:r.start.line,end:r.end.line}))}));
    return { version: 1, adapter: 'vscode', instance:vscode.env.sessionId, capturedAt: Date.now(), workspace: vscode.workspace.workspaceFile?.toString() ?? null, folders: vscode.workspace.workspaceFolders?.map(f => f.uri.toString()) ?? [], groups, editors, terminals, dirty: vscode.workspace.textDocuments.some(d => d.isDirty) || groups.some(g => g.tabs.some(t => t.dirty)), debugging: !!vscode.debug.activeDebugSession, tasks:vscode.tasks.taskExecutions.length, remote: !!vscode.env.remoteName, activeEditor: vscode.window.activeTextEditor?.document.uri.toString() ?? null, limitations: ['No document contents, terminal history or environment variables are captured. Unsaved buffers remain VS Code-owned.', 'Terminal shell integration may be unavailable; unknown cwd is preserved as unknown.'] };
}
async function revalidateClose(saved:ReturnType<typeof capture>){
    if(!saved||saved.version!==1||saved.adapter!=='vscode')throw new Error('Unsupported VS Code snapshot');
    const fresh=capture();
    if(saved.instance!==fresh.instance)throw new Error('This is a different VS Code window session. Capture and review its current state before closing.');
    const reason=fresh.dirty?'Unsaved editors detected.':fresh.terminals.length?'Close or save terminal work in VS Code first.':fresh.debugging||fresh.tasks?'A debugger or task is running.':fresh.groups.some(g=>g.tabs.some(t=>t.kind!=='text'||!t.uri?.startsWith('file:')))?'Custom, untitled or non-text editors cannot be reconstructed safely.':null;
    if(reason)throw new Error(reason+' Nothing was closed.');
    if(JSON.stringify(saved.folders)!==JSON.stringify(fresh.folders)||saved.workspace!==fresh.workspace||!sameEditorGroups(saved.groups,fresh.groups))throw new Error('VS Code context changed after review. Capture again.');
    for(const tab of fresh.groups.flatMap(g=>g.tabs)){const uri=vscode.Uri.parse(tab.uri!,true);if(uri.scheme!=='file'||uri.authority)throw new Error('Only saved local-drive text editors support reviewed close.');await vscode.workspace.fs.stat(uri);}
    const unchanged=()=>{const now=capture();return !now.dirty&&!now.terminals.length&&!now.debugging&&!now.tasks&&now.instance===fresh.instance&&now.workspace===fresh.workspace&&JSON.stringify(now.folders)===JSON.stringify(fresh.folders)&&sameEditorGroups(now.groups,fresh.groups);};
    if(!unchanged())throw new Error('VS Code changed during the safety check. Capture again; nothing was closed.');
    return unchanged;
}
export async function activate(context: vscode.ExtensionContext) {
    const workspaceIdentity=vscode.workspace.workspaceFile?.toString()??(vscode.workspace.workspaceFolders??[]).map(f=>f.uri.toString()).sort();
    // Keep enrolled legacy IDs. New identities do not depend on a last-moment VS Code storage flush.
    const client = context.workspaceState.get<string>('client') ?? 'vscode:'+createHash('sha256').update(JSON.stringify([context.globalStorageUri.toString(),workspaceIdentity])).digest('hex');
    await context.workspaceState.update('client', client);
    const output = vscode.window.createOutputChannel('ContextSleep');
    const reconstructedTerminals=new Map<string,(vscode.Terminal|undefined)[]>();
    const instrumented=new WeakMap<vscode.Terminal,string>();
    const sessionFor=(t:vscode.Terminal)=>instrumented.get(t)??('env' in t.creationOptions?t.creationOptions.env?.HIBERNATE_TERMINAL_SESSION:undefined);
    async function refreshTerminalEvidence(){
        const terminals=vscode.window.terminals.filter(t=>sessionFor(t));if(!terminals.length)return;
        const observed:{id:string;pid:number;cwd:string;observedAt:number}[]=await rpc(client,'terminalObservations');
        await Promise.all(terminals.map(async t=>{const pid=await t.processId;const actual=observed.find(o=>o.id===sessionFor(t)&&o.pid===pid);terminalEvidence.delete(t);if(actual&&typeof actual.cwd==='string')terminalEvidence.set(t,{cwd:vscode.Uri.file(actual.cwd).toString(),observedAt:actual.observedAt});}));
    }
    async function newTerminal(snapshotId:string,kind:'folder'|'terminal',index:number,name:string){
        const launch:{id:string;shellPath:string;shellArgs:string[];cwd:string}=await rpc(client,'prepareTerminal',{snapshotId,kind,index});
        const t=vscode.window.createTerminal({name,cwd:launch.cwd,shellPath:launch.shellPath,shellArgs:launch.shellArgs,env:{HIBERNATE_TERMINAL_SESSION:launch.id}});instrumented.set(t,launch.id);t.show(true);return t;
    }
    context.subscriptions.push(output);
    async function tick() {
        if (busy || !child)
            return;
        busy = true;
        try {
            await refreshTerminalEvidence();
            const state = capture();
            const signals = [...state.folders.map(path => ({ source: 'vscode', label: vscode.workspace.name ?? 'VS Code project', path })), ...state.terminals.filter(t => t.cwd).map(t => ({ source: 'terminal', label: t.name, path: t.cwd }))];
            const job: Job | null = await rpc(client, 'poll', { adapter: 'vscode', body: { label: 'VS Code', instance:state.instance, signals, items: [{ id: 'workspace', focused:vscode.window.state.focused, title: vscode.workspace.name ?? 'Untitled window', detail: `${state.groups.reduce((n, g) => n + g.tabs.length, 0)} editors, ${state.terminals.length} terminals${state.dirty ? ' - unsaved work' : ''}`, compatibility: 'Workspace and editors restore; clean close is reviewed' }] } });
            if (job) {
                let closeRequested=false;
                try {
                    // Native polling can overlap editor/terminal changes. Capture after receiving the job.
                    await refreshTerminalEvidence();
                    const state=capture();
                    if (job.action === 'capture' || job.action === 'hibernate') {
                        if (!state.workspace && !state.folders.length)
                            throw new Error('Open a local project folder or saved workspace before capture.');
                        await rpc(client, 'save', { body: state, jobId: job.id });
                        if (job.action === 'hibernate')
                            throw new Error(state.dirty ? 'Unsaved editors detected. Nothing was closed.' : state.terminals.length ? 'Terminal processes cannot be proven safe to stop. Nothing was closed.' : 'Capture saved. Automatic VS Code close remains blocked until close/restore verification is validated. Use VS Code\'s normal close action; native hot exit remains responsible for unsaved buffers.');
                        await rpc(client, 'complete', { id: job.id, ok: true, result: 'Workspace, editor metadata and available terminal cwd captured. No file contents captured.' });
                    }
                    else if(job.action==='preflight_close'){
                        await revalidateClose(job.snapshot?.body as ReturnType<typeof capture>);
                        await rpc(client,'complete',{id:job.id,ok:true,result:{checked:true,snapshotId:job.snapshot!.id,detail:'Clean saved local editor context checked. Window closure still requires verification.'}});
                    }
                    else if(job.action==='close'){
                        const snapshot=job.snapshot;const saved=snapshot?.body as ReturnType<typeof capture>;
                        if(!snapshot||snapshot.state!=='approved'||!saved||saved.version!==1)throw new Error('Review a fresh VS Code snapshot first.');
                        const unchanged=await revalidateClose(saved);
                        const observation:{observing:boolean;detail:string}=await rpc(client,'beginVscodeClose',{snapshotId:snapshot.id,jobId:job.id});
                        if(!observation.observing)throw new Error(`Native close verification is unavailable: ${observation.detail}`);
                        if(!unchanged()){await rpc(client,'finishSnapshot',{id:snapshot.id,state:'blocked',result:'VS Code changed before closure. Nothing was closed.'});throw new Error('VS Code changed before closure. Capture again.');}
                        closeRequested=true;await vscode.commands.executeCommand('workbench.action.closeWindow');
                        // The durable native observer owns completion, independently of
                        // this extension host and the child bridge that VS Code may end.
                    }
                    else if (job.action === 'resume') {
                        const saved = job.snapshot?.body as ReturnType<typeof capture>;
                        if (!saved || saved.version !== 1 || saved.adapter !== 'vscode')
                            throw new Error('Unsupported snapshot');
                        const actual = capture();
                        const same = JSON.stringify(saved.folders) === JSON.stringify(actual.folders) && saved.workspace === actual.workspace;
                        if (!same)
                            throw new Error('Open the original workspace with VS Code first. This integration will not replace the current workspace.');
                        const expected = saved.groups.flatMap(g => g.tabs.filter(t=>t.uri).map(t=>`${g.viewColumn}:${t.uri}`));
                        const present = new Set(actual.groups.flatMap(g => g.tabs.filter(t=>t.uri).map(t=>`${g.viewColumn}:${t.uri}`)));
                        // Open missing saved text editors only; never replace or close any existing editor.
                        for (const group of saved.groups)
                            for (const tab of group.tabs) {
                                if (tab.uri && !present.has(`${group.viewColumn}:${tab.uri}`)) {
                                    const uri = vscode.Uri.parse(tab.uri, true);
                                    if (uri.scheme !== 'file' || uri.authority)
                                        continue;
                                    await vscode.window.showTextDocument(uri, { viewColumn: group.viewColumn, preserveFocus: true, preview: false });
                                }
                            }
                        for(const entry of saved.editors??[]){
                            const editor=vscode.window.visibleTextEditors.find(e=>e.document.uri.toString()===entry.uri&&e.viewColumn===entry.viewColumn);
                            if(!editor||editor.document.isDirty)continue;
                            editor.selections=entry.selections.map(s=>new vscode.Selection(editor.document.validatePosition(new vscode.Position(s.anchor.line,s.anchor.character)),editor.document.validatePosition(new vscode.Position(s.active.line,s.active.character))));
                            const range=entry.visibleRanges[0];if(range)editor.revealRange(new vscode.Range(Math.min(range.start,editor.document.lineCount-1),0,Math.min(range.end,editor.document.lineCount-1),0),vscode.TextEditorRevealType.AtTop);
                        }
                        const created=reconstructedTerminals.get(job.snapshot!.id)??[];
                        const terminalProblems:string[]=[];
                        const usedTerminals=new Set<vscode.Terminal>();
                        for(const [index,terminal] of saved.terminals.entries()){
                            if(!terminal.cwd){terminalProblems.push(`${terminal.name}: working directory was not available at capture`);continue;}
                            const cwd=vscode.Uri.parse(terminal.cwd,true);if(cwd.scheme!=='file'||cwd.authority){terminalProblems.push(`${terminal.name}: only local terminal directories are supported`);continue;}
                            const prior=created[index];
                            const present=vscode.window.terminals.find(t=>!usedTerminals.has(t)&&t.name===terminal.name&&(t.shellIntegration?.cwd?.toString()??terminalEvidence.get(t)?.cwd)===terminal.cwd)??(prior&&vscode.window.terminals.includes(prior)&&!usedTerminals.has(prior)?prior:undefined);
                            if(present){usedTerminals.add(present);continue;}
                            try{if((await vscode.workspace.fs.stat(cwd)).type!==vscode.FileType.Directory)throw new Error('Not a directory');const t=await newTerminal(job.snapshot!.id,'terminal',index,terminal.name);created[index]=t;usedTerminals.add(t);}catch(e){terminalProblems.push(`${terminal.name}: ${String(e)}`);}
                        }
                        reconstructedTerminals.set(job.snapshot!.id,created);
                        const terminalDeadline=Date.now()+8000;
                        const knownTerminals=saved.terminals.filter(t=>t.cwd);
                        while(matchedCount(knownTerminals,capture().terminals,t=>t.cwd?`${t.name}:${t.cwd}`:null)<knownTerminals.length&&Date.now()<terminalDeadline){await new Promise(r=>setTimeout(r,300));await refreshTerminalEvidence();}
                        if (saved.activeEditor) {
                            const uri = vscode.Uri.parse(saved.activeEditor, true);
                            const column=saved.groups.find(g=>g.active&&g.tabs.some(t=>t.uri===saved.activeEditor))?.viewColumn;
                            if (uri.scheme === 'file'&&!uri.authority)
                                await vscode.window.showTextDocument(uri, { preview: false,viewColumn:column });
                        }
                        const returned = new Set(capture().groups.flatMap(g => g.tabs.map(t=>`${g.viewColumn}:${t.uri}`)));
                        const missing = expected.filter(uri => !returned.has(uri));
                        const remaining=capture();const expectedTerminals=saved.terminals.length;
                        const verifiedTerminals=matchedCount(saved.terminals,remaining.terminals,t=>t.cwd?`${t.name}:${t.cwd}`:null);
                        const result = { verified: same && missing.length === 0 && verifiedTerminals===expectedTerminals, facts:[{capability:'VS Code workspace',expected:1,observed:same?1:0,status:same?'verified':'missing'},{capability:'Saved text editors',expected:expected.length,observed:expected.length-missing.length,status:missing.length?'missing':'verified'},{capability:'Terminal working directories',expected:expectedTerminals,observed:verifiedTerminals,status:verifiedTerminals===expectedTerminals?'verified':'missing'}], details: [`Workspace identity matches; ${expected.length - missing.length}/${expected.length} saved text editors present.`, `${verifiedTerminals}/${expectedTerminals} terminal names and working directories observed through shell integration or the process-bound prompt handshake.`, 'New terminal contexts use instrumented Windows PowerShell without profile scripts. Previous running commands, exact terminal state, non-text editors and exact layout are not restored.'], missing,terminalProblems };
                        await rpc(client, 'finishSnapshot', { id: job.snapshot!.id, state: 'incomplete', result });
                        await rpc(client, 'complete', { id: job.id, ok: true, result });
                    }
                    else
                        throw new Error('Unsupported VS Code operation');
                }
                catch (e) {
                    if(job.action==='close'&&job.snapshot)await rpc(client,'finishSnapshot',{id:job.snapshot.id,state:closeRequested?'partial':'blocked',result:String(e)}).catch(()=>{});
                    await rpc(client, 'complete', { id: job.id, ok: false, result: String(e) });
                }
            }
        }
        catch (e) {
            output.appendLine(String(e));
        }
        finally {
            busy = false;
        }
    }
    function disconnect() { child = undefined; for (const p of pending.values()) {
        clearTimeout(p.timer);
        p.reject(new Error('Native host exited'));
    } pending.clear(); }
    async function connect() {
        if (child)
            return;
        if (!vscode.workspace.isTrusted || vscode.env.remoteName)
            throw new Error('Only trusted, local workspaces are supported.');
        const configured = vscode.workspace.getConfiguration('hibernate').get<string>('nativeHostPath') ?? '';
        const bundled = process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, 'ContextSleep', 'integrations', 'hibernate-host.exe') : '';
        const path = isAbsolute(configured) && basename(configured).toLowerCase() === 'hibernate-host.exe' ? configured : bundled;
        if (!path || !isAbsolute(path) || basename(path).toLowerCase() !== 'hibernate-host.exe' || !existsSync(path))
            throw new Error('ContextSleep native bridge is not installed. Use Connect apps in the ContextSleep desktop app, or set hibernate.nativeHostPath manually.');
        child = spawn(path, ['--vscode'], { windowsHide: true, stdio: 'pipe' });
        let input: Buffer = Buffer.alloc(0);
        child.stdout.on('data', (chunk: Buffer) => {
            input = Buffer.concat([input, chunk]);
            while (input.length >= 4) {
                const size = input.readUInt32LE();
                if (size > 900000 || size === 0) {
                    child?.kill();
                    return;
                }
                if (input.length < size + 4)
                    return;
                try {
                    const message = JSON.parse(input.subarray(4, size + 4).toString());
                    const p = pending.get(message.requestId);
                    if (p) {
                        clearTimeout(p.timer);
                        pending.delete(message.requestId);
                        message.ok ? p.resolve(message.value) : p.reject(new Error(message.error));
                    }
                }
                catch (e) {
                    output.appendLine('Invalid native response');
                    child?.kill();
                    return;
                }
                input = input.subarray(size + 4);
            }
        });
        child.stderr.on('data', () => output.appendLine('Native bridge reported an error. Check installation.'));
        child.on('error', e => { output.appendLine(e.message); disconnect(); });
        child.on('exit', disconnect);
        await tick();
    }
    context.subscriptions.push(vscode.commands.registerCommand('hibernate.connect', () => connect().catch(e => vscode.window.showErrorMessage(String(e)))));
    context.subscriptions.push(vscode.commands.registerCommand('hibernate.openProjectTerminal',async()=>{
        await connect();await refreshTerminalEvidence();const state=capture();if(!state.folders.length)throw new Error('Open a local project folder first.');
        const snapshot=await rpc(client,'save',{body:state});return newTerminal(snapshot.id,'folder',0,'Project context');
    }));
    const timer = setInterval(() => void tick(), 2000);
    context.subscriptions.push({ dispose() { clearInterval(timer); child?.stdin.end(); } });
    const configuredHost = vscode.workspace.getConfiguration('hibernate').get<string>('nativeHostPath') ?? '';
    const bundledHost = process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, 'ContextSleep', 'integrations', 'hibernate-host.exe') : '';
    if ((configuredHost && existsSync(configuredHost)) || (bundledHost && existsSync(bundledHost)))
        await connect().catch(e => output.appendLine(String(e)));
}

