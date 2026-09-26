import {describe,it,expect} from 'vitest';
import {withNativeDiscovery} from '../packages/core/app-discovery';
import {pruneSelectedTargets} from '../packages/core/projects';
import type {Dashboard,NativeApp} from '../packages/core/model';

const app = (id:string,group:string|null,known=true):NativeApp => ({id,label:'Fixture.exe',group,enhancement:null,
  window:{title:'Fixture',handle:id,class:'Fixture',owner:known?{pid:10,processStarted:'20',executable:'D:\\Fixture.exe'}:null,sameOwnerWindows:3,appUserModelId:null},
  capabilities:{contextCapture:known,reviewedClose:false,semanticRestore:false,limitation:'Metadata only; retained during Sleep.'}});
const dashboard = (items:NativeApp[],at=100):Dashboard => ({reports:[],apps:{at,error:null,items}} as unknown as Dashboard);
describe('native app discovery selections',()=>{
  it('keeps shared-process windows separate and exposes limitations',()=>{
    const d=withNativeDiscovery(dashboard([app('one','10:20'),app('two','10:20')]));
    expect(d.reports[0].body.items.map(i=>i.id)).toEqual(['one','two']);
    expect(d.reports[0].body.items[0].nativeGroup).toBe('10:20');
    expect(d.reports[0].body.items[0].compatibility).toContain('retained');
  });
  it('does not offer capture for unavailable identity',()=>{
    expect(withNativeDiscovery(dashboard([app('one',null,false)])).reports[0].body.items[0].capturable).toBe(false);
  });
  it('revokes replaced native selections while retaining unchanged semantic reports',()=>{
    const old=withNativeDiscovery(dashboard([app('one','10:20')]));
    const fresh=withNativeDiscovery(dashboard([app('two','10:21')],101));
    expect(pruneSelectedTargets(['["windows-discovery","one"]'],old.reports,fresh.reports,102)).toEqual([]);
    expect(withNativeDiscovery({...old,apps:undefined}).reports).toEqual([]);
  });
});
