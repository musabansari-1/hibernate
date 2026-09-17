// Use the already pinned TypeScript parser/printer for consistent, dependency-free source formatting.
import ts from 'typescript';
import {readFile,writeFile,readdir} from 'node:fs/promises';
import {join} from 'node:path';
async function format(dir){for(const entry of await readdir(dir,{withFileTypes:true})){const path=join(dir,entry.name);if(entry.isDirectory()){if(!['dist','src-tauri','node_modules'].includes(entry.name))await format(path);}else if(/\.tsx?$/.test(entry.name)){const source=await readFile(path,'utf8');const file=ts.createSourceFile(path,source,ts.ScriptTarget.Latest,true,entry.name.endsWith('.tsx')?ts.ScriptKind.TSX:ts.ScriptKind.TS);await writeFile(path,ts.createPrinter({newLine:ts.NewLineKind.LineFeed}).printFile(file));}}}
for(const root of ['packages','extensions','apps/desktop/src','tests'])await format(root);
