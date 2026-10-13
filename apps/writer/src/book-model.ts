import type {ProjectIndex,SceneSummary} from './workspace';
import type {PlanningDocument} from './planning/model';
import {buildReferenceIndex} from './planning/scene-reference';

export function manuscriptOrder(index:ProjectIndex):SceneSummary[]{
  const rank=new Map([...index.chapters].sort((a,b)=>a.position-b.position||a.id.localeCompare(b.id)).map((chapter,i)=>[chapter.id,i]));
  return [...index.scenes].sort((a,b)=>(rank.get(a.chapterId)??Infinity)-(rank.get(b.chapterId)??Infinity)||a.position-b.position||a.id.localeCompare(b.id));
}

/** Authored links only: a name appearing in prose is not evidence of presence. */
export function characterSceneIds(document:PlanningDocument,scenes:readonly SceneSummary[],characterId:string):Set<string>{
  const result=new Set<string>();
  if(!document.records.some(record=>record.id===characterId&&record.kind==='character'))return result;
  const index=buildReferenceIndex(document,scenes);
  for(const [scene,records] of index.recordsByScene)if(records.has(characterId))result.add(scene);
  const facts=index.factsBySubject.get(characterId)??[],factIds=new Set(facts.map(fact=>fact.id)),eventIds=new Set<string>();
  for(const fact of facts)for(const eventId of [fact.eventId,fact.untilEventId])if(eventId)eventIds.add(eventId);
  for(const reveal of document.readerReveals??[])if(reveal.factId&&factIds.has(reveal.factId))result.add(reveal.scene.id);
  for(const entry of index.knowledgeByCharacter.get(characterId)??[]){
    if(entry.scope.kind==='scenes')for(const scene of entry.scope.sceneIds)result.add(scene);
    else for(const eventId of [entry.scope.fromEventId,entry.scope.untilEventId])if(eventId)eventIds.add(eventId);
  }
  for(const eventId of eventIds)for(const scene of index.events.get(eventId)?.sceneIds??[])result.add(scene);
  return new Set([...result].filter(id=>index.scenes.has(id)));
}

export interface BookRow {scene:SceneSummary;top:number;height:number}
export const BOOK_FOLDED_HEIGHT=72;
export const BOOK_REVEAL_EVENT='novel-book-reveal';
export const BOOK_REVEAL_CANCEL_EVENT='novel-book-cancel-reveal';
export const BOOK_REVEAL_RESULT_EVENT='novel-book-reveal-result';
export interface BookRevealRequest {requestId:string;sceneId?:string;offset?:number;characterId?:string}
export interface BookRevealResult {requestId:string;ok:boolean;error?:string}
export function initialBookFolds(ids:unknown,scenes:readonly SceneSummary[]):Set<string>{
  const known=new Set(scenes.map(scene=>scene.id)),folds=new Set<string>();
  if(Array.isArray(ids))for(const id of ids.slice(0,20000))if(typeof id==='string'&&known.has(id))folds.add(id);
  return folds;
}
export function bookRows(scenes:readonly SceneSummary[],heights:ReadonlyMap<string,number>,folded:ReadonlySet<string>=new Set()):BookRow[]{
  let top=0;
  return scenes.map(scene=>{const height=folded.has(scene.id)?BOOK_FOLDED_HEIGHT:Math.max(200,Math.min(100_000_000,heights.get(scene.id)??560));const row={scene,top,height};top+=height;return row;});
}
export interface BookAnchor {sceneId:string;offset:number}
export function bookAnchor(rows:readonly BookRow[],top:number):BookAnchor|null{
  top=Number.isFinite(top)?Math.max(0,top):0;
  const row=rows.find(row=>row.top<=top&&row.top+row.height>top)??rows.at(-1);
  return row?{sceneId:row.scene.id,offset:Math.max(0,top-row.top)}:null;
}
export function bookAnchorTop(rows:readonly BookRow[],anchor:BookAnchor):number|null{
  const row=rows.find(row=>row.scene.id===anchor.sceneId);
  return row?row.top+Math.max(0,Math.min(Number.isFinite(anchor.offset)?anchor.offset:0,Math.max(0,row.height-120))):null;
}
export function visibleBookRows(rows:readonly BookRow[],top:number,height:number):BookRow[]{
  const start=Math.max(0,top-300),end=top+Math.max(120,height)+300;
  let low=0,high=rows.length;
  while(low<high){const mid=(low+high)>>>1;if(rows[mid].top+rows[mid].height<start)low=mid+1;else high=mid;}
  const result:BookRow[]=[];
  for(let i=low;i<rows.length&&rows[i].top<end&&result.length<8;i++)result.push(rows[i]);
  return result;
}

/** Offset of a scene's visible portion inside a shared book scroll container. */
export function bookViewport(surface:{top:number;height:number},host:{top:number;height:number}){
  return {top:Math.max(0,host.top-surface.top),height:Math.max(120,Math.min(host.height,surface.height))};
}
