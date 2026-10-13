import type {Project} from './workspace';
export interface UniverseSeries {id:string;title:string}
export interface UniverseCatalogue {schemaVersion:1;series:UniverseSeries[];novels:{projectId:string;seriesId:string|null}[]}
export interface UniverseEntry {project:Project;catalogue:UniverseCatalogue;revisionId:string}
export type LibraryRpc=(operation:string,args:Record<string,unknown>)=>Promise<unknown>;
export function validateUniverseCatalogue(value:unknown):asserts value is UniverseCatalogue {
 const object=value as UniverseCatalogue;
 if(!object||object.schemaVersion!==1||!Array.isArray(object.series)||!Array.isArray(object.novels)||object.series.length>1000||object.novels.length>20000)throw new Error('Invalid universe catalogue. Saved data was kept.');
 const ids=new Set<string>(),novels=new Set<string>();
 for(const series of object.series){if(!series||typeof series.id!=='string'||!series.id||ids.has(series.id)||typeof series.title!=='string'||!series.title.trim()||series.title.length>500)throw new Error('Invalid universe series. Saved data was kept.');ids.add(series.id);}
 for(const novel of object.novels){if(!novel||typeof novel.projectId!=='string'||!novel.projectId||novels.has(novel.projectId)||(novel.seriesId!==null&&!ids.has(novel.seriesId)))throw new Error('Invalid universe novel. Saved data was kept.');novels.add(novel.projectId);}
}
export async function readUniverses(rpc:LibraryRpc,projects:readonly Project[]):Promise<UniverseEntry[]> {
 const entries=await Promise.all(projects.map(async project=>{
  const data=await rpc('load_project_data',{projectId:project.id,kind:'universeCatalogue'});
  if(data===null)return null;if(!data||typeof data!=='object'||!('payload' in data)||!('revisionId' in data)||typeof data.revisionId!=='string')throw new Error('Invalid universe revision. Saved data was kept.');validateUniverseCatalogue(data.payload);return {project,catalogue:data.payload,revisionId:data.revisionId};
 }));
 const result=entries.filter((entry):entry is UniverseEntry=>entry!==null),owners=new Map<string,string>(),roots=new Set(result.map(entry=>entry.project.id));
 for(const entry of result)for(const novel of entry.catalogue.novels){if(roots.has(novel.projectId)||owners.has(novel.projectId))throw new Error('A novel has conflicting universe membership. Resolve the saved catalogue before editing it.');owners.set(novel.projectId,entry.project.id);}
 return result;
}
export function novelUniverse(entries:readonly UniverseEntry[],projectId:string):{universe:UniverseEntry;series:UniverseSeries|null}|null {
 for(const universe of entries){const novel=universe.catalogue.novels.find(item=>item.projectId===projectId);if(novel)return {universe,series:universe.catalogue.series.find(series=>series.id===novel.seriesId)??null};}return null;
}
export function unassignedNovels(projects:readonly Project[],entries:readonly UniverseEntry[]):Project[] {
 const used=new Set(entries.flatMap(entry=>[entry.project.id,...entry.catalogue.novels.map(novel=>novel.projectId)]));return projects.filter(project=>!used.has(project.id));
}
export async function addUniverseSeries(rpc:LibraryRpc,entry:UniverseEntry,title:string):Promise<string> {
 if(!title.trim()||title.length>500)throw new Error('Enter a series name.');
 const series={id:crypto.randomUUID(),title:title.trim()},payload={...entry.catalogue,series:[...entry.catalogue.series,series]};
 await rpc('save_project_data',{projectId:entry.project.id,kind:'universeCatalogue',payload,parent:entry.revisionId});return series.id;
}
export async function attachUniverseNovel(rpc:LibraryRpc,entry:UniverseEntry,seriesId:string,projectId:string):Promise<void> {
 if(!entry.catalogue.series.some(series=>series.id===seriesId))throw new Error('Choose a saved series.');
 const payload={...entry.catalogue,novels:[...entry.catalogue.novels,{projectId,seriesId}]};
 await rpc('save_project_data',{projectId:entry.project.id,kind:'universeCatalogue',payload,parent:entry.revisionId});
}
