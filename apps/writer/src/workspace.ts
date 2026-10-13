import { invoke } from '@tauri-apps/api/core';
import type { Repository } from './storage';
import { validateScene, type Scene } from './document';
import {typingIsActive,waitForTypingIdle} from './typing-priority';
import type {Snapshot,SceneRecord as WireSceneRecord,ProjectBundle as WireProjectBundle,ProjectDataSnapshot,ProjectIndex,SceneSummary,StorageRpcContract} from './generated/workspace-contract';
export type {Project,Chapter,Revision,ProjectAsset,ProjectIndex,SceneSummary} from './generated/workspace-contract';
// Rust document/payload Values remain opaque in the generated contract; this
// binding uses the existing validated frontend document schema.
export type SceneRecord = WireSceneRecord<Scene>;
export type ProjectBundle = WireProjectBundle<Scene>;
export type DataSnapshot<T> = ProjectDataSnapshot<T>;
export function workspaceDirectCommand<K extends keyof StorageRpcContract<Scene>>(operation:K,args:StorageRpcContract<Scene>[K]['args']):Promise<StorageRpcContract<Scene>[K]['result']>{return workspaceCommand(operation,args);}
export function summarizeScene(record:SceneRecord):SceneSummary{return {id:record.id,chapterId:record.chapterId,position:record.position,revisionId:record.revisionId,title:record.scene.title};}
export function projectIndexFromBundle(bundle:ProjectBundle):ProjectIndex{return {project:bundle.project,chapters:bundle.chapters,scenes:bundle.scenes.map(summarizeScene),chapterNumberStart:bundle.chapterNumberStart,chapterNumberingRevisionId:bundle.chapterNumberingRevisionId,chapterNumberingConflicted:bundle.chapterNumberingConflicted};}
export interface ProjectSelection {index:ProjectIndex;scene:SceneRecord|null;fullBundle:ProjectBundle|null}
type WorkspaceReader=<T>(operation:string,args:Record<string,unknown>)=>Promise<T>;
/** Read before publishing: a failed scoped load never substitutes a document or changes a draft. */
export async function readProjectSelection(rpc:WorkspaceReader,projectId:string,{sceneId,includeDocuments=false,missingScene='first'}:{sceneId?:string;includeDocuments?:boolean;missingScene?:'error'|'first'|'none'}={}):Promise<ProjectSelection>{
  const fullBundle=includeDocuments?await rpc<ProjectBundle>('open_project',{projectId}):null;
  let index=fullBundle?projectIndexFromBundle(fullBundle):await rpc<ProjectIndex>('open_project_index',{projectId});
  if(index.project.id!==projectId)throw new Error('The loaded project does not match this tab.');
  const requested=sceneId?index.scenes.find(scene=>scene.id===sceneId):undefined;
  if(sceneId&&!requested&&missingScene==='error')throw new Error('This scene is no longer in the manuscript. Open Project to review deleted scenes. Your current tab is kept.');
  const selected=requested??(missingScene==='none'?undefined:index.scenes[0]);
  const scene=selected?(fullBundle?.scenes.find(record=>record.id===selected.id)??await rpc<SceneRecord>('load_project_scene',{projectId,sceneId:selected.id})):null;
  if(scene){
    validateScene(scene.scene);
    if(scene.id!==selected!.id||!index.chapters.some(chapter=>chapter.id===scene.chapterId&&chapter.projectId===projectId))throw new Error('The scene changed project or chapter during loading. Try again; your current draft is kept.');
    // The head can advance between index and scoped reads. Bind the selected
    // row to the exact verified scene returned by the scoped transaction.
    index={...index,scenes:index.scenes.map(row=>row.id===scene.id?summarizeScene(scene):row)};
  }
  return {index,scene,fullBundle};
}
export async function workspaceCommand<T>(operation:string,args:Record<string,unknown>={}):Promise<T>{
  const saved=await invoke<T>('workspace_command',{operation,args});
  if ((operation==='save_project_data'||operation==='save_project_data_checked')&&typeof window!=='undefined'&&typeof args.projectId==='string'&&typeof args.kind==='string') {
    window.dispatchEvent(new CustomEvent('workspace-data-saved',{detail:{projectId:args.projectId,kind:args.kind}}));
  }
  return saved;
}
/** Read-only background work yields to input; write acknowledgements never wait. */
export async function workspaceBackgroundCommand<T>(operation:string,args:Record<string,unknown>={},signal?:AbortSignal):Promise<T>{
  signal?.throwIfAborted();
  if(typingIsActive())await waitForTypingIdle(signal);
  signal?.throwIfAborted();
  const value=await workspaceCommand<T>(operation,args);
  if(typingIsActive())await waitForTypingIdle(signal);
  signal?.throwIfAborted();
  return value;
}
export function sceneRepository(sceneId:string):Repository {
  let attempt:{key:string;id:string}|null=null;
  return {
    async load(){const snapshot=await workspaceCommand<Snapshot<Scene>>('load_scene',{sceneId});validateScene(snapshot.scene);return snapshot;},
    async save(scene,parent){validateScene(scene);const key=JSON.stringify([parent,scene]);if(attempt?.key!==key)attempt={key,id:crypto.randomUUID()};const saved=await workspaceCommand<Snapshot<Scene>>('save_scene',{sceneId,scene,parent,requestId:attempt.id});attempt=null;return saved;}
  };
}
