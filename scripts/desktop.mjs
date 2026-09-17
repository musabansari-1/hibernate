import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
// Tauri locates src-tauri from cwd; the monorepo root holds the npm scripts and dependencies.
const child=spawn(process.execPath,[resolve('node_modules/@tauri-apps/cli/tauri.js'),...process.argv.slice(2)],{cwd:resolve('apps/desktop'),stdio:'inherit'});
child.on('exit',code=>process.exit(code??1));
