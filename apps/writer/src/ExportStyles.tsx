import React,{useCallback,useEffect,useRef,useState} from 'react';
import {DEFAULT_EXPORT_PROFILE,EXPORT_PRESETS,loadExportProfile,saveExportProfile,validateExportProfile,type ExportProfile,type ExportPreset,type ExportStylePart} from './export-styles';
import './export-styles.css';

type Rpc=<T>(operation:string,args:Record<string,unknown>)=>Promise<T>;
type Props={projectId:string;rpc:Rpc;onRegisterFlush:(flush:(()=>Promise<void>)|null)=>void;onStatus:(profileRevisionId:string|null,ready:boolean,conflicted:boolean)=>void};
const labels:Record<ExportStylePart,string>={title:'Manuscript title',chapter:'Chapter title',scene:'Scene heading',body:'Body text'};

export function ExportStyles({projectId,rpc,onRegisterFlush,onStatus}:Props){
 const [profile,setProfile]=useState<ExportProfile>(DEFAULT_EXPORT_PROFILE),[ready,setReady]=useState(false),[conflicted,setConflicted]=useState(false),[error,setError]=useState(''),[status,setStatus]=useState(''),[saving,setSaving]=useState(false);
 const base=useRef<ExportProfile>(DEFAULT_EXPORT_PROFILE),draft=useRef(profile),revision=useRef<string|null>(null),dirty=useRef(false),loading=useRef(false),savingTask=useRef<{generation:number;promise:Promise<void>}|null>(null),generation=useRef(0),callbacks=useRef({rpc,onStatus});callbacks.current={rpc,onStatus};
 const publish=(value:ExportProfile)=>{draft.current=value;setProfile(value);dirty.current=JSON.stringify(value)!==JSON.stringify(base.current);setError('');setStatus('');let valid=true;try{validateExportProfile(value);}catch{valid=false;}callbacks.current.onStatus(revision.current,ready&&valid&&!conflicted,conflicted);};

 const reload=useCallback(async(discard=false)=>{
  if(dirty.current&&!discard&&!window.confirm('Discard unsaved Word export style changes and reload the saved version?'))return;
  const loadId=++generation.current;loading.current=true;setSaving(false);setReady(false);setConflicted(false);setError('');setStatus('');callbacks.current.onStatus(null,false,false);
  try{const result=await loadExportProfile(projectId,callbacks.current.rpc);if(loadId!==generation.current)return;base.current=result.snapshot.payload;draft.current=result.snapshot.payload;revision.current=result.snapshot.revisionId||null;dirty.current=false;setProfile(result.snapshot.payload);setConflicted(result.conflicted);setReady(true);callbacks.current.onStatus(revision.current,!result.conflicted,result.conflicted);}
  catch(reason){if(loadId===generation.current){setError(String(reason));setReady(false);callbacks.current.onStatus(null,false,false);}}
  finally{if(loadId===generation.current)loading.current=false;}
 },[projectId]);

 const flush=useCallback(async()=>{
  if(loading.current)throw new Error('Word export styles are still loading.');
  if(!dirty.current)return;
  if(conflicted)throw new Error('Resolve the competing Word export styles in Recovery before exporting.');
  if(!ready)throw new Error('Word export styles are not ready to save.');
  while(dirty.current){
   if(savingTask.current?.generation===generation.current){await savingTask.current.promise;continue;}
   const saveGeneration=generation.current;
   const value=structuredClone(draft.current),valueJson=JSON.stringify(value);
   validateExportProfile(value);
   const task=(async()=>{
    const saved=await saveExportProfile(projectId,value,revision.current,callbacks.current.rpc);
    if(saveGeneration!==generation.current)return;
    base.current=saved.payload;revision.current=saved.revisionId;
    if(JSON.stringify(draft.current)===valueJson){draft.current=saved.payload;dirty.current=false;setProfile(saved.payload);setStatus('Word export styles saved.');}
    else dirty.current=JSON.stringify(draft.current)!==JSON.stringify(base.current);
    callbacks.current.onStatus(revision.current,ready&&!conflicted,conflicted);
   })();
   const taskRecord={generation:saveGeneration,promise:task};savingTask.current=taskRecord;setSaving(true);
   try{await task;}catch(reason){if(saveGeneration!==generation.current)return;setError(String(reason));throw reason;}
   finally{if(savingTask.current===taskRecord)savingTask.current=null;if(saveGeneration===generation.current)setSaving(false);}
   if(saveGeneration!==generation.current)return;
  }
 },[projectId,ready,conflicted]);

 useEffect(()=>{void reload();return()=>{++generation.current;loading.current=false;onRegisterFlush(null);onStatus(null,false,false);};},[reload,onRegisterFlush,onStatus]);
 useEffect(()=>{onRegisterFlush(flush);return()=>onRegisterFlush(null);},[flush,onRegisterFlush]);
 const update=(edit:(current:ExportProfile)=>ExportProfile)=>publish(edit(structuredClone(draft.current)));
 const choose=(preset:ExportPreset)=>update(()=>structuredClone(EXPORT_PRESETS[preset]));
 const changeStyle=(part:ExportStylePart,key:'font'|'size'|'bold'|'italic',value:string|number|boolean)=>update(current=>({...current,[part]:{...current[part],[key]:value}}));
 let validation='';try{validateExportProfile(profile);}catch(reason){validation=String(reason);}
 return <details className="tool-section export-styles" aria-busy={!ready||loading.current}>
  <summary>Word export styles</summary>
  <p>Changes apply to the Word copy. Saved writing keeps its own formatting.</p>
  {error&&<p className="save-error" role="alert">{error}{conflicted?' Resolve the conflict in Recovery, then reload the project.':''}</p>}
  {conflicted&&<p role="status">Several saved style versions need a choice in Recovery. Reload this project after resolving them.</p>}
  {!ready?<p role="status">Loading saved Word export styles…</p>:<>
   <label>Starting layout<select aria-label="Starting Word layout" value="" disabled={saving||conflicted} onChange={event=>{if(event.target.value)choose(event.target.value as ExportPreset);}}><option value="" disabled>Choose a preset</option><option value="current">Default layout</option><option value="standard">Standard manuscript</option><option value="compact">Compact reading</option></select></label>
   <div className="export-page-style"><label>Paper<select value={profile.pageSize} disabled={saving||conflicted} onChange={event=>update(current=>({...current,pageSize:event.target.value}))}><option value="letter">Letter</option><option value="a4">A4</option></select></label><label>Margins (mm)<input aria-label="Page margins in millimeters" type="number" min="10" max="40" step="0.1" value={Number.isFinite(profile.marginMm)?profile.marginMm:''} disabled={saving||conflicted} onChange={event=>update(current=>({...current,marginMm:event.target.value===''?Number.NaN:Number(event.target.value)}))}/></label><label>Body line spacing<select value={profile.lineSpacing} disabled={saving||conflicted} onChange={event=>update(current=>({...current,lineSpacing:event.target.value}))}><option value="source">Keep source spacing</option><option value="1">Single</option><option value="1.15">1.15</option><option value="1.5">1.5</option><option value="2">Double</option></select></label></div>
   <div className="export-style-list">{(['title','chapter','scene','body'] as const).map(part=><fieldset key={part} disabled={saving||conflicted}><legend>{labels[part]}</legend><label>Font name<input value={profile[part].font} onChange={event=>changeStyle(part,'font',event.target.value)}/></label><label>Size (pt)<input type="number" min="8" max="72" step="0.5" value={Number.isFinite(profile[part].size)?profile[part].size:''} onChange={event=>changeStyle(part,'size',event.target.value===''?Number.NaN:Number(event.target.value))}/></label><label><input type="checkbox" checked={profile[part].bold} onChange={event=>changeStyle(part,'bold',event.target.checked)}/> Bold</label><label><input type="checkbox" checked={profile[part].italic} onChange={event=>changeStyle(part,'italic',event.target.checked)}/> Italic</label></fieldset>)}</div>
   {validation&&<p className="save-error" role="alert">{validation}</p>}{status&&<p role="status">{status}</p>}
   <button disabled={saving||conflicted||!dirty.current||Boolean(validation)} onClick={()=>void flush().catch(()=>{})}>Save Word export styles</button>
   <button disabled={saving||loading.current} onClick={()=>void reload().catch(reason=>setError(String(reason)))}>Reload saved styles</button>
  </>}
 </details>;
}
