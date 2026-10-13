import {validatePlanningDocument,type PlanningDocument,type StoryFact,type StoryEvent} from './model';
import type {SceneReference} from './controls';
export interface RippleMutation {kind:'fact'|'event';id:string;before:StoryFact|StoryEvent|null;after:StoryFact|StoryEvent|null}
export interface RippleDependency {kind:'scene'|'knowledge'|'reader-reveal'|'relative-event'|'constraint'|'board-card'|'board-edge'|'record'|'plot-point'|'fact'|'issue';id:string;label:string;reason:string}
export interface PlanningRipple {mutations:RippleMutation[];dependencies:RippleDependency[];limited:boolean;errors:string[]}
const factMeaning=(fact:StoryFact)=>[fact.subjectId,fact.property,fact.value,fact.status,fact.confirmed,fact.observerId,fact.eventId,fact.untilEventId];
/** Stable comparison only. It never rewrites strings or stored JSON. */
export function planningFingerprint(value:unknown):string {
 return JSON.stringify(value,(_key,item)=>item&&typeof item==='object'&&!Array.isArray(item)?Object.fromEntries(Object.keys(item).sort().map(key=>[key,item[key]])):item);
}
export function previewPlanningRipple(before:PlanningDocument,next:PlanningDocument,scenes:readonly SceneReference[],limit=200):PlanningRipple {
 const mutations:RippleMutation[]=[];const beforeFacts=new Map(before.facts.map(fact=>[fact.id,fact])),nextFacts=new Map(next.facts.map(fact=>[fact.id,fact]));
 for(const [id,fact] of beforeFacts){const after=nextFacts.get(id)??null;if(!after||planningFingerprint(factMeaning(fact))!==planningFingerprint(factMeaning(after)))mutations.push({kind:'fact',id,before:fact,after});}
 for(const fact of next.facts)if(!beforeFacts.has(fact.id)&&fact.confirmed)mutations.push({kind:'fact',id:fact.id,before:null,after:fact});
 const nextEvents=new Map(next.events.map(event=>[event.id,event]));
 for(const event of before.events){const after=nextEvents.get(event.id)??null;if(!after||planningFingerprint(event.time)!==planningFingerprint(after.time))mutations.push({kind:'event',id:event.id,before:event,after});}
 const dependencies:RippleDependency[]=[],seen=new Set<string>(),sceneLabels=new Map(scenes.map(scene=>[scene.id,scene.title])),facts=new Set(mutations.filter(item=>item.kind==='fact').map(item=>item.id)),events=new Set(mutations.filter(item=>item.kind==='event').map(item=>item.id));let work=0,limited=false;
 const add=(kind:RippleDependency['kind'],id:string,label:string,reason:string)=>{const key=kind+':'+id;if(seen.has(key))return;seen.add(key);if(dependencies.length<limit)dependencies.push({kind,id,label,reason});else limited=true;};
 const visit=()=>{if(++work>20000){limited=true;return false;}return true;};
 const scene=(id:string,reason:string)=>add('scene',id,sceneLabels.get(id)??'Missing scene',reason);
 // Follow only authored relative links, with one indexed traversal in each document.
 for(const doc of [before,next]){
  const relative=new Map<string,StoryEvent[]>();for(const event of doc.events){if(!visit())break;if(event.time.kind==='relative'){const list=relative.get(event.time.eventId)??[];list.push(event);relative.set(event.time.eventId,list);}}
  const queue=[...events],visited=new Set(queue);for(let cursor=0;cursor<queue.length&&visit();cursor++)for(const event of relative.get(queue[cursor])??[]){if(!visit())break;if(!visited.has(event.id)){visited.add(event.id);events.add(event.id);queue.push(event.id);add('relative-event',event.id,event.title,'Relative date depends on changed time');}}
  for(const event of doc.events){if(!visit())break;if(events.has(event.id))for(const id of event.sceneIds)scene(id,'Occurrence event time affected');}
  for(const fact of doc.facts){if(!visit())break;if(events.has(fact.eventId??'')||events.has(fact.untilEventId??'')){facts.add(fact.id);add('fact',fact.id,`${fact.property}: ${fact.value}`,'Event-scoped applicability');}}
  for(const entry of doc.knowledge??[]){if(!visit())break;if(facts.has(entry.factId)||entry.scope.kind==='events'&&(events.has(entry.scope.fromEventId)||events.has(entry.scope.untilEventId??''))){add('knowledge',entry.id,`${entry.stance}: ${entry.factSnapshot.property}`,'Recorded knowledge references affected fact/time');if(entry.scope.kind==='scenes')for(const id of entry.scope.sceneIds)scene(id,'Explicit knowledge scene');}}
  for(const reveal of doc.readerReveals??[]){if(!visit())break;if(facts.has(reveal.factId??'')||events.has(reveal.eventId??'')){add('reader-reveal',reveal.id,'Reader reveal','Authored reveal references affected fact/event');scene(reveal.scene.id,'Authored reveal scene');}}
  for(const constraint of doc.constraints){if(!visit())break;if(events.has(constraint.beforeId)||events.has(constraint.afterId))add('constraint',constraint.id,constraint.notes||'Chronology constraint',constraint.confirmed?'Confirmed time constraint':'Unconfirmed constraint');}
  const changedSubjects=new Set(mutations.filter(item=>item.kind==='fact').flatMap(item=>[item.before,item.after].filter((fact):fact is StoryFact=>fact!==null&&'subjectId' in fact).map(fact=>fact.subjectId)));
  const relevantLink=(link:{kind:string;id:string})=>link.kind==='event'&&events.has(link.id)||link.kind==='record'&&changedSubjects.has(link.id);
  for(const record of doc.records){if(!visit())break;if(changedSubjects.has(record.id)||[...record.links,...Object.values(record.values).flatMap(value=>Array.isArray(value)?value.filter((item):item is import('./model').RecordLink=>!!item&&typeof item==='object'&&'kind' in item):[])].some(relevantLink)){add('record',record.id,record.name,'Canonical subject or event link');for(const link of record.links)if(link.kind==='scene')scene(link.id,'Record scene link');}}
  for(const point of doc.plotPoints){if(!visit())break;if(point.links.some(relevantLink)){add('plot-point',point.id,point.title,'Authored canonical link');for(const link of point.links)if(link.kind==='scene')scene(link.id,'Plot scene link');}}
  for(const board of doc.boards){const cards=new Set<string>();for(const card of board.cards){if(!visit())break;if(card.reference&&relevantLink(card.reference)){cards.add(card.id);add('board-card',card.id,card.title||board.name,'Board representation of affected record/event');}}for(const edge of board.edges){if(!visit())break;if(cards.has(edge.fromId)||cards.has(edge.toId))add('board-edge',edge.id,edge.label||edge.kind,'Edge touches affected representation');}}
  for(const issue of doc.issues){if(!visit())break;if(issue.links.some(relevantLink)||issue.evidence.some(evidence=>evidence.link&&relevantLink(evidence.link)))add('issue',issue.id,issue.description,'Manual issue/evidence canonical link');}
 }
 const errors:string[]=[];try{validatePlanningDocument(next);}catch(error){errors.push(String(error));}
 return {mutations,dependencies,limited,errors};
}
export function assertFreshRipple(base:PlanningDocument,acknowledged:PlanningDocument,expectedRevision:string|null,actualRevision:string|null){
 if(expectedRevision!==actualRevision||planningFingerprint(base)!==planningFingerprint(acknowledged))throw new Error('Planning changed since this preview. Cancel and refresh before proposing the change again.');
}
export async function confirmPlanningRipple(options:{base:PlanningDocument;proposed:PlanningDocument;drain:()=>Promise<void>;read:()=>Promise<import('./model').PlanningSnapshot>;expectedRevision:()=>string|null;isCurrent:()=>boolean;isComposing:()=>boolean}):Promise<PlanningDocument>{
 const check=()=>{if(!options.isCurrent())throw new Error('This preview is stale. Cancel and review the current planning draft.');if(options.isComposing())throw new Error('Finish entering the current word before applying this change.');};
 check();validatePlanningDocument(options.proposed);const proposed=structuredClone(options.proposed);await options.drain();check();const acknowledged=await options.read();check();validatePlanningDocument(acknowledged.document);assertFreshRipple(options.base,acknowledged.document,options.expectedRevision(),acknowledged.revisionId);return proposed;
}
