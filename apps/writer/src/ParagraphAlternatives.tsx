import React,{useCallback,useEffect,useMemo,useRef,useState} from 'react';
import {ProjectDataCell} from './assistance/persistence';
import {workspaceCommand} from './workspace';
import {plainText,type DocumentNode} from './document';
import {alternativeWithText,emptyParagraphAlternatives,validateParagraphAlternatives,type ParagraphAlternativeGroup,type ParagraphAlternativesDocument,type ParagraphCapture} from './paragraph-alternatives';
import './paragraph-alternatives.css';
interface Receipt {capture:ParagraphCapture;revisionId:string}
export function ParagraphAlternatives({projectId,sceneId,capture,apply,onRegisterFlush,onBusy,onClose}:{projectId:string;sceneId:string;capture:()=>Promise<Receipt>;apply:(anchor:ParagraphCapture,replacement:DocumentNode,sceneRevisionId:string)=>Promise<Receipt>;onRegisterFlush:(flush:(()=>Promise<void>)|null)=>void;onBusy:(busy:boolean)=>void;onClose:()=>void}){
 const cell=useMemo(()=>new ProjectDataCell<ParagraphAlternativesDocument>(workspaceCommand,projectId,'paragraphAlternatives',emptyParagraphAlternatives()),[projectId]);
 const [document,setDocument]=useState<ParagraphAlternativesDocument|null>(null),[groupId,setGroupId]=useState(''),[versionId,setVersionId]=useState(''),[text,setText]=useState(''),[label,setLabel]=useState('Alternative'),[error,setError]=useState(''),[busy,setBusy]=useState(false),[status,setStatus]=useState('Opening alternatives…');
 const documentRef=useRef(document),draft=useRef({dirty:false,text:'',label:'Alternative'}),composing=useRef(false),active=useRef<Promise<void>|null>(null),loading=useRef<Promise<void>|null>(null),scope=useRef(sceneId);scope.current=sceneId;documentRef.current=document;
 const pendingDraft=useRef<{id:string;groupId:string;sourceId:string;text:string;label:string}|null>(null);
 const selected=useRef({groupId,versionId});selected.current={groupId,versionId};
 const group=document?.scenes.find(scene=>scene.sceneId===sceneId)?.groups.find(group=>group.id===groupId),version=group?.versions.find(version=>version.id===versionId);
 useEffect(()=>{let alive=true;const task=cell.load().then(value=>{validateParagraphAlternatives(value);if(alive){documentRef.current=value;setDocument(value);setStatus('Saved');}}).catch(reason=>{if(alive){setError(String(reason));setStatus('Could not open alternatives');}throw reason;});void task.catch(()=>{});loading.current=task;return()=>{alive=false;};},[cell]);
 const save=useCallback(async(candidate:ParagraphAlternativesDocument)=>{validateParagraphAlternatives(candidate);documentRef.current=candidate;setDocument(candidate);setStatus('Saving alternatives…');try{const saved=await cell.update(()=>structuredClone(candidate));documentRef.current=saved;setDocument(saved);setStatus('Saved');setError('');}catch(reason){setStatus('Alternatives not saved');setError(String(reason));throw reason;}},[cell]);
 const saveDraft=useCallback(async()=>{
  if(composing.current)throw new Error('Finish entering the alternative before leaving.');
  if(!draft.current.dirty)return;
  const value=documentRef.current,choice=selected.current,selectedGroup=value?.scenes.find(scene=>scene.sceneId===sceneId)?.groups.find(group=>group.id===choice.groupId),source=selectedGroup?.versions.find(version=>version.id===choice.versionId);
  if(!value||!selectedGroup||!source)throw new Error('Capture a paragraph before saving an alternative.');
  const pending=pendingDraft.current;
  if(pending&&pending.groupId===choice.groupId&&pending.sourceId===choice.versionId&&pending.text===draft.current.text&&pending.label===draft.current.label){
   await cell.retryFailed();const saved=cell.current();validateParagraphAlternatives(saved);documentRef.current=saved;setDocument(saved);pendingDraft.current=null;draft.current.dirty=false;setVersionId(pending.id);setStatus('Saved');setError('');return;
  }
  if(selectedGroup.versions.length>=20)throw new Error('This paragraph already has twenty saved versions. Capture another paragraph group to keep further alternatives.');
  const id=crypto.randomUUID(),block=alternativeWithText(source.block,draft.current.text);
  const next=structuredClone(value),target=next.scenes.find(scene=>scene.sceneId===sceneId)!.groups.find(group=>group.id===choice.groupId)!;
  target.versions.push({id,label:draft.current.label.trim()||'Alternative',block});
  pendingDraft.current={id,groupId:choice.groupId,sourceId:choice.versionId,text:draft.current.text,label:draft.current.label};
  await save(next);pendingDraft.current=null;draft.current.dirty=false;setVersionId(id);
 },[save,sceneId,cell]);
 const drain=useCallback(async()=>{await loading.current;await active.current;await saveDraft();await cell.drain();},[cell,saveDraft]);
 useEffect(()=>{onRegisterFlush(drain);return()=>onRegisterFlush(null);},[drain,onRegisterFlush]);
 function run(action:()=>Promise<void>){if(active.current)return;setBusy(true);onBusy(true);setError('');const task=action().catch(reason=>{setError(String(reason));throw reason;}).finally(()=>{active.current=null;setBusy(false);onBusy(false);});active.current=task;void task.catch(()=>{});}
 function choose(nextGroup:ParagraphAlternativeGroup,nextVersionId=nextGroup.activeId){const next=nextGroup.versions.find(version=>version.id===nextVersionId)!;setGroupId(nextGroup.id);setVersionId(next.id);setText(plainText(next.block));setLabel(next.id===nextGroup.originalId?'Alternative':next.label+' edited');draft.current={dirty:false,text:plainText(next.block),label:next.id===nextGroup.originalId?'Alternative':next.label+' edited'};}
 async function captureCurrent(){await loading.current;await saveDraft();const receipt=await capture();if(scope.current!==sceneId)throw new Error('The writing scene changed while capturing a paragraph.');
  const id=crypto.randomUUID(),originalId=crypto.randomUUID(),value=structuredClone(documentRef.current!);
  const nextGroup:ParagraphAlternativeGroup={id,label:'Paragraph '+(receipt.capture.blockIndex+1),anchor:receipt.capture,sceneRevisionId:receipt.revisionId,originalId,activeId:originalId,versions:[{id:originalId,label:'Original',block:structuredClone(receipt.capture.block)}]};
  let scene=value.scenes.find(scene=>scene.sceneId===sceneId);if(!scene){scene={sceneId,groups:[]};value.scenes.push(scene);}scene.groups.push(nextGroup);await save(value);choose(nextGroup);
 }
 async function swap(){await saveDraft();await cell.drain();const choice=selected.current,value=documentRef.current!,current=value.scenes.find(scene=>scene.sceneId===sceneId)?.groups.find(group=>group.id===choice.groupId),replacement=current?.versions.find(version=>version.id===choice.versionId);if(!current||!replacement)throw new Error('Choose a saved paragraph version.');
  const receipt=await apply(current.anchor,replacement.block,current.sceneRevisionId);if(scope.current!==sceneId)throw new Error('The writing scene changed during the swap.');
  const next=structuredClone(value),target=next.scenes.find(scene=>scene.sceneId===sceneId)!.groups.find(group=>group.id===current.id)!;target.anchor=receipt.capture;target.sceneRevisionId=receipt.revisionId;target.activeId=replacement.id;await save(next);
 }
 return <section className="paragraph-alternatives" aria-label="Paragraph alternatives" onCompositionStart={()=>{composing.current=true;}} onCompositionEnd={()=>{composing.current=false;}}>
  <header><h2>Paragraph alternatives</h2><button disabled={busy} onClick={()=>run(async()=>{await saveDraft();await cell.drain();onClose();})}>Close alternatives</button></header>
  <p>Keep versions of the paragraph at the caret. Swap a saved version into the draft; Undo restores the previous paragraph.</p>
  <button disabled={busy||!document} onClick={()=>run(captureCurrent)}>Capture current paragraph</button>
  {document&&<label>Captured paragraph<select aria-label="Captured paragraph" value={groupId} disabled={busy||draft.current.dirty} onChange={event=>{const next=document.scenes.find(scene=>scene.sceneId===sceneId)?.groups.find(group=>group.id===event.target.value);if(next)choose(next);}}><option value="">Choose a paragraph</option>{document.scenes.find(scene=>scene.sceneId===sceneId)?.groups.map(group=><option key={group.id} value={group.id}>{group.label}</option>)}</select></label>}
  {group&&version&&<><label>Saved version<select aria-label="Saved paragraph version" value={versionId} disabled={busy||draft.current.dirty} onChange={event=>choose(group,event.target.value)}>{group.versions.map(version=><option key={version.id} value={version.id}>{version.label}{version.id===group.activeId?' · last swapped':''}</option>)}</select></label><label>Name for new version<input maxLength={200} value={label} disabled={busy||group.versions.length>=20} onChange={event=>{setLabel(event.target.value);draft.current.label=event.target.value;draft.current.dirty=true;}}/></label><label>Alternative text<textarea dir="auto" aria-label="Alternative paragraph text" rows={6} maxLength={500000} disabled={busy||group.versions.length>=20} value={text} onChange={event=>{setText(event.target.value);draft.current.text=event.target.value;draft.current.dirty=true;}}/></label><p>Unchanged passages retain their formatting. New text inherits the formatting at the edit.</p><button disabled={busy||!draft.current.dirty} onClick={()=>run(saveDraft)}>Save as new alternative</button><button disabled={busy||draft.current.dirty} onClick={()=>run(swap)}>Swap saved version into draft</button></>}
  <span role="status">{draft.current.dirty?'Unsaved alternative':status}</span>{error&&<div role="alert"><p>{error}</p><button disabled={busy} onClick={()=>run(async()=>{await cell.retryFailed();const saved=cell.current();validateParagraphAlternatives(saved);documentRef.current=saved;setDocument(saved);const pending=pendingDraft.current;if(pending){setVersionId(pending.id);if(pending.text===draft.current.text&&pending.label===draft.current.label)draft.current.dirty=false;pendingDraft.current=null;}setStatus('Saved');setError('');})}>Retry alternatives save</button><p>Saved originals and other versions remain intact. If the scene changed, capture its paragraph again before swapping.</p></div>}
 </section>;
}
