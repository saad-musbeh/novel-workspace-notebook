import React,{useEffect,useRef,useState} from 'react';
import {type Snapshot} from './document';
import {workspaceCommand,type Revision} from './workspace';
import {waitForTypingIdle} from './typing-priority';
import {changedParagraphs,revisionExcerpt,type ChangedParagraph} from './revision-changes';

export function RevisionChanges({sceneId,currentRevision,sharedVersion,held,open,prepare,onClose}:{sceneId:string;currentRevision:string|null;sharedVersion:number;held:boolean;open:boolean;prepare:()=>Promise<void>;onClose:()=>void}){
 const [history,setHistory]=useState<Revision[]>([]),[selected,setSelected]=useState(''),[changes,setChanges]=useState<ChangedParagraph[]>([]),[error,setError]=useState(''),[pending,setPending]=useState(false),[page,setPage]=useState(0),[titleChanged,setTitleChanged]=useState(false);
 const prepareRef=useRef(prepare);prepareRef.current=prepare;
 useEffect(()=>{let alive=true;if(!open&&!held)return;void workspaceCommand<Revision[]>('history',{sceneId}).then(rows=>{if(alive){setHistory(rows);setSelected(previous=>rows.some(row=>row.id===previous)?previous:rows[1]?.id??rows[0]?.id??'');}}).catch(reason=>{if(alive)setError(String(reason));});return()=>{alive=false;};},[sceneId,open,held,currentRevision,sharedVersion]);
 useEffect(()=>{
  let alive=true;const controller=new AbortController();if((!held&&!open)||!selected)return;setPending(true);setError('');setPage(0);
  void(async()=>{await waitForTypingIdle(controller.signal);await prepareRef.current();controller.signal.throwIfAborted();const before=await workspaceCommand<Snapshot>('load_revision',{sceneId,revisionId:selected}),after=await workspaceCommand<Snapshot>('load_scene',{sceneId});await waitForTypingIdle(controller.signal);if(alive){setChanges(changedParagraphs(before.scene.document,after.scene.document));setTitleChanged(before.scene.title!==after.scene.title);setPending(false);}})().catch(reason=>{if(alive){setError(String(reason));setPending(false);}});
  return()=>{alive=false;controller.abort();};
 },[sceneId,selected,held,open,currentRevision,sharedVersion]);
 if(!open&&!held)return null;
 return <aside className={'revision-changes'+(held?' revision-changes-held':'')} inert={held} aria-label="Changes since revision"><header><strong>Changes</strong>{!held&&<button onClick={onClose}>Close</button>}</header>{!held&&<><label>Compare with<select value={selected} onChange={event=>setSelected(event.target.value)}>{history.map(row=><option key={row.id} value={row.id}>{row.createdAt} · {row.operation}</option>)}</select></label><p>Hold the “Show revision changes” shortcut to compare saved paragraphs. Paragraph insertions can shift later comparisons.</p></>}{error&&<p role="alert">{error}</p>}{(held||open)&&(pending?<p role="status">Comparing…</p>:<><p role="status">{changes.length} changed paragraphs{titleChanged?' · Title changed':''}</p>{changes.slice(page*12,page*12+12).map(row=><section key={row.index}><strong>Paragraph {row.index+1}{row.formattingOnly?' · Formatting changed':''}</strong>{!row.formattingOnly&&<><div><small>Earlier</small><pre dir="auto">{revisionExcerpt(row.before).text}{revisionExcerpt(row.before).truncated?'…':''}</pre></div><div><small>Current</small><pre dir="auto"><mark>{revisionExcerpt(row.after).text}</mark>{revisionExcerpt(row.after).truncated?'…':''}</pre></div></>}</section>)}{changes.length>12&&<div><button disabled={page===0} onClick={()=>setPage(value=>value-1)}>Previous</button><span> {page*12+1}–{Math.min(changes.length,page*12+12)} </span><button disabled={(page+1)*12>=changes.length} onClick={()=>setPage(value=>value+1)}>Next</button></div>}</>)}
 </aside>;
}
