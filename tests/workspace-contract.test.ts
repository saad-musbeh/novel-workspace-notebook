import {spawnSync} from 'node:child_process';
import {copyFileSync,mkdtempSync,mkdirSync,readFileSync,readdirSync,rmSync,writeFileSync} from 'node:fs';
import {homedir,tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {afterEach,describe,expect,expectTypeOf,it,vi} from 'vitest';
import {storageRpcCoverage,workspaceWireSchema} from '../apps/writer/src/generated/workspace-contract';
import type {StorageRpcContract,Snapshot,ProjectIndex,SceneRecord,SceneArrangementSnapshot,MetadataResolutionSnapshot,UniverseNovelCreated,NovelCreated,Project} from '../apps/writer/src/generated/workspace-contract';
import type {Scene} from '../apps/writer/src/document';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke}));
import {workspaceCommand} from '../apps/writer/src/workspace';

const root=resolve('.');
const script=join(root,'tools/generate-workspace-contract.py');
const output='apps/writer/src/generated/workspace-contract.ts';
const source='crates/storage/src/workspace.rs';
const temporary:string[]=[];
function temp(){const path=mkdtempSync(join(tmpdir(),'novel-contract-'));temporary.push(path);return path;}
function run(args:string[]=[]){return spawnSync('python3',[script,...args],{encoding:'utf8',timeout:10000});}
function fixtureRoot(){const path=temp();for(const file of ['crates/storage/src/lib.rs',source,'crates/storage/src/scene_arrangement.rs','crates/storage/src/sync.rs','crates/storage/src/export_profile.rs','crates/storage/src/time_machine.rs','crates/storage/src/branches.rs','crates/storage/src/universe.rs','crates/companion/src/main.rs',output]){mkdirSync(join(path,file,'..'),{recursive:true});copyFileSync(join(root,file),join(path,file));}return path;}
afterEach(()=>{for(const path of temporary.splice(0))rmSync(path,{recursive:true,force:true});vi.unstubAllGlobals();invoke.mockReset();});

