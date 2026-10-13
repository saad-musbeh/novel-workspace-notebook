import { exportLocalJson } from '../assistance/export';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { validatePlanningDocument, type PlanningAdapter, type PlanningDocument } from './model';
import { PlanningSaver, type PlanningSaveStatus } from './saver';
import { StoryBible } from './StoryBible';
import { Board } from './Board';
import { Chronology } from './Chronology';
import { Issues } from './Issues';
import type { SceneReference } from './controls';
import {ReaderRehearsal} from './ReaderRehearsal';
import {RipplePreview} from './RipplePreview';
import {confirmPlanningRipple,previewPlanningRipple} from './ripple';
import './planning.css';
export interface PlanningWorkspaceProps {
  pageActions?:React.ReactNode;
  adapter: PlanningAdapter; projectId: string; scenes: SceneReference[]; onOpenScene?: (id:string)=>void;
  destination?: 'bible' | 'board' | 'chronology' | 'issues' | 'rehearsal';
  onRegisterFlush?: (flush:(()=>Promise<void>)|null)=>void;
}
export function PlanningWorkspace({pageActions,adapter,projectId,scenes,onOpenScene,destination='bible',onRegisterFlush}:PlanningWorkspaceProps) {
  const [document,setDocument]=useState<PlanningDocument|null>(null);const [error,setError]=useState('');const [loadError,setLoadError]=useState('');const [status,setStatus]=useState<PlanningSaveStatus>('saved');const [tab,setTab]=useState(destination);const [reload,setReload]=useState(0);const saver=useRef<PlanningSaver|null>(null);const statusRef=useRef(status);const composing=useRef(false);
  const [proposal,setProposal]=useState<{base:PlanningDocument;next:PlanningDocument;version:number;show:boolean}|null>(null),[applying,setApplying]=useState(false);
  const documentRef=useRef<PlanningDocument|null>(null),proposalRef=useRef(proposal),revisionRef=useRef<string|null>(null),version=useRef(0),epoch=useRef(0),applyingRef=useRef(false);
  const propose=(next:typeof proposal)=>{proposalRef.current=next;setProposal(next);};
  statusRef.current=status;
  useEffect(()=>setTab(destination),[destination]);
  useEffect(()=>{let active=true;epoch.current++;documentRef.current=null;propose(null);setDocument(null);setLoadError('');setError('');void adapter.load(projectId).then(snapshot=>{validatePlanningDocument(snapshot.document);if(!active)return;documentRef.current=snapshot.document;revisionRef.current=snapshot.revisionId;version.current++;setDocument(snapshot.document);setStatus('saved');const guardedAdapter:PlanningAdapter={load:adapter.load,async save(...args){const saved=await adapter.save(...args);if(active)revisionRef.current=saved.revisionId;return saved;}};saver.current=new PlanningSaver(guardedAdapter,projectId,snapshot.revisionId,(next,message)=>{if(active){setStatus(next);setError(message??'');}});}).catch(e=>{if(active)setLoadError(String(e));});return()=>{active=false;epoch.current++;saver.current?.dispose();saver.current=null;};},[adapter,projectId,reload]);
  const drain=useCallback(async()=>{if(loadError||!saver.current)return;if(composing.current)throw new Error('Finish entering the current word before leaving planning.');if(proposalRef.current)throw new Error('Apply or cancel the ripple preview before leaving planning.');await saver.current.drain();},[loadError]);
  useEffect(()=>{onRegisterFlush?.(drain);return()=>onRegisterFlush?.(null);},[onRegisterFlush,drain]);
  useEffect(()=>{const prevent=(e:BeforeUnloadEvent)=>{if(statusRef.current!=='saved'||proposalRef.current){e.preventDefault();e.returnValue='';}};window.addEventListener('beforeunload',prevent);return()=>window.removeEventListener('beforeunload',prevent);},[]);
  const change=(next:PlanningDocument)=>{try{
    if(applyingRef.current)throw new Error('Wait for the planning revision check.');
    const current=documentRef.current;if(!current||!saver.current)return;
    if(proposalRef.current){if(composing.current&&!proposalRef.current.show){propose({...proposalRef.current,next:structuredClone(next)});return;}throw new Error('Apply or cancel the current ripple preview first.');}
    const preview=previewPlanningRipple(current,next,scenes);
    if(preview.mutations.length){propose({base:structuredClone(current),next:structuredClone(next),version:version.current,show:!composing.current});setError('');return;}
    validatePlanningDocument(next);saver.current.change(next);documentRef.current=next;version.current++;setDocument(next);
  }catch(e){setError(String(e));}};
  const apply=async()=>{
    const captured=proposalRef.current,currentSaver=saver.current,currentEpoch=epoch.current;if(!captured||!currentSaver||applyingRef.current)return;
    applyingRef.current=true;setApplying(true);setError('');
    try{const next=await confirmPlanningRipple({base:captured.base,proposed:captured.next,drain:()=>currentSaver.drain(),read:()=>adapter.load(projectId),expectedRevision:()=>revisionRef.current,isCurrent:()=>epoch.current===currentEpoch&&proposalRef.current===captured&&version.current===captured.version,isComposing:()=>composing.current});
      currentSaver.change(next);documentRef.current=next;version.current++;setDocument(next);propose(null);
    }catch(e){if(epoch.current===currentEpoch)setError(String(e));}finally{applyingRef.current=false;if(epoch.current===currentEpoch)setApplying(false);}
  };
  const displayDocument=proposal&&!proposal.show?proposal.next:document;

  const exportDraft=()=>{if(!document)return;void exportLocalJson('planning-recovery.json',proposalRef.current?.next??document,'dictionary').catch(e=>setError(String(e)));};
  return <section className="planning-workspace" onCompositionStart={()=>{composing.current=true;saver.current?.composition(true);}} onCompositionEnd={()=>{composing.current=false;saver.current?.composition(false);if(proposalRef.current&&!proposalRef.current.show)propose({...proposalRef.current,show:true});}}>{!document&&pageActions}<div className="planning-heading"><div className="planning-save" role="status">{proposal?'Change awaiting review':status==='saved'?'Saved':status==='saving'?'Saving…':status==='error'?'Changes need attention':'Unsaved changes'}{status!=='saved'&&<button disabled={!document||!!proposal} onClick={()=>void (status==='error'&&saver.current?saver.current.retry():drain()).catch(e=>setError(String(e)))}>Save now</button>}</div></div>{error&&<div className="planning-error" role="alert"><p>{error}</p><p>Export a recovery copy before reloading to keep your unsaved edits.</p><button onClick={exportDraft}>Export recovery copy</button><button disabled={applying} onClick={()=>{if(window.confirm('Reload the saved planning revision? Unsaved planning edits will be discarded. Download a recovery copy first to preserve them.'))setReload(n=>n+1);}}>Reload saved revision</button></div>}{loadError?<div className="planning-error" role="alert"><h2>Could not open planning</h2><p>{loadError}</p><button onClick={()=>setReload(n=>n+1)}>Retry opening</button></div>:!document?<p role="status">Opening local planning…</p>:<><nav className="planning-tabs" aria-label="Planning destinations">{([['bible','Story bible'],['board','Board'],['chronology','Chronology'],['issues','Manual issues'],['rehearsal','Reader rehearsal']] as const).map(([id,label])=><button key={id} disabled={!!proposal} aria-pressed={tab===id} onClick={()=>setTab(id)}>{label}{id==='issues'&&<small>{document.issues.filter(i=>i.status==='open'||i.status==='investigating').length}</small>}</button>)}{pageActions}</nav><fieldset className="planning-panel planning-panel-fields" disabled={!!proposal?.show||applying}>{tab==='bible'&&<StoryBible document={displayDocument!} change={change} scenes={scenes} onOpenScene={onOpenScene}/>} {tab==='board'&&<Board document={displayDocument!} change={change} scenes={scenes} onOpenScene={onOpenScene}/>} {tab==='chronology'&&<Chronology document={displayDocument!} change={change} scenes={scenes} onOpenScene={onOpenScene}/>} {tab==='issues'&&<Issues document={displayDocument!} change={change} scenes={scenes} onOpenScene={onOpenScene}/>} {tab==='rehearsal'&&<ReaderRehearsal document={displayDocument!} scenes={scenes} onOpenScene={onOpenScene}/>}</fieldset></>}{proposal?.show&&<RipplePreview before={proposal.base} proposed={proposal.next} scenes={scenes} preview={previewPlanningRipple(proposal.base,proposal.next,scenes)} busy={applying} error={error} onPropose={next=>{if(!applyingRef.current)propose({...proposal,next:structuredClone(next)});}} onApply={()=>void apply()} onCancel={()=>{if(!applyingRef.current)propose(null);}}/>}</section>;
}
