import React, { useCallback, useEffect, useRef, useState } from 'react';
import {useCharacterCount} from './use-character-count';
import {dispatchShortcut} from './shortcuts';
import { Autosave, type SaveStatus } from './autosave';
import type { WritingSystemDefinition } from './writing-systems/model';
import { invoke } from '@tauri-apps/api/core';
import type { Repository } from './storage';
import { plainText, type LineSpacing, type Snapshot } from './document';
import {ParagraphAlternatives} from './ParagraphAlternatives';
import {Translations} from './translations/Translations';
import {workspaceCommand,type SceneRecord} from './workspace';
import type {ParagraphCapture} from './paragraph-alternatives';
import { NativeEditor, type EditorCommand, type EditorSearchResult, type NativeEditorHandle } from './NativeEditor';
import type {NavigationController} from './navigation-trail';
import {useCommandSequenceExecutor} from './use-command-sequences';
const native = '__TAURI_INTERNALS__' in window;
export function Writer({ initial, onTitle, performanceProbe, closeProbe, flushRef, repository, editorSessionId, tabActive=true, onEditorReady, onEditorError, projectId, sceneId, writingSystem, externalScroll, onGeometry, onRegisterNavigation, keyboardActive=tabActive, deskTools, deskActions }: { deskTools?:React.ReactNode; deskActions?:React.ReactNode; keyboardActive?:boolean; onRegisterNavigation?:(controller:NavigationController|null)=>void; onGeometry?:()=>void; externalScroll?:React.RefObject<HTMLElement|null>; editorSessionId?:string; tabActive?:boolean; onEditorReady?:()=>void; onEditorError?:(message:string)=>void; projectId?:string; sceneId?:string; writingSystem?:WritingSystemDefinition|null; initial: Snapshot; onTitle: (title: string) => void; performanceProbe: boolean; closeProbe: boolean; flushRef: React.RefObject<(() => Promise<void>) | null>; repository: Repository }) {
  const [title, setTitle] = useState(initial.scene.title);
  useEffect(()=>{const dismiss=(event:PointerEvent)=>{for(const menu of writerElement.current?.querySelectorAll<HTMLDetailsElement>('.writer-menu,.paragraph-options,.split-chooser')??[]){if(!menu.contains(event.target as Node))menu.open=false;}};document.addEventListener('pointerdown',dismiss);return()=>document.removeEventListener('pointerdown',dismiss);},[]);
  const titleRef = useRef(title);
  const documentRef = useRef(initial.scene.document);
  const [confirmedText, setConfirmedText] = useState(() => plainText(initial.scene.document));
  const count = useCharacterCount(confirmedText);
  const [status, setStatus] = useState<SaveStatus>(initial.revisionId ? 'saved' : 'unsaved');
  const [error, setError] = useState('');
  const [focus, setFocus] = useState(false);
  const [searchOpen,setSearchOpen]=useState(false),[replaceOpen,setReplaceOpen]=useState(false),[searchQuery,setSearchQuery]=useState(''),[replacement,setReplacement]=useState(''),[searchResult,setSearchResult]=useState<EditorSearchResult|null>(null);
  const findInput=useRef<HTMLInputElement>(null);
  function searchShortcut(action:string){if(action==='editor.findNext'){if(searchQuery)nativeEditor.current?.find(searchQuery);else setSearchOpen(true);return;}setSearchOpen(true);setReplaceOpen(action==='editor.replace');requestAnimationFrame(()=>{findInput.current?.focus();findInput.current?.select();});}
  const [paragraphStyle,setParagraphStyle]=useState<'paragraph'|'heading'>('paragraph'),[paragraphLanguage,setParagraphLanguage]=useState('en'),[paragraphDirection,setParagraphDirection]=useState<'auto'|'ltr'|'rtl'>('auto');
  const nativeEditor = useRef<NativeEditorHandle>(null);
  const alternativesFlush=useRef<(()=>Promise<void>)|null>(null),alternativesBusy=useRef(false),committedRevision=useRef(initial.revisionId);
  const [alternativesOpen,setAlternativesOpen]=useState(false),[alternativesWorking,setAlternativesWorking]=useState(false);
  const translationsFlush=useRef<(()=>Promise<void>)|null>(null);
  const [translationsOpen,setTranslationsOpen]=useState(false);
  const registerTranslationsFlush=useCallback((flush:(()=>Promise<void>)|null)=>{translationsFlush.current=flush;},[]);
  const sequenceActive=useRef(false),[sequencePlaying,setSequencePlaying]=useState(false);
  const restoreSequenceFocus=useRef(false);
  const sequenceContext=useRef({tabActive,keyboardActive});sequenceContext.current={tabActive,keyboardActive};
  const editorPending = useRef(false);
  const [editorBusy, setEditorBusy] = useState(false);
  const [editorReady, setEditorReady] = useState(false);
  const [editorError, setEditorError] = useState('');
  const saver = useRef<Autosave | null>(null);
  const [closing, setClosing] = useState(false);
  const closingRef = useRef(false); const titleComposing = useRef(false);
  const statusRef = useRef(status); statusRef.current = status;
  useEffect(() => {
    if (!native) return;
    const trackingRepository:Repository={...repository,async save(scene,parent){const saved=await repository.save(scene,parent);committedRevision.current=saved.revisionId;return saved;}};
    const controller = new Autosave(trackingRepository, initial.revisionId, (next, message) => { statusRef.current = next; setStatus(next); setError(message ?? ''); }); saver.current = controller;
    controller.composition(editorPending.current || titleComposing.current);
    if (!initial.revisionId) controller.change(initial.scene);
    const preventClose = (event: BeforeUnloadEvent) => { if (statusRef.current !== 'saved' || editorPending.current) { event.preventDefault(); event.returnValue = ''; } };
    const visibility = () => { if (document.visibilityState === 'hidden') void controller.flush(); };
    window.addEventListener('beforeunload', preventClose); document.addEventListener('visibilitychange', visibility);
    return () => { controller.dispose(); window.removeEventListener('beforeunload', preventClose); document.removeEventListener('visibilitychange', visibility); };
  }, [initial]);
  useEffect(() => {
    if (!native) return;
    flushRef.current = async () => {
      if(sequenceActive.current)throw new Error('Wait for the command sequence to finish before closing.');
      if (titleComposing.current) throw new Error('Finish entering the current word, then close again.');
      closingRef.current = true; setClosing(true);
      try {
        await translationsFlush.current?.();
        await alternativesFlush.current?.();
        await nativeEditor.current?.prepareClose();
        if (!saver.current) throw new Error('The writing area is still opening. Try closing again in a moment.');
        await saver.current.drain();

      } finally {
        closingRef.current = false; setClosing(false); nativeEditor.current?.resume();
      }
    };
    return () => { flushRef.current = null; };
  }, [flushRef]);
  const writerElement=useRef<HTMLDivElement>(null);
  const sequence=useCommandSequenceExecutor({
    async begin(){
      if(!native||!keyboardActive||!tabActive||translationsOpen||closingRef.current||alternativesBusy.current||titleComposing.current)throw new Error('Finish the current writing action before playing a sequence.');
      if(!nativeEditor.current||!saver.current)throw new Error('The writing area is still opening.');
      await nativeEditor.current.flush();await saver.current.drain();
      nativeEditor.current.lockSequenceInput(true);restoreSequenceFocus.current=true;sequenceActive.current=true;setSequencePlaying(true);
    },
    async flush(){
      if(titleComposing.current)throw new Error('Finish entering the scene title first.');
      if(!nativeEditor.current||!saver.current)throw new Error('The writing area is still opening.');
      await nativeEditor.current.flush();await saver.current.drain();
    },
    async execute(action){if(!sequenceContext.current.tabActive||!sequenceContext.current.keyboardActive||!nativeEditor.current)throw new Error('The active draft changed during the sequence.');await nativeEditor.current.executeSequenceAction(action);},
    end(){sequenceActive.current=false;nativeEditor.current?.lockSequenceInput(false);setSequencePlaying(false);}
  },native&&keyboardActive,writerElement);
  useEffect(()=>{if(!sequencePlaying&&restoreSequenceFocus.current){restoreSequenceFocus.current=false;if(sequenceContext.current.tabActive&&sequenceContext.current.keyboardActive)nativeEditor.current?.focus();}},[sequencePlaying]);

  useEffect(() => { const app=writerElement.current?.closest('.app');app?.classList.toggle('focus-mode',focus&&tabActive);return()=>app?.classList.remove('focus-mode'); }, [focus,tabActive]);
  useEffect(()=>{
    if(!native||!onRegisterNavigation)return;
    const readingElement=()=>externalScroll?.current??writerElement.current?.querySelector<HTMLElement>('.native-scroll');
    const controller:NavigationController={
      async capture(){
        if(titleComposing.current||alternativesBusy.current)throw new Error('Finish the current writing action before navigating.');
        if(!nativeEditor.current||!saver.current)throw new Error('The writing area is still opening.');
        await nativeEditor.current.flush();await saver.current.drain();
        const bookmark=await nativeEditor.current.captureNavigation();
        return {native:bookmark,reading:{scrollTop:readingElement()?.scrollTop??0}};
      },
      async restore(bookmark){
        if(titleComposing.current||alternativesBusy.current)throw new Error('Finish the current writing action before navigating.');
        if(!nativeEditor.current)throw new Error('The writing area is still opening.');
        if(bookmark.native)await nativeEditor.current.restoreNavigation(bookmark.native);
        const element=readingElement();if(element){element.scrollTop=bookmark.reading.scrollTop;element.dispatchEvent(new Event('scroll'));}
      }
    };
    onRegisterNavigation(controller);return()=>onRegisterNavigation(null);
  },[onRegisterNavigation,externalScroll]);
  function rename(value: string) { if (closingRef.current||alternativesBusy.current||sequenceActive.current) return; setTitle(value); onTitle(value); titleRef.current = value; saver.current?.change({ schemaVersion: 1, title: value, document: documentRef.current }); }
  async function exportText() {
    if (native) { try { await nativeEditor.current?.flush(); } catch (error) { setError(String(error)); return; } }
    const text = titleRef.current + '\n\n' + plainText(documentRef.current);
    if(native){try{await invoke('export_document',{format:'text',content:text,suggestedName:'scene.txt'});}catch(error){setError(String(error));}return;}
    const url = URL.createObjectURL(new Blob([text], { type: 'text/plain;charset=utf-8' }));
    const anchor = document.createElement('a'); anchor.href = url; anchor.download = 'scene.txt'; anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  function formatting(name: EditorCommand) {
    if (!translationsOpen&&!closingRef.current&&!alternativesBusy.current&&!sequenceActive.current) nativeEditor.current?.command(name);
  }
  async function prepareParagraphScene(expectedRevision?:string){
    if(!sceneId||!projectId||!nativeEditor.current||!saver.current)throw new Error('Open a saved scene to use paragraph alternatives.');
    if(titleComposing.current)throw new Error('Finish entering the scene title before using paragraph alternatives.');
    await nativeEditor.current.flush();await saver.current.drain();
    const head=await workspaceCommand<SceneRecord>('load_project_scene',{projectId,sceneId});
    if(head.id!==sceneId||!committedRevision.current||head.revisionId!==committedRevision.current||(expectedRevision!==undefined&&head.revisionId!==expectedRevision))throw new Error('This scene changed since the paragraph was captured. Refresh the scene and capture its paragraph again.');
    return head.revisionId;
  }
  async function captureParagraph(){const revisionId=await prepareParagraphScene();const capture=await nativeEditor.current!.captureParagraph();return {capture,revisionId};}
  async function applyParagraph(anchor:ParagraphCapture,replacement:import('./document').DocumentNode,expectedRevision:string){
    await prepareParagraphScene(expectedRevision);const capture=await nativeEditor.current!.swapParagraph(anchor,replacement);
    await nativeEditor.current!.flush();await saver.current!.drain();if(!committedRevision.current)throw new Error('The paragraph swap was not saved.');
    return {capture,revisionId:committedRevision.current};
  }
  async function captureTranslationSource(){
    if(!projectId||!sceneId||!nativeEditor.current||!saver.current)throw new Error('Open a saved scene to translate.');
    if(titleComposing.current||sequenceActive.current)throw new Error('Finish the current writing action before translating.');
    await nativeEditor.current.flush();await saver.current.drain();
    const record=await workspaceCommand<SceneRecord>('load_project_scene',{projectId,sceneId});
    if(record.id!==sceneId||!committedRevision.current||record.revisionId!==committedRevision.current)throw new Error('This scene changed in another window. Refresh it before translating.');
    return record;
  }
  async function toggleTranslations(){
    try{
      if(translationsOpen){await translationsFlush.current?.();setTranslationsOpen(false);}
      else {await alternativesFlush.current?.();setAlternativesOpen(false);setTranslationsOpen(true);}
    }catch(reason){setError(String(reason));}
  }
  const registerAlternativesFlush=useCallback((flush:(()=>Promise<void>)|null)=>{alternativesFlush.current=flush;},[]);
  const alternativesActivity=useCallback((busy:boolean)=>{alternativesBusy.current=busy;setAlternativesWorking(busy);},[]);
  useEffect(()=>{
    const toolbar=writerElement.current?.querySelector<HTMLElement>('.toolbar');
    const layout=writerElement.current?.closest<HTMLElement>('.writing-layout');
    if(!toolbar||!layout)return;
    const measure=()=>layout.style.setProperty('--editor-toolbar-height',toolbar.getBoundingClientRect().height+'px');
    measure();const observer=new ResizeObserver(measure);observer.observe(toolbar);
    return()=>observer.disconnect();
  },[]);
  const statuses = { unsaved: 'Unsaved changes', saving: 'Saving…', saved: native ? 'Saved' : 'Saved in this browser', error: error.startsWith('Conflict') ? 'Conflict' : 'Save failed' };
  return <div ref={writerElement} className="writer" inert={closing||sequencePlaying} onKeyDown={e=>{if(native&&tabActive&&!translationsOpen&&!closingRef.current&&!alternativesBusy.current)dispatchShortcut(e.nativeEvent,'editor',{'editor.find':()=>searchShortcut('editor.find'),'editor.replace':()=>searchShortcut('editor.replace')});}}>
    
    {!native && <div className="preview-note">Browser preview <span>Open the desktop app to edit this draft.</span></div>}
    <section className="editor-card">{native && <div className="toolbar" aria-label="Editor controls">
      <div className="editor-navigation">{deskActions}</div>
      <div className="format-tools" inert={translationsOpen}><button disabled={!editorReady || Boolean(editorError)} title="Bold" aria-label="Bold" onClick={() => formatting('bold')}><b>B</b></button><button disabled={!editorReady || Boolean(editorError)} title="Italic" aria-label="Italic" onClick={() => formatting('italic')}><i>I</i></button><button disabled={!editorReady || Boolean(editorError)} title="Underline" aria-label="Underline" onClick={() => formatting('underline')}><u>U</u></button><button disabled={!editorReady || Boolean(editorError)} title="Convert selected or preceding hexadecimal code to a Unicode character, or reverse it" aria-label="Convert Unicode code or character" onClick={()=>formatting('unicodeToggle')}>U+</button><span className="divider"/><button aria-label="Undo" disabled={!editorReady || Boolean(editorError)} onClick={() => formatting('undo')}>↶</button><button aria-label="Redo" disabled={!editorReady || Boolean(editorError)} onClick={() => formatting('redo')}>↷</button></div>
      <div className="paragraph-tools" inert={translationsOpen} aria-label="Paragraph direction"><button disabled={!editorReady || Boolean(editorError)} title="Set paragraph direction to left to right" onClick={() => formatting('ltr')}>Left to right</button><button disabled={!editorReady || Boolean(editorError)} title="Set paragraph direction to right to left" onClick={() => formatting('rtl')}>Right to left</button></div>
        {native&&<details inert={translationsOpen} className="paragraph-options" onKeyDown={event=>{if(event.key==='Escape'){event.preventDefault();event.currentTarget.open=false;event.currentTarget.querySelector<HTMLElement>('summary')?.focus();}}}><summary>Format</summary><div className="paragraph-settings"><label>Line spacing<select aria-label="Line spacing" value="" disabled={!editorReady||Boolean(editorError)} onChange={e=>{if(!closingRef.current)nativeEditor.current?.lineSpacing(Number(e.target.value) as LineSpacing);}}><option value="" disabled>Set spacing</option><option value="1">Single</option><option value="1.15">1.15</option><option value="1.5">1.5</option><option value="2">Double</option></select></label><label>Style<select value={paragraphStyle} onChange={e=>setParagraphStyle(e.target.value as 'paragraph'|'heading')}><option value="paragraph">Paragraph</option><option value="heading">Heading</option></select></label><label>Language<select value={paragraphLanguage} onChange={e=>setParagraphLanguage(e.target.value)}>{[['en','English'],['ar','Arabic'],['zh-Hans','Simplified Chinese'],['zh-Hant','Traditional Chinese'],['und','Unspecified']].map(([v,l])=><option key={v} value={v}>{l}</option>)}</select></label><label>Direction<select value={paragraphDirection} onChange={e=>setParagraphDirection(e.target.value as 'auto'|'ltr'|'rtl')}><option value="auto">Automatic</option><option value="ltr">Left to right</option><option value="rtl">Right to left</option></select></label><button disabled={!editorReady||Boolean(editorError)} onClick={()=>nativeEditor.current?.paragraph({style:paragraphStyle,language:paragraphLanguage,direction:paragraphDirection})}>Apply</button></div></details>}<div className="editor-everyday">{projectId&&sceneId&&<button disabled={!editorReady||editorBusy||Boolean(editorError)||alternativesWorking||sequencePlaying} aria-expanded={translationsOpen} onClick={()=>void toggleTranslations()}>Translate</button>}<button aria-label="Find in draft" disabled={translationsOpen||!editorReady||Boolean(editorError)} onClick={()=>searchShortcut('editor.find')}>Find</button><button onClick={()=>setFocus(!focus)} aria-pressed={focus}>{focus?'Exit focus':'Focus'}</button></div><footer className="editor-footer writer-status"><span className={'save-status ' + status} role="status"><span aria-hidden="true">{status === 'saved' ? '✓' : '●'}</span> {!native ? 'Read-only preview' : closing ? 'Saving before closing…' : editorError ? 'Editor stopped' : editorBusy ? (editorReady ? 'Unsaved changes' : 'Opening writing area…') : statuses[status]}</span><span data-character-count={count??'pending'}>{count===null?'Counting…':count<0?'Count unavailable':count.toLocaleString()+' characters'}</span></footer><details className="writer-menu" onClick={event=>{if((event.target as HTMLElement).closest('button'))event.currentTarget.open=false;}} onKeyDown={event=>{if(event.key==='Escape'){event.preventDefault();event.currentTarget.open=false;event.currentTarget.querySelector<HTMLElement>('summary')?.focus();}}}><summary aria-label="Writer menu" title="More writing tools">More</summary><div className="writer-menu-panel">{deskTools}{projectId&&sceneId&&<button disabled={!editorReady||editorBusy||Boolean(editorError)||alternativesWorking||translationsOpen} aria-expanded={alternativesOpen} onClick={()=>{if(alternativesOpen)void alternativesFlush.current?.().then(()=>setAlternativesOpen(false)).catch(reason=>setError(String(reason)));else setAlternativesOpen(true);}}>Paragraph alternatives</button>}<button onClick={exportText}>Export plain text</button><small>{count===null?'Counting…':count<0?'Count unavailable':count.toLocaleString()+' characters'}</small></div></details></div>}{native&&searchOpen&&<div className="editor-search" aria-label="Find and replace"><label>Find<input ref={findInput} aria-label="Find text" maxLength={1000} value={searchQuery} onChange={e=>{setSearchQuery(e.target.value);setSearchResult(null);}} onKeyDown={e=>{if(e.key==='Enter'&&searchQuery){e.preventDefault();nativeEditor.current?.find(searchQuery);}if(e.key==='Escape'){e.preventDefault();setSearchOpen(false);}}}/></label>{replaceOpen&&<label>Replace with<input maxLength={10000} value={replacement} onChange={e=>setReplacement(e.target.value)}/></label>}<button disabled={!editorReady||Boolean(editorError)||!searchQuery} onClick={()=>nativeEditor.current?.find(searchQuery)}>Find next</button>{replaceOpen&&<button disabled={!editorReady||Boolean(editorError)||!searchResult?.found||searchResult.replaced} onClick={()=>nativeEditor.current?.replace(searchQuery,replacement)}>Replace selection</button>}<button aria-label="Close find" onClick={()=>setSearchOpen(false)}>×</button>{searchResult&&<span role="status">{searchResult.message??(searchResult.replaced?'Replaced':!searchResult.found?'Not found':searchResult.wrapped?'Search restarted at the beginning':'Selected')}</span>}</div>}<div className="page" inert={translationsOpen}><input className="scene-title" dir="auto" aria-label="Scene title" maxLength={500} readOnly={!native||alternativesWorking||sequencePlaying} onCompositionStart={() => { titleComposing.current = true; saver.current?.composition(true); }} onCompositionEnd={() => { titleComposing.current = false; saver.current?.composition(editorPending.current); }} value={title} onChange={e => rename(e.target.value)}/><div className="title-rule"/>{native ? <NativeEditor ref={nativeEditor} onGeometry={onGeometry} externalScroll={externalScroll} keyboardActive={keyboardActive&&!translationsOpen} inputLocked={alternativesWorking||sequencePlaying} editorSessionId={editorSessionId} tabActive={tabActive} onReady={onEditorReady} projectId={projectId} writingSystem={writingSystem} performanceProbe={performanceProbe} closeProbe={closeProbe} initial={{ schemaVersion: 1, title, document: documentRef.current }}
      onShortcut={searchShortcut} onSearch={setSearchResult}
      onChange={document => { documentRef.current = document; setConfirmedText(plainText(document)); saver.current?.change({ schemaVersion: 1, title: titleRef.current, document }); }}
      onPending={active => { if(editorPending.current===active)return;editorPending.current = active; setEditorBusy(active); if (!active) setEditorReady(true); saver.current?.composition(active || titleComposing.current); }}
      onError={message => {setEditorError(message);onEditorError?.(message);}}/>
      : <div className="manuscript" aria-label="Saved browser draft"><p style={{ whiteSpace: 'pre-wrap' }} dir="auto">{confirmedText}</p></div>}</div>
</section>
    {sequence.error&&<div role="alert" className="save-error">{sequence.error}</div>}
    {error && <div role="alert" className="save-error">{error} <button onClick={() => saver.current?.retry()}>Retry save</button></div>}
    {native&&projectId&&sceneId&&translationsOpen&&<div className="translations-overlay"><Translations projectId={projectId} sceneId={sceneId} currentText={confirmedText} initialLanguage={String(initial.scene.document.content?.[0]?.attrs?.lang??'en')} capture={captureTranslationSource} onRegisterFlush={registerTranslationsFlush} onClose={()=>setTranslationsOpen(false)}/></div>}
    {native&&projectId&&sceneId&&alternativesOpen&&<div className="alternatives-overlay"><ParagraphAlternatives projectId={projectId} sceneId={sceneId} capture={captureParagraph} apply={applyParagraph} onRegisterFlush={registerAlternativesFlush} onBusy={alternativesActivity} onClose={()=>setAlternativesOpen(false)}/></div>}
  </div>;
}