describe('generated storage wire contract',()=>{
 it('is deterministic and matches the committed Rust declarations',()=>{
  expect(run(['--check']).status).toBe(0);
  const path=fixtureRoot();expect(run(['--root',path]).status).toBe(0);
  expect(readFileSync(join(path,output),'utf8')).toBe(readFileSync(join(root,output),'utf8'));
  expect(workspaceWireSchema.Revision.fields.map(field=>field.name)).toEqual(['id','projectId','entityId','entityType','parents','payload','operation','schemaVersion','checksum','createdAt','sequence','originEpoch','originDevice']);
 });
 it('fails drift without overwriting the file, then regenerates changed fields',()=>{
  const path=fixtureRoot();const before=readFileSync(join(path,output),'utf8');
  const rust=readFileSync(join(path,source),'utf8').replace('pub struct Project {','pub struct Project {\n    pub subtitle: String,');writeFileSync(join(path,source),rust);
  const drift=run(['--root',path,'--check']);expect(drift.status).toBe(1);expect(drift.stderr).toContain('contract drift');expect(readFileSync(join(path,output),'utf8')).toBe(before);
  expect(run(['--root',path]).status).toBe(0);expect(run(['--root',path,'--check']).status).toBe(0);expect(readFileSync(join(path,output),'utf8')).toContain('subtitle: string;');
 });
 it.each(['pub unsupported: &SceneArrangementSnapshot,','pub unsupported: Box<String>,','#[serde(skip)] pub unsupported: String,','pub unsupported: std::collections::HashMap<String,String>,'])('refuses unsupported fields: %s',(field)=>{
  const path=fixtureRoot();const before=readFileSync(join(path,output),'utf8');writeFileSync(join(path,source),readFileSync(join(path,source),'utf8').replace('pub struct Project {',`pub struct Project {\n${field}`));
  const result=run(['--root',path]);expect(result.status).toBe(2);expect(result.stderr).toContain('refused');expect(readFileSync(join(path,output),'utf8')).toBe(before);
 });
 it('refuses serde behaviors and generic structs rather than guessing',()=>{
  for(const replacement of ['#[serde(default)]\npub struct Project {','pub struct Project<T> {']){
   const path=fixtureRoot();writeFileSync(join(path,source),readFileSync(join(path,source),'utf8').replace('pub struct Project {',replacement));expect(run(['--root',path]).status).toBe(2);
  }
 });
 it('keeps typed document bindings and JSON null responses compatible',()=>{
  expectTypeOf<StorageRpcContract<Scene>['save']['args']>().toEqualTypeOf<{scene:Scene;parent:string|null}>();
  expectTypeOf<StorageRpcContract<Scene>['load_scene']['result']>().toEqualTypeOf<Snapshot<Scene>>();
  expectTypeOf<StorageRpcContract['create_universe']['result']>().toEqualTypeOf<Project>();
  expectTypeOf<StorageRpcContract<Scene>['create_universe_novel']['args']>().toEqualTypeOf<{universeId:string;seriesId:string|null;title:string;parent:string}>();
  expectTypeOf<StorageRpcContract<Scene>['create_universe_novel']['result']>().toEqualTypeOf<UniverseNovelCreated<Scene>>();
  expectTypeOf<StorageRpcContract['create_novel']['args']>().toEqualTypeOf<{title:string}>();
  expectTypeOf<StorageRpcContract<Scene>['create_novel']['result']>().toEqualTypeOf<NovelCreated<Scene>>();
  expectTypeOf<StorageRpcContract['rename_project']['result']>().toEqualTypeOf<null>();
  expectTypeOf<StorageRpcContract['open_project_index']['result']>().toEqualTypeOf<ProjectIndex>();
  expectTypeOf<StorageRpcContract['edit_chapter']['args']>().toEqualTypeOf<{chapterId:string;expectedChapterHead:string|null;expectedProjectHead:string|null;expectedOrder:string[];numberingParent:string|null;title:string;number:number}>();
  expectTypeOf<StorageRpcContract['arrange_scene']['args']>().toEqualTypeOf<{snapshot:SceneArrangementSnapshot;targetChapterId:string;position:number}>();
  expectTypeOf<StorageRpcContract['metadata_resolution_snapshot']['result']>().toEqualTypeOf<MetadataResolutionSnapshot>();
  expectTypeOf<StorageRpcContract['resolve_metadata_checked']['args']>().toEqualTypeOf<{snapshot:MetadataResolutionSnapshot;payload:unknown}>();
  expectTypeOf<StorageRpcContract['edit_chapter']['result']>().toEqualTypeOf<ProjectIndex>();
  expectTypeOf<StorageRpcContract<Scene>['load_project_scene']['result']>().toEqualTypeOf<SceneRecord<Scene>>();
 });
 it('derives direct RPC keys/types from dispatch and storage signatures, including JSON unit null',()=>{
  const generated=readFileSync(join(root,output),'utf8');
  expect(generated).toContain('rename_project: {args: {projectId: string; title: string;}; result: null}');
  expect(generated).toContain('save: {args: {scene: TScene; parent: (string | null);}; result: Snapshot<TScene>}');
  expect(storageRpcCoverage.covered).toEqual(expect.arrayContaining(['load_scene','open_project_index','load_project_scene','create_universe','create_universe_novel']));
  expect(workspaceWireSchema.SceneSummary.fields.map(field=>field.name)).toEqual(['id','chapterId','position','revisionId','title']);
  expect(storageRpcCoverage.uncovered).toEqual(expect.arrayContaining(['save_scene','provider_run','peer_pair','sync_start']));
  expect(new Set([...storageRpcCoverage.covered,...storageRpcCoverage.uncovered]).size).toBe(storageRpcCoverage.covered.length+storageRpcCoverage.uncovered.length);
  const path=fixtureRoot();const dispatch=join(path,'crates/companion/src/main.rs');
  writeFileSync(dispatch,readFileSync(dispatch,'utf8').replace('"load_scene" => encode(storage.load_scene(&argument::<String>(&args,"sceneId")?).await?),','"load_scene" => encode(storage.load_scene(&argument::<String>(&args,"selectedScene")?).await?),'));
  expect(run(['--root',path,'--check']).status).toBe(1);expect(run(['--root',path]).status).toBe(0);
  expect(readFileSync(join(path,output),'utf8')).toContain('load_scene: {args: {selectedScene: string;}; result: Snapshot<TScene>}');
 });
 it.each(['dispatch shape','explicit argument type','result type'])('fails closed on covered RPC %s drift',kind=>{
  const path=fixtureRoot();const file=join(path,kind==='result type'?source:'crates/companion/src/main.rs');const before=readFileSync(join(path,output),'utf8');let changed=readFileSync(file,'utf8');
  if(kind==='dispatch shape')changed=changed.replace('"load_scene" => encode(storage.load_scene','"load_scene" => wrap(storage.load_scene');
  else if(kind==='explicit argument type')changed=changed.replace('storage.load_scene(&argument::<String>','storage.load_scene(&argument::<u32>');
  else changed=changed.replace('pub async fn load_scene(&self,sid:&str)->Result<Snapshot,','pub async fn load_scene(&self,sid:&str)->Result<Box<Snapshot>,');
  writeFileSync(file,changed);expect(run(['--root',path]).status).toBe(2);expect(readFileSync(join(path,output),'utf8')).toBe(before);
 });
 it('cross-checks generated keys and primitive shapes against actual cached Rust serde',()=>{
  const path=temp();const emitted=run(['--rust-fixture']);expect(emitted.status).toBe(0);writeFileSync(join(path,'fixture.rs'),emitted.stdout);
  const deps=join(root,'target/debug/deps');const files=readdirSync(deps);const serde=files.filter(name=>/^libserde-[\da-f]+\.rlib$/.test(name));const json=files.filter(name=>/^libserde_json-[\da-f]+\.rlib$/.test(name));
  expect(serde.length,'cached serde fixture libraries required').toBeGreaterThan(0);expect(json.length,'cached serde_json fixture libraries required').toBeGreaterThan(0);
  let compiled=false;
  for(const ser of serde){for(const js of json){
   const result=spawnSync(process.env.RUSTC??join(homedir(),'.cargo/bin/rustc'),['--edition=2021',join(path,'fixture.rs'),'-L',`dependency=${deps}`,'--extern',`serde=${join(deps,ser)}`,'--extern',`serde_json=${join(deps,js)}`,'-o',join(path,'fixture')],{encoding:'utf8',timeout:20000});
   if(result.status===0){compiled=true;break;}
  }if(compiled)break;}
  expect(compiled,'a compatible cached serde pair must compile the isolated fixture').toBe(true);
  const executed=spawnSync(join(path,'fixture'),[],{encoding:'utf8',timeout:5000});expect(executed.status).toBe(0);const values=JSON.parse(executed.stdout) as Record<string,Record<string,unknown>>;
  for(const [name,schema] of Object.entries(workspaceWireSchema)){
   expect(Object.keys(values[name]).sort(),name).toEqual(schema.fields.map(field=>field.name).sort());
   for(const field of schema.fields){const value=values[name][field.name];if(field.rustType==='String')expect(typeof value).toBe('string');else if(['u32','i64'].includes(field.rustType))expect(typeof value).toBe('number');else if(field.rustType.startsWith('Vec<'))expect(Array.isArray(value)).toBe(true);}
  }
  expect(values.UniverseNovelCreated.project).toEqual(values.Project);expect(values.UniverseNovelCreated.scene).toEqual(values.SceneRecord);expect(typeof values.UniverseNovelCreated.catalogueRevisionId).toBe('string');expect(values.UniverseCatalogue.schemaVersion).toBe(1);
  expect(values.__rpcUnit).toBeNull();expect(values.ProjectIndex.scenes).toEqual([values.SceneSummary]);expect(values.SceneSummary).not.toHaveProperty('scene');expect(values.ProjectAsset.content).toEqual([0,128,255]);expect(values.Scene.title).toContain('مِ 中文 😀');expect(values.Snapshot.revisionId).toEqual(expect.any(String));expect(values.Scene.document).toEqual({type:'doc',content:[]});
 },60000);
});

