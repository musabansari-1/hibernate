import { downloadAndUnzipVSCode } from '@vscode/test-electron';
import { resolve } from 'node:path';
import { writeFile } from 'node:fs/promises';
const executable=await downloadAndUnzipVSCode({version:'stable',platform:'win32-x64-archive',cachePath:resolve('.validation/vscode-download')});
await writeFile('.validation/vscode-executable.txt',executable);
console.log(executable);
