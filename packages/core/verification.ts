/** Consume observations once: duplicate names or editor URIs must not inflate completeness. */
export function matchedCount<T>(expected:readonly T[],observed:readonly T[],key:(value:T)=>string|null):number{
    const available=new Map<string,number>();for(const item of observed){const k=key(item);if(k!==null)available.set(k,(available.get(k)??0)+1);}
    let count=0;for(const item of expected){const k=key(item);if(k!==null&&(available.get(k)??0)>0){available.set(k,available.get(k)!-1);count++;}}return count;
}
type EditorGroupState={viewColumn:number;active:boolean;tabs:{label:string;active:boolean;pinned:boolean;dirty:boolean;uri:string|null;kind:string}[]};
/** Native JSON serialization may reorder object keys. Compare semantic fields in a fixed order. */
export function sameEditorGroups(expected:readonly EditorGroupState[],actual:readonly EditorGroupState[]):boolean{
    const shape=(groups:readonly EditorGroupState[])=>groups.map(g=>[g.viewColumn,g.active,g.tabs.map(t=>[t.kind,t.uri,t.label,t.active,t.pinned,t.dirty])]);
    return JSON.stringify(shape(expected))===JSON.stringify(shape(actual));
}
