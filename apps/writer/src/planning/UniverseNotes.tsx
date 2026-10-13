import React,{useMemo,useState} from 'react';
import type {PlanningDocument} from './model';
import {PagedPlanningList} from './controls';
import {referencePreview} from './scene-reference';
export function UniverseNotes({document,title,onEdit}:{document:PlanningDocument|null;title:string;onEdit:()=>void}){
 const [query,setQuery]=useState('');
 const records=useMemo(()=>document?.records.filter(record=>(record.name+' '+record.aliases.join(' ')).toLocaleLowerCase().includes(query.toLocaleLowerCase()))??[],[document,query]);
 return <section className="universe-notes" aria-label="Shared universe notes"><div className="planning-toolbar"><strong dir="auto">{title}</strong><button onClick={onEdit}>Edit world notes</button></div><input aria-label="Find universe record" placeholder="Find a person or place" value={query} onChange={event=>setQuery(event.target.value)}/>{!document?<p role="status">Opening world notes…</p>:<PagedPlanningList items={records} empty="No shared world records yet.">{record=><><strong dir="auto">{record.name}</strong><small>{record.kind}</small>{record.notes&&<p dir="auto">{referencePreview(record.notes)}</p>}{document.fields.filter(field=>field.recordKind===record.kind&&record.values[field.id]!==undefined).map(field=><p key={field.id} dir="auto"><strong>{field.label}: </strong>{referencePreview(typeof record.values[field.id]==='string'?String(record.values[field.id]):JSON.stringify(record.values[field.id]))}</p>)}{document.facts.filter(fact=>fact.subjectId===record.id).map(fact=><p key={fact.id} dir="auto"><strong>{fact.property}: </strong>{referencePreview(fact.value)} <small>{fact.confirmed?'Approved':fact.status}</small></p>)}</>}</PagedPlanningList>}</section>;
}
