import type {PlanningDocument,PlotPoint,ReaderReveal,StoryEvent,StoryFact} from './model';
import type {SceneReference} from './controls';
export interface RehearsalFact {fact:StoryFact;reveals:ReaderReveal[]}
export interface RehearsalEvent {event:StoryEvent;reveals:ReaderReveal[]}
export interface RehearsalThread {point:PlotPoint;status:'open'|'payoff-unresolved'|'closed-without-visible-payoff'}
export function readerRehearsal(doc:PlanningDocument,scenes:readonly SceneReference[],sceneId:string|null,position:number|null=null){
 const order=new Map(scenes.map((scene,i)=>[scene.id,i])),progress=sceneId?order.get(sceneId):undefined;const factsById=new Map(doc.facts.map(fact=>[fact.id,fact])),eventsById=new Map(doc.events.map(event=>[event.id,event]));
 const visibleReveals:ReaderReveal[]=[],factRows=new Map<string,RehearsalFact>(),eventRows=new Map<string,RehearsalEvent>();let missingScenes=0,missingTargets=0,limited=false,work=0;
 const visit=()=>{if(++work>20000){limited=true;return false;}return true;};
 if(progress!==undefined)for(const reveal of doc.readerReveals??[]){if(!visit())break;const at=order.get(reveal.scene.id);if(at===undefined){missingScenes++;continue;}if(at>progress||at===progress&&position!==null&&reveal.position>position)continue;visibleReveals.push(reveal);if(reveal.factId){const fact=factsById.get(reveal.factId);if(!fact){missingTargets++;continue;}const row=factRows.get(fact.id)??{fact,reveals:[]};row.reveals.push(reveal);factRows.set(fact.id,row);}else if(reveal.eventId){const event=eventsById.get(reveal.eventId);if(!event){missingTargets++;continue;}const row=eventRows.get(event.id)??{event,reveals:[]};row.reveals.push(reveal);eventRows.set(event.id,row);}}
 visibleReveals.sort((a,b)=>order.get(a.scene.id)!-order.get(b.scene.id)!||a.position-b.position||a.id.localeCompare(b.id));
 const visiblePoints:PlotPoint[]=[];
 if(progress!==undefined)for(const point of doc.plotPoints){if(!visit())break;let visible=false;for(const link of point.links){if(!visit())break;if(link.kind==='scene'&&order.has(link.id)&&(order.get(link.id)!<progress||order.get(link.id)===progress&&position===null)||link.kind==='event'&&eventRows.has(link.id)){visible=true;break;}}if(visible)visiblePoints.push(point);}
 const pointIds=new Set(visiblePoints.map(point=>point.id));const threads:RehearsalThread[]=visiblePoints.filter(point=>point.requiresPayoff&&(!point.payoffId||!pointIds.has(point.payoffId))).map(point=>({point,status:point.scopeClosed?'closed-without-visible-payoff':point.payoffId?'payoff-unresolved':'open'}));
 // Conflicts are only explicitly revealed, currently approved facts for the same subject/property.
 const groups=new Map<string,Map<string,StoryFact[]>>();for(const row of factRows.values())if(row.fact.confirmed&&row.fact.status==='fact'){const properties=groups.get(row.fact.subjectId)??new Map<string,StoryFact[]>();groups.set(row.fact.subjectId,properties);const list=properties.get(row.fact.property)??[];list.push(row.fact);properties.set(row.fact.property,list);}
 const differentValueFactIds=new Set<string>();for(const properties of groups.values())for(const facts of properties.values())if(new Set(facts.map(fact=>fact.value)).size>1)for(const fact of facts)differentValueFactIds.add(fact.id);
 return {scene:sceneId?scenes.find(scene=>scene.id===sceneId)??null:null,progress:progress??null,reveals:visibleReveals,facts:[...factRows.values()],events:[...eventRows.values()],notes:visiblePoints,threads,differentValueFactIds,missingScenes,missingTargets,limited};
}
