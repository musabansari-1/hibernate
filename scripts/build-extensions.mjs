import { build } from 'esbuild';
import { mkdir, cp } from 'node:fs/promises';
await mkdir('extensions/chrome/dist', { recursive: true });
await build({ entryPoints: ['extensions/chrome/src/background.ts', 'extensions/chrome/src/popup.ts', 'extensions/chrome/src/placeholder.ts', 'extensions/chrome/src/fixture.ts', 'extensions/chrome/src/bootstrap.ts'], bundle: true, outdir: 'extensions/chrome/dist', format: 'esm', target: 'chrome120' });
await cp('extensions/chrome/public', 'extensions/chrome/dist', { recursive: true });
await build({ entryPoints: ['extensions/vscode/src/extension.ts'], outfile: 'extensions/vscode/dist/extension.cjs', bundle: true, platform: 'node', format: 'cjs', external: ['vscode'], target: 'node20' });
