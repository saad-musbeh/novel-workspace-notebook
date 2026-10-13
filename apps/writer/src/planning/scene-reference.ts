import type { CharacterKnowledge, PlanningDocument, ReaderReveal, StoryEvent, StoryFact, StoryRecord } from './model';
import type { SceneReference } from './controls';

export type EventOrder = 'before'|'after'|'same'|'unordered'|'unknown'|'contradiction';
export type Applicability = 'applies'|'not-yet'|'ended'|'unknown'|'unordered'|'contradiction'|'unscoped'|'outside-scenes'|'stale-fact';
export interface KnowledgeAtScene { entry:CharacterKnowledge; applicability:Applicability }
export interface RevealRow { reveal:ReaderReveal; scene:SceneReference|null; scenePosition:number|null; title:string }
export interface ReferenceIndex {
  document:PlanningDocument; scenes:ReadonlyMap<string,SceneReference>; scenePositions:ReadonlyMap<string,number>;
  records:ReadonlyMap<string,StoryRecord>; facts:ReadonlyMap<string,StoryFact>; events:ReadonlyMap<string,StoryEvent>;
  recordsByScene:ReadonlyMap<string,Set<string>>; eventsByScene:ReadonlyMap<string,StoryEvent[]>; revealsByScene:ReadonlyMap<string,ReaderReveal[]>;
  fieldsByKind:ReadonlyMap<StoryRecord['kind'],import('./model').FieldDefinition[]>; factsBySubject:ReadonlyMap<string,StoryFact[]>; factsByEvent:ReadonlyMap<string,StoryFact[]>; knowledgeByCharacter:ReadonlyMap<string,CharacterKnowledge[]>;
  bounds:ReadonlyMap<string,[number,number]|null>;
  order:(left:string,right:string)=>EventOrder; limited:()=>boolean; consume:()=>boolean;
}
function append<K,V>(map:Map<K,V[]>,key:K,value:V){const list=map.get(key);if(list)list.push(value);else map.set(key,[value]);}
/** One metadata pass; cached queries share a bounded traversal budget. No scene bodies. */
export function buildReferenceIndex(document:PlanningDocument,scenes:readonly SceneReference[]):ReferenceIndex {
  const sceneMap=new Map(scenes.map(scene=>[scene.id,scene]));const scenePositions=new Map(scenes.map((scene,i)=>[scene.id,i]));
  const records=new Map(document.records.map(r=>[r.id,r])),facts=new Map(document.facts.map(f=>[f.id,f])),events=new Map(document.events.map(e=>[e.id,e]));
  const fieldsByKind=new Map<StoryRecord['kind'],import('./model').FieldDefinition[]>();for(const field of document.fields)if(!field.hidden)append(fieldsByKind,field.recordKind,field);
  const eventsByScene=new Map<string,StoryEvent[]>(),recordsByScene=new Map<string,Set<string>>(),revealsByScene=new Map<string,ReaderReveal[]>(),factsBySubject=new Map<string,StoryFact[]>(),factsByEvent=new Map<string,StoryFact[]>(),knowledgeByCharacter=new Map<string,CharacterKnowledge[]>();
  for(const event of document.events)for(const sceneId of event.sceneIds)append(eventsByScene,sceneId,event);
  const linkRecord=(sceneId:string,id:string)=>{const ids=recordsByScene.get(sceneId)??new Set<string>();ids.add(id);recordsByScene.set(sceneId,ids);};
  for(const record of document.records){
    const links=[...record.links,...Object.values(record.values).flatMap(value=>Array.isArray(value)?value.filter((item):item is {kind:'scene'|'event';id:string}=>!!item&&typeof item==='object'&&'kind' in item&&['scene','event'].includes(item.kind)):[])];
    for(const link of links){if(link.kind==='scene')linkRecord(link.id,record.id);else if(link.kind==='event')for(const id of events.get(link.id)?.sceneIds??[])linkRecord(id,record.id);}
  }
  for(const fact of document.facts){append(factsBySubject,fact.subjectId,fact);if(fact.eventId)append(factsByEvent,fact.eventId,fact);if(fact.untilEventId&&fact.untilEventId!==fact.eventId)append(factsByEvent,fact.untilEventId,fact);}
  for(const reveal of document.readerReveals??[])append(revealsByScene,reveal.scene.id,reveal);
  for(const entry of document.knowledge??[])append(knowledgeByCharacter,entry.characterId,entry);
  // Resolve relative chains iteratively, including deep/cyclic imports without recursion.
  const bounds=new Map<string,[number,number]|null>();
  for(const event of document.events){
    if(bounds.has(event.id))continue;const path:StoryEvent[]=[],seen=new Set<string>();let current:StoryEvent|undefined=event;
    while(current&&!bounds.has(current.id)){
      if(seen.has(current.id)){for(const item of path)bounds.set(item.id,null);break;}
      seen.add(current.id);path.push(current);
      if(current.time.kind!=='relative'){const time=current.time;bounds.set(current.id,time.kind==='fixed'?[time.day,time.day]:(time.kind==='bounds'||time.kind==='interval')?[time.start,time.end]:null);break;}
      current=events.get(current.time.eventId);
    }
    for(let i=path.length-1;i>=0;i--){const item=path[i];if(bounds.has(item.id))continue;const anchor=item.time.kind==='relative'?bounds.get(item.time.eventId):null;const result: [number,number]|null=anchor&&item.time.kind==='relative'?[anchor[0]+item.time.offsetDays,anchor[1]+item.time.offsetDays]:null;bounds.set(item.id,result&&result.every(Number.isFinite)?result:null);}
  }
  const edges=new Map<string,{id:string;strict:boolean;days:number}[]>();
  for(const constraint of document.constraints)if(constraint.confirmed)append(edges,constraint.beforeId,{id:constraint.afterId,strict:constraint.strict||constraint.minimumDays>0,days:constraint.minimumDays});
  const cache=new Map<string,Map<string,EventOrder>>();let remaining=20000,wasLimited=false;
  const consume=()=>{if(remaining--<=0){wasLimited=true;return false;}return true;};
  const conflicting=new Set<string>(),lower=new Map<string,{day:number;strict:boolean}>(),feasibilityQueue:string[]=[];
  for(const [id,range] of bounds)if(range){lower.set(id,{day:range[0],strict:false});feasibilityQueue.push(id);}
  let feasibilityCursor=0;
  while(feasibilityCursor<feasibilityQueue.length){
    if(!consume())break;const id=feasibilityQueue[feasibilityCursor++],base=lower.get(id)!;
    for(const edge of edges.get(id)??[]){
      if(!consume())break;const candidate={day:base.day+edge.days,strict:base.strict||edge.strict},upper=bounds.get(edge.id);
      if(upper&&(candidate.day>upper[1]||candidate.day===upper[1]&&candidate.strict)){conflicting.add(id);conflicting.add(edge.id);continue;}
      const prior=lower.get(edge.id);if(!prior||candidate.day>prior.day||candidate.day===prior.day&&candidate.strict&&!prior.strict){lower.set(edge.id,candidate);feasibilityQueue.push(edge.id);}
    }
  }
  const path=(from:string,to:string):boolean|null=>{
    const queue:{id:string;strict:boolean}[]=[{id:from,strict:false}],seen=new Map<string,number>();let cursor=0;
    while(cursor<queue.length){if(!consume())return null;const item=queue[cursor++];if(item.id===to&&item.strict)return true;const mask=item.strict?2:1;if((seen.get(item.id)??0)&mask)continue;seen.set(item.id,(seen.get(item.id)??0)|mask);for(const edge of edges.get(item.id)??[]){if(!consume())return null;queue.push({id:edge.id,strict:item.strict||edge.strict});}}
    return false;
  };
  const order=(left:string,right:string):EventOrder=>{
    const cached=cache.get(left)?.get(right);if(cached)return cached;
    let result:EventOrder;
    if(!events.has(left)||!events.has(right))result='unknown';
    else if(conflicting.has(left)||conflicting.has(right))result='contradiction';
    else {
      const forward=path(left,right),backward=left===right?forward:path(right,left),a=bounds.get(left),b=bounds.get(right);
      if(forward===null||backward===null)result='unknown';
      else if((forward&&backward)||(forward&&a&&b&&a[0]>=b[1])||(backward&&a&&b&&b[0]>=a[1]))result='contradiction';
      else if(left===right)result='same';
      else if(forward||(a&&b&&a[1]<b[0]))result='before';
      else if(backward||(a&&b&&b[1]<a[0]))result='after';
      else result=a&&b?'unordered':'unknown';
    }
    const row=cache.get(left)??new Map<string,EventOrder>();row.set(right,result);cache.set(left,row);return result;
  };
  return {document,scenes:sceneMap,scenePositions,records,facts,events,recordsByScene,eventsByScene,revealsByScene,fieldsByKind,factsBySubject,factsByEvent,knowledgeByCharacter,bounds,order,limited:()=>wasLimited,consume};
}
function evaluateEventApplicability(index:ReferenceIndex,sceneId:string,fromId:string,untilId:string|null):Applicability {
  if(untilId){const range=index.order(fromId,untilId);if(range==='after'||range==='same'||range==='contradiction')return 'contradiction';}
  const sceneEvents=index.eventsByScene.get(sceneId)??[];if(!sceneEvents.length)return 'unknown';
  const statuses:Applicability[]=[];for(const event of sceneEvents){
    if(!index.consume())return 'unknown';
    const state=(():Applicability=>{
    const from=index.order(fromId,event.id),until=untilId?index.order(event.id,untilId):'before';
    if(from==='contradiction'||until==='contradiction')return 'contradiction';
    if(from==='after')return 'not-yet';if(until==='after'||until==='same')return 'ended';
    if(from==='unknown'||until==='unknown')return 'unknown';if(from==='unordered'||until==='unordered')return 'unordered';
    return 'applies';
    })();statuses.push(state);if(state==='contradiction')return state;
  }
  if(statuses.includes('contradiction'))return 'contradiction';
  if(statuses.every(status=>status===statuses[0]))return statuses[0];
  // A scene can contain events on both sides of a boundary. No invented intra-scene order.
  return statuses.includes('unknown')?'unknown':'unordered';
}
const applicabilityCache=new WeakMap<ReferenceIndex,Map<string,Map<string,Map<string|null,Applicability>>>>();
export function eventApplicability(index:ReferenceIndex,sceneId:string,fromId:string,untilId:string|null):Applicability {
  const scenes=applicabilityCache.get(index)??new Map();applicabilityCache.set(index,scenes);
  const from=scenes.get(sceneId)??new Map();scenes.set(sceneId,from);const until=from.get(fromId)??new Map();from.set(fromId,until);
  if(!until.has(untilId))until.set(untilId,evaluateEventApplicability(index,sceneId,fromId,untilId));return until.get(untilId)!;
}
export function knowledgeAtScene(index:ReferenceIndex,sceneId:string,characterId?:string):KnowledgeAtScene[] {
  const entries=characterId?index.knowledgeByCharacter.get(characterId)??[]:index.document.knowledge??[];
  return entries.map(entry=>{const fact=index.facts.get(entry.factId);const stale=!fact||entry.stance==='knows'&&(!fact.confirmed||fact.status!=='fact')||fact.subjectId!==entry.factSnapshot.subjectId||fact.property!==entry.factSnapshot.property||fact.value!==entry.factSnapshot.value;return {entry,applicability:stale?'stale-fact':entry.scope.kind==='scenes'?(entry.scope.sceneIds.includes(sceneId)?'applies':'outside-scenes'):eventApplicability(index,sceneId,entry.scope.fromEventId,entry.scope.untilEventId)};});
}
export function factAtScene(index:ReferenceIndex,fact:StoryFact,sceneId:string):Applicability {return fact.eventId?eventApplicability(index,sceneId,fact.eventId,fact.untilEventId):'unscoped';}
export function readerTimeline(index:ReferenceIndex):RevealRow[] {
  return (index.document.readerReveals??[]).map(reveal=>({reveal,scene:index.scenes.get(reveal.scene.id)??null,scenePosition:index.scenePositions.get(reveal.scene.id)??null,title:reveal.eventId?index.events.get(reveal.eventId)?.title??'Missing event':(()=>{const fact=index.facts.get(reveal.factId!);return fact?`${fact.property}: ${fact.value}`:'Missing fact';})()})).sort((a,b)=>(a.scenePosition??Infinity)-(b.scenePosition??Infinity)||a.reveal.position-b.reveal.position||a.reveal.id.localeCompare(b.reveal.id));
}
export function storyTimeline(index:ReferenceIndex):{event:StoryEvent;bounds:[number,number]|null}[] {
  return [...index.events.values()].map(event=>({event,bounds:index.bounds.get(event.id)??null})).sort((a,b)=>(a.bounds?.[0]??Infinity)-(b.bounds?.[0]??Infinity)||a.event.id.localeCompare(b.event.id));
}
export function projectSceneReference(index:ReferenceIndex,sceneId:string){
  const ids=new Set(index.recordsByScene.get(sceneId)??[]),events=index.eventsByScene.get(sceneId)??[],reveals=index.revealsByScene.get(sceneId)??[];
  const factIds=new Set<string>();for(const reveal of reveals)if(reveal.factId)factIds.add(reveal.factId);
  for(const event of events)for(const fact of index.factsByEvent.get(event.id)??[]){factIds.add(fact.id);ids.add(fact.subjectId);}
  const knowledge=knowledgeAtScene(index,sceneId).filter(row=>row.applicability==='applies'||ids.has(row.entry.characterId)||row.entry.scope.kind==='scenes'&&row.entry.scope.sceneIds.includes(sceneId));
  for(const row of knowledge){ids.add(row.entry.characterId);factIds.add(row.entry.factId);}
  for(const id of ids)for(const fact of index.factsBySubject.get(id)??[])factIds.add(fact.id);
  const records=[...ids].flatMap(id=>index.records.has(id)?[index.records.get(id)!]:[]);
  const facts=[...factIds].flatMap(id=>index.facts.has(id)?[{fact:index.facts.get(id)!,applicability:factAtScene(index,index.facts.get(id)!,sceneId)}]:[]);
  return {scene:index.scenes.get(sceneId)??null,records,facts,events,knowledge,reveals,limited:index.limited()};
}
export const applicabilityLabel:Record<Applicability,string>={applies:'Applies', 'not-yet':'Not yet',ended:'Ended',unknown:'Timing unknown',unordered:'Order unresolved',contradiction:'Conflicting order',unscoped:'No declared start', 'outside-scenes':'Outside selected scenes','stale-fact':'Referenced fact changed'};

/** Display preview only; source codepoints and strings are never modified. */
export function referencePreview(text:string,limit=2000):string {
  if(text.length<=limit)return text;let end=limit;
  if(/[\uD800-\uDBFF]/u.test(text[end-1]))end--;return text.slice(0,end)+'…';
}