describe('workspace save acknowledgements',()=>{
 it.each(['save_project_data','save_project_data_checked'])('publishes %s only after native acknowledgement and preserves the result',async(operation)=>{
  const dispatchEvent=vi.fn();vi.stubGlobal('window',{dispatchEvent});class Event{constructor(public type:string,public options:{detail:unknown}){}}vi.stubGlobal('CustomEvent',Event);
  let acknowledge!:(value:unknown)=>void;invoke.mockImplementation(()=>new Promise(resolve=>{acknowledge=resolve;}));const saved={revisionId:'confirmed',payload:{}};
  const pending=workspaceCommand(operation,{projectId:'project',kind:'sceneMetadata'});expect(dispatchEvent).not.toHaveBeenCalled();acknowledge(saved);expect(await pending).toBe(saved);expect(dispatchEvent).toHaveBeenCalledOnce();expect(dispatchEvent.mock.calls[0][0]).toEqual(new Event('workspace-data-saved',{detail:{projectId:'project',kind:'sceneMetadata'}}));
 });
 it('publishes no event on failed writes, reads, other operations, or missing identifiers',async()=>{
  const dispatchEvent=vi.fn();vi.stubGlobal('window',{dispatchEvent});invoke.mockRejectedValueOnce(new Error('stale revision'));await expect(workspaceCommand('save_project_data',{projectId:'project',kind:'settings'})).rejects.toThrow('stale revision');invoke.mockResolvedValue({});
  for(const [operation,args] of [['load_project_data',{projectId:'project',kind:'settings'}],['save_scene',{projectId:'project',kind:'settings'}],['save_project_data',{projectId:'project'}]] as const)await workspaceCommand(operation,args);
  expect(dispatchEvent).not.toHaveBeenCalled();
 });
 it('keeps acknowledged writes usable without a browser window',async()=>{vi.stubGlobal('window',undefined);invoke.mockResolvedValue('saved');expect(await workspaceCommand('save_project_data',{projectId:'project',kind:'settings'})).toBe('saved');});
});
