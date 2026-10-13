import type { PlanningDocument } from '../planning/model';
import { canonicalJson } from './packs';
import { sha256, type SceneSnapshot } from './model';
/** Fixed ASCII keys and UUID identities keep this projection identical in Rust and JavaScript. */
export function canonicalFactText(document:PlanningDocument):string {
  if (!document || document.schemaVersion!==1 || !Array.isArray(document.facts) || !Array.isArray(document.records)) throw new Error('Unsupported story-fact snapshot.');
  const facts=document.facts.filter(fact=>fact.status==='fact'&&fact.confirmed).map(fact=>({ id:fact.id,subjectId:fact.subjectId,property:fact.property,value:fact.value,eventId:fact.eventId,untilEventId:fact.untilEventId })).sort((a,b)=>a.id<b.id?-1:a.id>b.id?1:0);
  if (facts.length>1000 || facts.some(fact=>![fact.id,fact.subjectId,fact.property,fact.value].every(value=>typeof value==='string') || ![fact.eventId,fact.untilEventId].every(value=>value===null || typeof value==='string') || !/^[A-Za-z0-9._:-]{1,200}$/.test(fact.id) || !/^[A-Za-z0-9._:-]{1,200}$/.test(fact.subjectId))) throw new Error('Confirmed fact context exceeds its supported record limit.');
  const subjects=new Set(facts.map(fact=>fact.subjectId));
  const records=document.records.filter(record=>subjects.has(record.id)).map(record=>({ id:record.id,name:record.name })).sort((a,b)=>a.id<b.id?-1:a.id>b.id?1:0);
  if (new Set(facts.map(fact=>fact.id)).size!==facts.length || new Set(records.map(record=>record.id)).size!==records.length || records.some(record=>typeof record.id!=='string'||typeof record.name!=='string')) throw new Error('Invalid confirmed fact identities.');
  const text=canonicalJson({ schemaVersion:1,records,facts });
  if (new TextEncoder().encode(text).length>30000) throw new Error('Confirmed facts exceed the 30 KB context limit. Select a smaller saved scope before model review.');
  return text;
}
export async function canonicalFactSource(projectId:string,revisionId:string,document:PlanningDocument):Promise<SceneSnapshot> {
  if (!projectId || !revisionId) throw new Error('Fact context requires a saved planning revision.');
  const text=canonicalFactText(document);return { kind:'facts',sceneId:'planning:'+projectId,revisionId,text,hash:await sha256(text),title:'Confirmed story facts',language:'und' };
}
