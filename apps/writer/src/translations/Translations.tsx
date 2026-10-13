import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import {ProjectDataCell, validateSettingsPayload, type SettingsPayload} from '../assistance/persistence';
import {plainText} from '../document';
import {workspaceCommand, type SceneRecord} from '../workspace';
import {emptyTranslations, latestTranslations, translationLanguages, translationNeedsUpdate, validateTranslations, type MainLanguage, type TranslationDraft, type TranslationsDocument} from './model';
import './translations.css';

export interface TranslationsProps {
  projectId:string; sceneId:string; currentText:string; initialLanguage:string;
  capture:()=>Promise<SceneRecord>;
  onRegisterFlush:(flush:(()=>Promise<void>)|null)=>void;
  onClose:()=>void;
}
const message=(reason:unknown)=>reason instanceof Error?reason.message:String(reason);
const languageLabel=(code:string)=>translationLanguages.find(language=>language.code===code)?.label??code;

export function Translations({projectId,sceneId,currentText,initialLanguage,capture,onRegisterFlush,onClose}:TranslationsProps) {
  const cell=useMemo(()=>new ProjectDataCell<TranslationsDocument>(workspaceCommand,projectId,'sceneTranslations',emptyTranslations()),[projectId]);
  const [document,setDocument]=useState<TranslationsDocument|null>(null),[selectedId,setSelectedId]=useState('');
  const [sourceLanguage,setSourceLanguage]=useState<MainLanguage>(()=>translationLanguages.some(language=>language.code===initialLanguage)?initialLanguage as MainLanguage:'en');
  const [targetLanguage,setTargetLanguage]=useState<MainLanguage>(()=>initialLanguage==='ar'?'en':'ar');
  const [text,setText]=useState(''),[dirty,setDirty]=useState(false),[busy,setBusy]=useState(false),[translating,setTranslating]=useState(false);
  const [error,setError]=useState(''),[status,setStatus]=useState('Opening translations…'),[showOriginal,setShowOriginal]=useState(false);
  const documentRef=useRef<TranslationsDocument|null>(null),selection=useRef(''),draft=useRef({text:'',dirty:false}),composing=useRef(false);
  const loading=useRef<Promise<void>|null>(null),mutation=useRef<Promise<void>|null>(null),mounted=useRef(false),pending=useRef<TranslationsDocument|null>(null);
  const pendingChoice=useRef(''),runId=useRef<string|null>(null),scope=useRef({projectId,sceneId});scope.current={projectId,sceneId};
  const panelId=useRef('translations-'+crypto.randomUUID()).current;
  const selected=document?.drafts.find(value=>value.id===selectedId);
  const tabs=document?latestTranslations(document,sceneId):[];
  const blocked=busy||translating||Boolean(pending.current);

  function choose(value:TranslationDraft|undefined) {
    selection.current=value?.id??'';setSelectedId(selection.current);
    draft.current={text:value?.text??'',dirty:false};setText(draft.current.text);setDirty(false);
  }
  useEffect(()=>{
    mounted.current=true;let alive=true;
    const task=cell.load().then(value=>{validateTranslations(value);if(alive){documentRef.current=value;setDocument(value);setStatus('Saved');}}).catch(reason=>{if(alive){setError(message(reason));setStatus('Could not open translations');}throw reason;});
    loading.current=task;void task.catch(()=>{});
    return()=>{alive=false;mounted.current=false;const id=runId.current;runId.current=null;if(id)void workspaceCommand('provider_cancel',{jobId:id,runToken:id+':1'}).catch(()=>{});};
  },[cell]);

  const commit=useCallback(async(candidate:TranslationsDocument)=>{
    validateTranslations(candidate);pending.current=candidate;setStatus('Saving translation…');
    const saved=await cell.update(()=>structuredClone(candidate));validateTranslations(saved);
    pending.current=null;documentRef.current=saved;
    if(mounted.current){setDocument(saved);setStatus('Saved');setError('');}
    return saved;
  },[cell]);
  const saveDraft=useCallback(async()=>{
    if(composing.current)throw new Error('Finish entering the translation before saving or leaving.');
    if(pending.current)throw new Error('Retry the translation save before leaving. Your edits are kept here.');
    if(!draft.current.dirty)return;
    const value=documentRef.current;
    if(!value||!selection.current)throw new Error('Choose a translation before saving.');
    const candidate=structuredClone(value),target=candidate.drafts.find(value=>value.id===selection.current);
    if(!target)throw new Error('The selected translation is unavailable.');
    target.text=draft.current.text;pendingChoice.current=target.id;
    await commit(candidate);draft.current.dirty=false;if(mounted.current)setDirty(false);
  },[commit]);
  const cancelTranslation=useCallback(()=>{
    const id=runId.current;runId.current=null;
    if(mounted.current)setTranslating(false);
    if(id)void workspaceCommand('provider_cancel',{jobId:id,runToken:id+':1'}).catch(()=>{});
  },[]);
  const drain=useCallback(async()=>{
    cancelTranslation();await loading.current;await mutation.current;await saveDraft();await cell.drain();
  },[cell,saveDraft,cancelTranslation]);
  useEffect(()=>{onRegisterFlush(drain);return()=>onRegisterFlush(null);},[drain,onRegisterFlush]);
  useEffect(()=>{
    const preventClose=(event:BeforeUnloadEvent)=>{if(draft.current.dirty||pending.current){event.preventDefault();event.returnValue='';}};
    window.addEventListener('beforeunload',preventClose);return()=>window.removeEventListener('beforeunload',preventClose);
  },[]);

  function run(action:()=>Promise<void>) {
    if(mutation.current)return;
    setBusy(true);setError('');
    const task=action().catch(reason=>{if(mounted.current){setError(message(reason));setStatus('Translation not saved');}throw reason;}).finally(()=>{mutation.current=null;if(mounted.current)setBusy(false);});
    mutation.current=task;void task.catch(()=>{});
  }
  async function source() {
    await loading.current;await saveDraft();
    const record=await capture();
    if(scope.current.projectId!==projectId||scope.current.sceneId!==sceneId||record.id!==sceneId)throw new Error('The writing scene changed. Open translations again.');
    const sourceText=plainText(record.scene.document);
    if(!sourceText.trim())throw new Error('Write some original text before adding a translation.');
    if(sourceLanguage===targetLanguage)throw new Error('Choose a different translation language.');
    return {id:crypto.randomUUID(),sceneId,sceneRevisionId:record.revisionId,sourceLanguage,targetLanguage,sourceText,text:''} satisfies TranslationDraft;
  }
  async function add(value:TranslationDraft) {
    const candidate=structuredClone(documentRef.current!);candidate.drafts.push(value);pendingChoice.current=value.id;
    const saved=await commit(candidate);if(mounted.current)choose(saved.drafts.find(draft=>draft.id===value.id));
  }
  async function translate() {
    // Capture and save pending writing before sending an immutable local request.
    const value=await source();
    if(value.sourceText.length>12000)throw new Error('Local translation supports scenes up to 12,000 characters. You can still add and edit a translation yourself.');
    const settings=await workspaceCommand<{payload:SettingsPayload}|null>('load_project_data',{projectId,kind:'settings'});
    const provider=validateSettingsPayload(settings?.payload??{schemaVersion:1}).providers.find(provider=>provider.kind==='llm')!;
    const id=crypto.randomUUID();runId.current=id;setTranslating(true);setStatus('Translating on this PC…');
    // Inference is independent of the save/close barrier. Cancellation invalidates late output.
    void workspaceCommand<{text:string;provider:string}>('provider_translate',{projectId,sceneId,sceneRevisionId:value.sceneRevisionId,jobId:id,runToken:id+':1',endpoint:provider.endpoint,model:provider.model??null,sourceLanguage:value.sourceLanguage,targetLanguage:value.targetLanguage,text:value.sourceText,timeoutMs:120000}).then(async result=>{
      await mutation.current;
      if(!mounted.current||runId.current!==id)return;
      if(typeof result?.text!=='string'||!result.text.trim()||result.text.length>50000)throw new Error('The local model returned an invalid translation.');
      runId.current=null;setTranslating(false);run(async()=>{await add({...value,text:result.text});});
    }).catch(reason=>{
      if(!mounted.current||runId.current!==id)return;
      runId.current=null;setTranslating(false);setError(message(reason)+' You can add a translation yourself, or check the local model in Review.');setStatus('Translation unavailable');
    });
  }
  function pick(id:string) {run(async()=>{await saveDraft();choose(documentRef.current?.drafts.find(value=>value.id===id));});}
  async function retry() {
    await cell.retryFailed();const saved=cell.current();validateTranslations(saved);pending.current=null;documentRef.current=saved;setDocument(saved);
    choose(saved.drafts.find(value=>value.id===pendingChoice.current));setStatus('Saved');setError('');
  }

  return <section className="translations-panel" aria-label="Scene translations" onCompositionStart={()=>{composing.current=true;}} onCompositionEnd={()=>{composing.current=false;}}>
    <header><h2>Translations</h2><button disabled={busy} onClick={()=>run(async()=>{cancelTranslation();await loading.current;await saveDraft();await cell.drain();onClose();})}>Close translations</button></header>
    <p>Write in one language. Add another only when you need it, then adjust the wording freely.</p>
    <div className="translation-tabs" role="tablist" aria-label="Scene languages">
      <button type="button" role="tab" aria-selected={!selectedId} aria-controls={panelId+'-content'} id={panelId+'-original-tab'} disabled={blocked} onClick={()=>pick('')}>Original</button>
      {tabs.map(value=><button key={value.targetLanguage} type="button" role="tab" aria-selected={selected?.targetLanguage===value.targetLanguage} aria-controls={panelId+'-content'} id={panelId+'-'+value.id+'-tab'} disabled={blocked} onClick={()=>pick(value.id)}>{languageLabel(value.targetLanguage)}</button>)}
    </div>
    <div id={panelId+'-content'} role="tabpanel" aria-labelledby={selected?panelId+'-'+(tabs.find(value=>value.targetLanguage===selected.targetLanguage)?.id??selected.id)+'-tab':panelId+'-original-tab'}>
      {selected?<>
        {translationNeedsUpdate(selected,currentText)&&<p className="translation-stale" role="status">Translation may need updating. Your wording is kept. Use Translate to create a new draft from the current original.</p>}
        <div className="translation-options"><label>Saved draft<select aria-label="Saved translation draft" value={selectedId} disabled={blocked} onChange={event=>pick(event.target.value)}>{document!.drafts.filter(value=>value.sceneId===sceneId&&value.targetLanguage===selected.targetLanguage).map((value,index)=><option key={value.id} value={value.id}>Draft {index+1}</option>)}</select></label><label className="translation-compare"><input type="checkbox" checked={showOriginal} onChange={event=>setShowOriginal(event.target.checked)}/>Show original beside translation</label></div>
        <div className={'translation-editors'+(showOriginal?' compare':'')}>
          {showOriginal&&<label>Original used for this draft<textarea aria-label="Translation source text" readOnly dir="auto" value={selected.sourceText}/></label>}
          <label>{languageLabel(selected.targetLanguage)} translation<textarea aria-label="Translation text" dir={selected.targetLanguage==='ar'?'rtl':'ltr'} lang={selected.targetLanguage} value={text} maxLength={500000} disabled={blocked} onChange={event=>{draft.current={text:event.target.value,dirty:true};setText(event.target.value);setDirty(true);}}/></label>
        </div>
        <button disabled={blocked||!dirty} onClick={()=>run(saveDraft)}>Save translation</button>
      </>:<p className="translation-original" dir="auto">{currentText||'Your original scene is empty.'}</p>}
    </div>
    <div className="translation-add"><label>Original language<select aria-label="Original language" value={sourceLanguage} disabled={blocked} onChange={event=>{const next=event.target.value as MainLanguage;setSourceLanguage(next);if(next===targetLanguage)setTargetLanguage(translationLanguages.find(language=>language.code!==next)!.code);}}>{translationLanguages.map(language=><option key={language.code} value={language.code}>{language.label}</option>)}</select></label><label>Translate into<select aria-label="Translation language" value={targetLanguage} disabled={blocked} onChange={event=>setTargetLanguage(event.target.value as MainLanguage)}>{translationLanguages.filter(language=>language.code!==sourceLanguage).map(language=><option key={language.code} value={language.code}>{language.label}</option>)}</select></label><button disabled={blocked||!document} onClick={()=>run(translate)}>Translate</button><button disabled={blocked||!document} onClick={()=>run(async()=>{await add(await source());})}>Add translation myself</button></div>
    {translating&&<button onClick={()=>{cancelTranslation();setStatus('Translation cancelled');}}>Cancel translation</button>}
    <span role="status">{dirty?'Unsaved translation':status}</span>
    {error&&<div role="alert"><p>{error}</p>{pending.current&&<button disabled={busy} onClick={()=>run(retry)}>Retry translation save</button>}</div>}
  </section>;
}
