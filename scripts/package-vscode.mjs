import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
const child=spawn(process.execPath,[resolve('node_modules/@vscode/vsce/vsce'),'package','--no-dependencies','--allow-missing-repository','--skip-license'],{cwd:resolve('extensions/vscode'),stdio:'inherit'});
child.on('exit',code=>process.exit(code??1));
