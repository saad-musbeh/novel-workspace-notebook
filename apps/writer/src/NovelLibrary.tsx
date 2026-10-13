import React,{useLayoutEffect,useRef,useState} from 'react';
import type {Project} from './workspace';
import {unassignedNovels,type UniverseEntry} from './universe-library';
import './writing-desk.css';
export interface NovelLibraryProps {
 projects:readonly Project[];universes:readonly UniverseEntry[];error:string;
 onRefresh:()=>Promise<void>;onClose:()=>void;
 onCreateUniverse:(title:string)=>Promise<string>;onCreateSeries:(entry:UniverseEntry,title:string)=>Promise<string>;
 onCreateNovel:(entry:UniverseEntry,seriesId:string,title:string)=>Promise<void>;onAttachNovel:(entry:UniverseEntry,seriesId:string,projectId:string)=>Promise<void>;
 onOpenNovel:(projectId:string)=>Promise<void>;onOpenUniverse:(projectId:string)=>Promise<void>;
}
export function NovelLibrary({projects,universes,error:loadError,onRefresh,onClose,onCreateUniverse,onCreateSeries,onCreateNovel,onAttachNovel,onOpenNovel,onOpenUniverse}:NovelLibraryProps){
 const dialog=useRef<HTMLDialogElement>(null),working=useRef(false);
 const [page,setPage]=useState<'library'|'novel'|'series'|'universe'>('library'),[universeId,setUniverseId]=useState(universes[0]?.project.id??''),[seriesId,setSeriesId]=useState(''),[title,setTitle]=useState(''),[existing,setExisting]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useLayoutEffect(()=>{const node=dialog.current!;node.showModal();return()=>node.close();},[]);
 const universe=universes.find(entry=>entry.project.id===universeId),unassigned=unassignedNovels(projects,universes);
 function tab(next:typeof page){setPage(next);setTitle('');setError('');setExisting('');}
 async function act(action:()=>Promise<void>){if(working.current)return;working.current=true;setBusy(true);setError('');try{await action();}catch(reason){setError(String(reason));}finally{working.current=false;setBusy(false);}}
 async function submit(){
  if(page==='universe'){const id=await onCreateUniverse(title.trim());setUniverseId(id);tab('series');return;}
  if(!universe)throw new Error('Choose a universe.');
  if(page==='series'){const id=await onCreateSeries(universe,title.trim());setSeriesId(id);tab('novel');return;}
  const series=seriesId||universe.catalogue.series[0]?.id;if(!series)throw new Error('Create a series first.');
  if(existing)await onAttachNovel(universe,series,existing);else await onCreateNovel(universe,series,title.trim());
  onClose();
 }
 return <dialog ref={dialog} className="manuscript-dialog novel-library" aria-label="Novel library" onCancel={event=>{event.preventDefault();if(!working.current)onClose();}} onKeyDown={event=>event.stopPropagation()}>
  <header><h2>Library</h2><button disabled={busy} onClick={onClose} aria-label="Close library">×</button></header>
  <nav aria-label="Library actions">{(['library','universe','series','novel'] as const).map(id=><button key={id} disabled={busy} aria-pressed={page===id} onClick={()=>tab(id)}>{id==='library'?'Novels':'New '+id}</button>)}</nav>
  {(error||loadError)&&<p role="alert">{error||loadError} <button disabled={busy} onClick={()=>void act(onRefresh)}>Refresh library</button></p>}
  {page==='library'?<div className="novel-library-list">
   {!universes.length&&!unassigned.length&&<p>No novels yet.</p>}
   {universes.map(entry=><section key={entry.project.id}><div className="library-universe"><strong dir="auto">{entry.project.title}</strong><button disabled={busy} onClick={()=>void act(async()=>{await onOpenUniverse(entry.project.id);onClose();})}>World notes</button></div>
    {entry.catalogue.series.map(series=><div className="library-series" key={series.id}><h3 dir="auto">{series.title}</h3>{entry.catalogue.novels.filter(novel=>novel.seriesId===series.id).map(novel=>{const project=projects.find(project=>project.id===novel.projectId);return project?<button key={novel.projectId} disabled={busy} onClick={()=>void act(async()=>{await onOpenNovel(project.id);onClose();})} dir="auto">{project.title}</button>:<p key={novel.projectId}>Novel unavailable on this computer.</p>;})}{!entry.catalogue.novels.some(novel=>novel.seriesId===series.id)&&<p>No novels in this series.</p>}</div>)}
    {!entry.catalogue.series.length&&<p>No series yet.</p>}
    {entry.catalogue.novels.filter(novel=>novel.seriesId===null).map(novel=>{const project=projects.find(project=>project.id===novel.projectId);return project?<button key={project.id} disabled={busy} onClick={()=>void act(async()=>{await onOpenNovel(project.id);onClose();})}>{project.title}</button>:<p key={novel.projectId}>Novel unavailable on this computer.</p>;})}
   </section>)}
   {unassigned.length>0&&<section><h3>Unassigned novels</h3>{unassigned.map(project=><button key={project.id} disabled={busy} dir="auto" onClick={()=>void act(async()=>{await onOpenNovel(project.id);onClose();})}>{project.title}</button>)}</section>}
  </div>:<form onSubmit={event=>{event.preventDefault();void act(submit);}}>
   {page!=='universe'&&<label>Universe<select required disabled={busy} value={universe?.project.id??''} onChange={event=>{setUniverseId(event.target.value);setSeriesId('');}}><option value="" disabled>Choose a universe</option>{universes.map(entry=><option key={entry.project.id} value={entry.project.id}>{entry.project.title}</option>)}</select></label>}
   {page==='novel'&&<><label>Series<select required disabled={busy||!universe?.catalogue.series.length} value={seriesId||universe?.catalogue.series[0]?.id||''} onChange={event=>setSeriesId(event.target.value)}><option value="" disabled>Choose a series</option>{universe?.catalogue.series.map(series=><option key={series.id} value={series.id}>{series.title}</option>)}</select></label>{!universe?.catalogue.series.length&&<p>Create a universe and series before adding a novel.</p>}{unassigned.length>0&&<label>Novel<select disabled={busy} value={existing} onChange={event=>setExisting(event.target.value)}><option value="">Create a new novel</option>{unassigned.map(project=><option key={project.id} value={project.id}>{project.title}</option>)}</select></label>}</>}
   {!existing&&<label>{page==='universe'?'Universe name':page==='series'?'Series name':'Novel name'}<input required disabled={busy} maxLength={500} dir="auto" value={title} onChange={event=>setTitle(event.target.value)}/></label>}
   <div className="dialog-actions"><button type="button" disabled={busy} onClick={()=>tab('library')}>Cancel</button><button type="submit" disabled={busy||!!loadError||(!existing&&!title.trim())||(page==='novel'&&!universe?.catalogue.series.length)}>{busy?'Saving…':existing?'Add novel':'Create '+page}</button></div>
  </form>}
 </dialog>;
}
