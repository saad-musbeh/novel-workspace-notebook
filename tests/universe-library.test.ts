import {describe,it,expect,vi} from 'vitest';
import {readUniverses,novelUniverse,unassignedNovels,addUniverseSeries,attachUniverseNovel,validateUniverseCatalogue,type UniverseEntry} from '../apps/writer/src/universe-library';
const projects=['world','book-a','book-b'].map(id=>({id,title:id+' يس 中文',createdAt:'fixture'}));
const entry:UniverseEntry={project:projects[0],revisionId:'catalogue-head',catalogue:{schemaVersion:1,series:[{id:'series-a',title:'First series'},{id:'series-b',title:'第二系列'}],novels:[{projectId:'book-a',seriesId:'series-a'}]}};
describe('universe library',()=>{
 it('discovers shared roots without reading manuscript bodies, retaining unresolved novel references',async()=>{
  const rpc=vi.fn(async(operation,args)=>{expect(operation).toBe('load_project_data');expect(args.kind).toBe('universeCatalogue');return args.projectId==='world'?{payload:{...entry.catalogue,novels:[...entry.catalogue.novels,{projectId:'offline-book',seriesId:'series-b'}]},revisionId:entry.revisionId}:null;});
  const universes=await readUniverses(rpc,projects);expect(rpc).toHaveBeenCalledTimes(3);expect(universes[0].catalogue.novels.at(-1)?.projectId).toBe('offline-book');expect(novelUniverse(universes,'book-a')).toEqual({universe:universes[0],series:entry.catalogue.series[0]});expect(unassignedNovels(projects,universes)).toEqual([projects[2]]);
 });
 it('rejects duplicate memberships and universe roots used as novels rather than silently choosing one',async()=>{
  const second={schemaVersion:1,series:[],novels:[{projectId:'book-a',seriesId:null}]};
  await expect(readUniverses(vi.fn(async(_op,args)=>args.projectId==='world'?{payload:entry.catalogue,revisionId:'one'}:args.projectId==='book-b'?{payload:second,revisionId:'two'}:null),projects)).rejects.toThrow('conflicting universe membership');
  expect(()=>validateUniverseCatalogue({...entry.catalogue,novels:[{projectId:'book-a',seriesId:'missing'}]})).toThrow('Invalid universe novel');
  expect(()=>validateUniverseCatalogue({...entry.catalogue,series:[...entry.catalogue.series,entry.catalogue.series[0]]})).toThrow('Invalid universe series');
 });
 it('uses the captured catalogue head for series and existing-novel writes, propagating CAS refusal without mutation',async()=>{
  const before=JSON.stringify(entry),rpc=vi.fn(async()=>{throw new Error('Conflict: saved head changed');});
  await expect(addUniverseSeries(rpc,entry,'سلسلة 中文')).rejects.toThrow('Conflict');expect(rpc).toHaveBeenCalledWith('save_project_data',expect.objectContaining({projectId:'world',kind:'universeCatalogue',parent:'catalogue-head',payload:expect.objectContaining({series:expect.arrayContaining([expect.objectContaining({title:'سلسلة 中文'})])})}));
  rpc.mockClear();await expect(attachUniverseNovel(rpc,entry,'series-b','book-b')).rejects.toThrow('Conflict');expect(rpc).toHaveBeenCalledWith('save_project_data',{projectId:'world',kind:'universeCatalogue',parent:'catalogue-head',payload:{...entry.catalogue,novels:[...entry.catalogue.novels,{projectId:'book-b',seriesId:'series-b'}]}});expect(JSON.stringify(entry)).toBe(before);
 });
});
