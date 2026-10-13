import type { ContinuityWorld } from '../assistance/continuity';
import type { SourceRef } from '../assistance/model';
import { eventBounds, type PlanningDocument } from './model';
/** Only author-approved world facts participate. Uncertain bounds never imply presence. */
export function toContinuityWorld(doc:PlanningDocument,sources:SourceRef[]):ContinuityWorld {
  const usableFacts=doc.facts.filter(f=>f.confirmed&&f.status==='fact'&&f.eventId);
  const refs=(eventId:string)=>{const event=doc.events.find(e=>e.id===eventId);return sources.filter(s=>event?.sceneIds.includes(s.sceneId));};
  const definite=(id:string,seen=new Set<string>()):boolean=>{if(seen.has(id))return false;const event=doc.events.find(e=>e.id===id);if(!event)return false;if(event.time.kind==='fixed'||event.time.kind==='interval')return true;if(event.time.kind==='relative')return definite(event.time.eventId,new Set([...seen,id]));return false;};
  return {
    events:doc.events.map(event=>{const bounds=definite(event.id)?eventBounds(doc,event.id):null;return {id:event.id,label:event.title,confirmed:!!bounds,...(bounds?{start:bounds[0],end:bounds[1]}:{})};}),
    before:doc.constraints.filter(c=>c.strict||c.minimumDays>0).map(c=>({id:c.id,beforeId:c.beforeId,afterId:c.afterId,confirmed:c.confirmed})),
    prerequisites:doc.boards.flatMap(board=>board.edges.filter(edge=>edge.kind==='depends-on'&&edge.confirmed===true).flatMap(edge=>{const dependent=board.cards.find(c=>c.id===edge.fromId)?.reference;const prerequisite=board.cards.find(c=>c.id===edge.toId)?.reference;if(dependent?.kind!=='event'||prerequisite?.kind!=='event'||!doc.events.some(e=>e.id===dependent.id)||!doc.events.some(e=>e.id===prerequisite.id))return [];return [{id:edge.id,dependentEventId:dependent.id,prerequisiteEventId:prerequisite.id,confirmed:true,sources:refs(dependent.id),evidence:[]}];})),
    knowledge:usableFacts.filter(f=>f.property==='knowledge:learn'||f.property==='knowledge:use').map(f=>({id:f.id,characterId:f.subjectId,factId:f.value,eventId:f.eventId!,kind:f.property==='knowledge:learn'?'learn':'use',confirmed:true,sources:refs(f.eventId!),evidence:[]})),
    presence:usableFacts.filter(f=>f.property==='exclusive-location').map(f=>({id:f.id,subjectId:f.subjectId,locationId:f.value,eventId:f.eventId!,exclusive:true,confirmed:true,sources:refs(f.eventId!),evidence:[]})),
    states:usableFacts.filter(f=>!['knowledge:learn','knowledge:use','exclusive-location'].includes(f.property)).map(f=>({id:f.id,subjectId:f.subjectId,property:f.property,value:f.value,eventId:f.eventId!,...(f.untilEventId?{untilEventId:f.untilEventId}:{}),confirmed:true,sources:refs(f.eventId!),evidence:[]})),
    setups:doc.plotPoints.map(point=>({id:point.id,label:point.title,requiresPayoff:point.requiresPayoff,closed:point.scopeClosed,...(point.payoffId?{payoffId:point.payoffId}:{}),sources:sources.filter(s=>point.links.some(link=>link.kind==='scene'&&link.id===s.sceneId)),evidence:[]})),sources,
  };
}
