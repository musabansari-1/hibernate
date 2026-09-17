import {expect,it} from 'vitest';
import {matchedCount,sameEditorGroups} from '../packages/core/verification';
it('does not count the same observed terminal or editor twice',()=>{expect(matchedCount(['cwd-a','cwd-a'],['cwd-a'],s=>s)).toBe(1);expect(matchedCount(['cwd-a','cwd-a'],['cwd-a','cwd-a'],s=>s)).toBe(2);});
it('unknown context never verifies an expected context',()=>{expect(matchedCount([null,null],[null,null],s=>s)).toBe(0);});
it('accepts unchanged editor state after native JSON key reordering',()=>{
 const expected=[{viewColumn:1,active:true,tabs:[{label:'saved.txt',active:true,pinned:true,dirty:false,uri:'file:///C:/project/saved.txt',kind:'text'}]}];
 const actual=[{tabs:[{uri:'file:///C:/project/saved.txt',pinned:true,label:'saved.txt',kind:'text',dirty:false,active:true}],active:true,viewColumn:1}];
 expect(sameEditorGroups(expected,actual)).toBe(true);
});
it('refuses dirty or reordered editor state even when the same files are open',()=>{
 const a={label:'a',active:true,pinned:true,dirty:false,uri:'file:///C:/a',kind:'text'},b={...a,label:'b',active:false,uri:'file:///C:/b'};
 const expected=[{viewColumn:1,active:true,tabs:[a,b]}];
 expect(sameEditorGroups(expected,[{viewColumn:1,active:true,tabs:[b,a]}])).toBe(false);
 expect(sameEditorGroups(expected,[{viewColumn:1,active:true,tabs:[{...a,dirty:true},b]}])).toBe(false);
});
