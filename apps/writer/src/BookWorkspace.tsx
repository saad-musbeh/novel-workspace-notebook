import React,{useCallback,useEffect,useLayoutEffect,useMemo,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {Writer} from './Writer';
import type {Snapshot} from './document';
import type {Repository} from './storage';
import {sceneRepository,workspaceBackgroundCommand,type ProjectIndex,type SceneRecord} from './workspace';
import type {PlanningDocument} from './planning/model';
import {selectedDefinition,type WritingSystemsDocument} from './writing-systems/model';
import {waitForTypingIdle} from './typing-priority';
import {BOOK_REVEAL_EVENT,BOOK_REVEAL_CANCEL_EVENT,BOOK_REVEAL_RESULT_EVENT,bookAnchor,bookAnchorTop,bookRows,characterSceneIds,initialBookFolds,manuscriptOrder,visibleBookRows,type BookAnchor,type BookRevealRequest} from './book-model';
import './book.css';
function nextLayout():Promise<void>{return new Promise(resolve=>{
  let frame:number|undefined;
  const finish=()=>{clearTimeout(timer);if(frame!==undefined)cancelAnimationFrame(frame);resolve();};
  const timer=setTimeout(finish,50);frame=requestAnimationFrame(finish);
});}

interface Slot {record:SceneRecord;initial:Snapshot;repository:Repository;flush:React.RefObject<(()=>Promise<void>)|null>;ready:boolean;measuredReady:boolean}
interface Props {pageActions?:React.ReactNode;index:ProjectIndex;planning:PlanningDocument|null;alphabets:WritingSystemsDocument;tabId:string;active:boolean;initialScrollTop?:number;initialPosition?:{bookSceneId?:string;bookSceneOffset?:number;bookCharacterId?:string;bookFoldedSceneIds?:string[]};onSaved:(record:SceneRecord)=>void;onReady:()=>void;onError:(message:string)=>void;onRegisterFlush:(flush:(()=>Promise<void>)|null)=>void}
export function BookWorkspace({pageActions,index,planning,alphabets,tabId,active,initialScrollTop=0,initialPosition,onSaved,onReady,onError,onRegisterFlush}:Props){
  const host=useRef<HTMLDivElement>(null),slots=useRef(new Map<string,Slot>()),heights=useRef(new Map<string,number>()),cardElements=useRef(new Map<string,HTMLElement>());
  const [version,setVersion]=useState(0),[heightVersion,setHeightVersion]=useState(0),[position,setPosition]=useState({top:initialScrollTop,height:800}),[character,setCharacter]=useState(initialPosition?.bookCharacterId??''),[error,setError]=useState(''),[changing,setChanging]=useState(false);
  const [paused,setPaused]=useState<Set<string>>(new Set()),[retry,setRetry]=useState(0);
  const [folded,setFolded]=useState(()=>initialBookFolds(initialPosition?.bookFoldedSceneIds,index.scenes));
  const foldedRef=useRef(folded);foldedRef.current=folded;
  const layoutAnchor=useRef<BookAnchor|null>(null),changingRef=useRef(false);
  const foldButtons=useRef(new Map<string,HTMLButtonElement>()),foldFocus=useRef<string|null>(null);
  const revealReply=useRef<{element:HTMLElement;requestId:string;sceneId?:string;characterId:string}|null>(null);
  const revealRestore=useRef<string|null>(null);
  function replyReveal(error?:string){const pending=revealReply.current;if(pending){revealReply.current=null;if(revealRestore.current===pending.requestId){if(error)restorePosition.current=null;revealRestore.current=null;}pending.element.dispatchEvent(new CustomEvent(BOOK_REVEAL_RESULT_EVENT,{detail:{requestId:pending.requestId,ok:!error,...(error?{error}:{})}}));}}
  const focused=useRef<string|null>(null),alive=useRef(true),transition=useRef<Promise<void>>(Promise.resolve()),requested=useRef(new Set<string>()),restoreScroll=useRef<number|null>(initialScrollTop),restorePosition=useRef(initialPosition?.bookSceneId?initialPosition:null),ready=useRef(false);
  const callbacks=useRef({onSaved,onReady,onError});callbacks.current={onSaved,onReady,onError};
  const ordered=useMemo(()=>manuscriptOrder(index),[index.chapters,index.scenes]);
  const navigationScope=useRef({planning,ordered});navigationScope.current={planning,ordered};
  const characters=planning?.records.filter(record=>record.kind==='character')??[];
  const cut=useMemo(()=>character&&planning?characterSceneIds(planning,ordered,character):null,[character,planning,ordered]);
  const scenes=useMemo(()=>cut?ordered.filter(scene=>cut.has(scene.id)):ordered,[cut,ordered]);
  const rows=useMemo(()=>bookRows(scenes,heights.current,folded),[scenes,heightVersion,folded]);
  const rowsRef=useRef(rows);rowsRef.current=rows;
  const total=rows.length?rows.at(-1)!.top+rows.at(-1)!.height:0;
  const visible=visibleBookRows(rows,position.top,position.height);
  useLayoutEffect(()=>{const id=foldFocus.current;if(!changing&&active&&id){foldButtons.current.get(id)?.focus({preventScroll:true});foldFocus.current=null;}},[folded,changing,active,version]);
  const visibleIds=new Set(visible.map(row=>row.scene.id));
  requested.current=new Set(visible.filter(row=>!folded.has(row.scene.id)).map(row=>row.scene.id));
  if(restorePosition.current?.bookSceneId&&!folded.has(restorePosition.current.bookSceneId)&&scenes.some(scene=>scene.id===restorePosition.current!.bookSceneId))requested.current.add(restorePosition.current.bookSceneId);
  if(focused.current&&!folded.has(focused.current)&&scenes.some(scene=>scene.id===focused.current))requested.current.add(focused.current);
  useLayoutEffect(()=>{
    const anchor=layoutAnchor.current,element=host.current;
    if(!anchor||!element||!active||element.clientHeight<=0||element.clientWidth<=0)return;
    const top=bookAnchorTop(rows,anchor);layoutAnchor.current=null;
    if(top!==null){element.scrollTop=top;setPosition({top:element.scrollTop,height:element.clientHeight});element.dispatchEvent(new Event('scroll'));}
  },[rows,active,position.height]);
  useLayoutEffect(()=>{
    const pending=restorePosition.current,element=host.current;
    if(!pending?.bookSceneId||!element||!active||element.clientHeight<=0||element.clientWidth<=0)return;
    const row=rows.find(row=>row.scene.id===pending.bookSceneId);
    if(!row){restorePosition.current=null;return;}
    if(folded.has(row.scene.id)||heights.current.has(row.scene.id)&&slots.current.get(row.scene.id)?.ready&&slots.current.get(row.scene.id)?.measuredReady&&row.height===heights.current.get(row.scene.id)){element.scrollTop=row.top+Math.min(pending.bookSceneOffset??0,Math.max(0,row.height-120));restorePosition.current=null;restoreScroll.current=null;setPosition({top:element.scrollTop,height:element.clientHeight});element.dispatchEvent(new Event('scroll'));replyReveal();}
    else if(element.scrollTop!==row.top){element.scrollTop=row.top;setPosition({top:element.scrollTop,height:element.clientHeight});}
  },[heightVersion,version,rows,active,position.height]);
  useLayoutEffect(()=>{
    const pending=revealReply.current,element=host.current;
    if(!pending||pending.sceneId||revealRestore.current!==pending.requestId||character!==pending.characterId||rows.length||!element||!active||element.clientHeight<=0||element.clientWidth<=0)return;
    element.scrollTop=0;setPosition({top:0,height:element.clientHeight});element.dispatchEvent(new Event('scroll'));replyReveal();
  },[character,rows,version,active,position.height]);
  const ids=[...requested.current].join(',');
  const fail=useCallback((reason:unknown)=>{const message=String(reason);setError(message);callbacks.current.onError(message);replyReveal(message);},[]);
  const flush=useCallback(async()=>{await transition.current;for(const slot of slots.current.values()){if(!slot.flush.current)throw new Error('A scene is still opening. Try again in a moment.');await slot.flush.current();}},[]);
  useEffect(()=>{onRegisterFlush(flush);return()=>onRegisterFlush(null);},[flush,onRegisterFlush]);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;replyReveal('The book closed before the scene finished opening.');};},[]);
  useLayoutEffect(()=>{
    const element=host.current;if(!element)return;
    if(!restorePosition.current&&restoreScroll.current!==null&&(restoreScroll.current===0||(active&&element.clientHeight>0&&element.clientWidth>0))){element.scrollTop=restoreScroll.current;restoreScroll.current=null;}
    const measure=()=>setPosition({top:element.scrollTop,height:element.clientHeight||800});
    const observer=new ResizeObserver(measure);observer.observe(element);measure();return()=>observer.disconnect();
  },[active]);
  useEffect(()=>{
    const element=host.current;if(!element)return;
    const reveal=(event:Event)=>{
      const request=(event as CustomEvent<BookRevealRequest>).detail;
      if(!request||typeof request.requestId!=='string'||request.sceneId!==undefined&&typeof request.sceneId!=='string')return;
      const refuse=(message:string)=>{element.dispatchEvent(new CustomEvent(BOOK_REVEAL_RESULT_EVENT,{detail:{requestId:request.requestId,ok:false,error:message}}));setError(message);};
      if(!request.requestId||request.requestId.length>300||request.sceneId!==undefined&&(!request.sceneId||request.sceneId.length>300)||request.characterId!==undefined&&(typeof request.characterId!=='string'||request.characterId.length>300)){refuse('The reading restoration request is invalid.');return;}
      if(changingRef.current||revealReply.current){refuse('A scene is still opening or changing. Try again after it finishes.');return;}
      const requestedCharacter=request.characterId??character;
      const selection=()=>{const {planning,ordered}=navigationScope.current;if(typeof requestedCharacter!=='string'||requestedCharacter&&!planning?.records.some(record=>record.id===requestedCharacter&&record.kind==='character'))throw new Error('The saved character cut is no longer available.');return requestedCharacter&&planning?characterSceneIds(planning,ordered,requestedCharacter):new Set(ordered.map(scene=>scene.id));};
      const includesTarget=(ids:Set<string>)=>request.sceneId?ids.has(request.sceneId):ids.size===0;
      try{if(!includesTarget(selection())){refuse('This scene is outside the requested book selection.');return;}}catch(reason){refuse(String(reason));return;}
      if(request.offset!==undefined&&(!Number.isFinite(request.offset)||request.offset<0||request.offset>100_000_000)){refuse('The scene position is invalid.');return;}
      changingRef.current=true;setChanging(true);setError('');const pending={element,requestId:request.requestId,sceneId:request.sceneId,characterId:requestedCharacter};revealReply.current=pending;
      const previousTransition=transition.current;
      const task=previousTransition.catch(()=>{}).then(async()=>{
        try{
          setPaused(new Set(slots.current.keys()));await nextLayout();
          for(const slot of slots.current.values()){if(!slot.flush.current)throw new Error('A scene is still opening. Try again in a moment.');await slot.flush.current();}
          if(!alive.current||revealReply.current!==pending)return;
          const nextIds=selection();if(!includesTarget(nextIds))throw new Error('This scene is outside the requested book selection.');
          const excluded=[...slots.current.keys()].filter(id=>!nextIds.has(id));
          for(const id of excluded){await invoke('editor_park',{sessionId:tabId+':book:'+id});if(!alive.current||revealReply.current!==pending)return;}
          if(!includesTarget(selection()))throw new Error('This scene is outside the requested book selection.');
          for(const id of excluded)slots.current.delete(id);
          setCharacter(requestedCharacter);
          const next=new Set(foldedRef.current);if(request.sceneId)next.delete(request.sceneId);foldedRef.current=next;setFolded(next);
          focused.current=request.sceneId??null;
          revealRestore.current=request.requestId;restorePosition.current=request.sceneId?{bookSceneId:request.sceneId,bookSceneOffset:request.offset??0}:null;restoreScroll.current=null;layoutAnchor.current=null;
          setVersion(value=>value+1);
        }catch(reason){if(alive.current)fail(reason);throw reason;}
        finally{changingRef.current=false;if(alive.current){setPaused(new Set());setChanging(false);}}
      });
      transition.current=task;void task.catch(()=>{});
    };
    const cancel=(event:Event)=>{const requestId=(event as CustomEvent).detail?.requestId;if(typeof requestId==='string'&&revealReply.current?.requestId===requestId)replyReveal('Reading restoration was cancelled.');};
    element.addEventListener(BOOK_REVEAL_EVENT,reveal);element.addEventListener(BOOK_REVEAL_CANCEL_EVENT,cancel);return()=>{element.removeEventListener(BOOK_REVEAL_EVENT,reveal);element.removeEventListener(BOOK_REVEAL_CANCEL_EVENT,cancel);};
  },[scenes,character,tabId,fail]);
  useEffect(()=>{
    const controller=new AbortController();
    transition.current=transition.current.catch(()=>{}).then(async()=>{
      if((!active&&ready.current)||controller.signal.aborted)return;
      await waitForTypingIdle(controller.signal);
      for(const [id,slot] of slots.current){
        if(requested.current.has(id))continue;
        // Pause before the save barrier. A failure keeps this exact mounted
        // draft and its recovery controls, rather than disposing its autosave.
        setPaused(previous=>new Set(previous).add(id));
        await nextLayout();
        try{
          if(!slot.flush.current)throw new Error('A scene is still opening. Scroll again after it opens.');
          await slot.flush.current();
          await invoke('editor_park',{sessionId:tabId+':book:'+id});
          slots.current.delete(id);if(alive.current)setVersion(value=>value+1);
        }catch(reason){if(alive.current){setPaused(previous=>{const next=new Set(previous);next.delete(id);return next;});fail(reason);}return;}
        if(controller.signal.aborted)return;
      }
      for(const id of requested.current){
        if(slots.current.has(id))continue;
        const record=await workspaceBackgroundCommand<SceneRecord>('load_project_scene',{projectId:index.project.id,sceneId:id},controller.signal);
        if(!alive.current||controller.signal.aborted||!requested.current.has(id))return;
        const base=sceneRepository(id),slot:Slot={record,initial:{scene:record.scene,revisionId:record.revisionId},flush:{current:null},repository:base,ready:false,measuredReady:false};
        slot.repository={...base,async save(scene,parent){const saved=await base.save(scene,parent);slot.record={...slot.record,scene:saved.scene,revisionId:saved.revisionId!};callbacks.current.onSaved(slot.record);return saved;}};
        slots.current.set(id,slot);setPaused(previous=>{const next=new Set(previous);next.delete(id);return next;});setVersion(value=>value+1);
        // Let each editor open before loading the next body.
        await nextLayout();
      }
      if(!ready.current&&!requested.current.size){ready.current=true;callbacks.current.onReady();}
    }).catch(reason=>{if(!controller.signal.aborted&&alive.current)fail(reason);});
    return()=>controller.abort();
  },[ids,active,index.project.id,tabId,fail,retry]);
  async function changeCut(value:string){
    if(changingRef.current)return;changingRef.current=true;
    setChanging(true);setError('');
    try{
      setPaused(new Set(slots.current.keys()));
      await nextLayout();
      await flush();
      const nextIds=value&&planning?characterSceneIds(planning,ordered,value):new Set(ordered.map(scene=>scene.id));
      for(const [id] of slots.current)if(!nextIds.has(id)){await invoke('editor_park',{sessionId:tabId+':book:'+id});slots.current.delete(id);}
      focused.current=null;setCharacter(value);setVersion(previous=>previous+1);
      if(host.current){host.current.scrollTop=0;setPosition({top:0,height:host.current.clientHeight});}
    }
    catch(reason){fail(reason);}finally{changingRef.current=false;setPaused(new Set());setChanging(false);}
  }
  function changeFold(id:string){
    if(changingRef.current)return;changingRef.current=true;setChanging(true);setError('');
    const previousTransition=transition.current;
    const task=previousTransition.catch(()=>{}).then(async()=>{
      const closing=!foldedRef.current.has(id),slot=slots.current.get(id);
      try{
        if(closing&&foldedRef.current.size>=20000)throw new Error('Up to 20,000 scenes can be folded in one tab. Unfold a scene first.');
        if(closing&&slot){
          setPaused(previous=>new Set(previous).add(id));await nextLayout();
          if(!slot.flush.current)throw new Error('A scene is still opening. Try again in a moment.');
          await slot.flush.current();
          await invoke('editor_park',{sessionId:tabId+':book:'+id});
          slots.current.delete(id);
        }
        if(!alive.current)return;
        layoutAnchor.current=bookAnchor(rowsRef.current,host.current?.scrollTop??position.top);
        if(focused.current===id)focused.current=null;
        const next=new Set(foldedRef.current);if(closing)next.add(id);else next.delete(id);
        foldedRef.current=next;setFolded(next);setVersion(value=>value+1);
      }catch(reason){if(alive.current)fail(reason);throw reason;}
      finally{changingRef.current=false;if(alive.current){foldFocus.current=id;setPaused(previous=>{const next=new Set(previous);next.delete(id);return next;});setChanging(false);}}
    });
    transition.current=task;void task.catch(()=>{});
  }
  function measureHeight(id:string,element:HTMLElement){
    if(foldedRef.current.has(id)||element.isConnected===false||element.clientWidth===0)return;
    if(heights.current.has(id)&&slots.current.get(id)?.measuredReady===false)return;
    const measured=Math.ceil(element.getBoundingClientRect().height)+8,previous=heights.current.get(id)??560;
    if(!Number.isFinite(measured)||measured<200||(heights.current.has(id)&&Math.abs(measured-previous)<2))return;
    const row=rowsRef.current.find(row=>row.scene.id===id);
    heights.current.set(id,measured);
    if(host.current&&row&&row.top+previous<=host.current.scrollTop)host.current.scrollTop+=measured-previous;
    setHeightVersion(value=>value+1);
  }
  function measureCard(id:string,element:HTMLElement|null){
    if(!element)return()=>{};
    cardElements.current.set(id,element);
    const measure=()=>measureHeight(id,element),observer=new ResizeObserver(measure);
    observer.observe(element);measure();return()=>{observer.disconnect();if(cardElements.current.get(id)===element)cardElements.current.delete(id);};
  }
  const chapters=new Map(index.chapters.map(chapter=>[chapter.id,(chapter.position+index.chapterNumberStart)+'. '+chapter.title]));
  return <section className="book-workspace" aria-label="Whole book" data-book-character={character} data-book-folded-scene-ids={JSON.stringify([...folded])} inert={changing} data-loaded-scenes={slots.current.size}>
    <div className="book-controls"><label>Read and write<select aria-label="Book selection" value={character} disabled={changing} onChange={event=>void changeCut(event.target.value)}><option value="">Whole book</option>{characters.map(record=><option key={record.id} value={record.id}>{record.name}</option>)}</select></label><span>{scenes.length} scenes</span>{character&&<span>Scenes linked to this character</span>}{pageActions}</div>
    {error&&<div role="alert" className="save-error">{error}<button onClick={()=>{setError('');setRetry(value=>value+1);}}>Try again</button></div>}
    {!scenes.length&&<p>{character?'Link this character to scenes in Planning to build their cut.':'Create a scene in Project to begin.'}</p>}
    <div className="book-scroll" ref={host} onScroll={()=>{const element=host.current;if(element){const row=rowsRef.current.find(row=>row.top<=element.scrollTop&&row.top+row.height>element.scrollTop);element.dataset.bookSceneId=row?.scene.id??'';element.dataset.bookSceneOffset=String(row?element.scrollTop-row.top:0);setPosition({top:element.scrollTop,height:element.clientHeight});}}}>
      <div className="book-pages" style={{height:total}}>
        {rows.filter(row=>visibleIds.has(row.scene.id)||requested.current.has(row.scene.id)||slots.current.has(row.scene.id)).map(row=>{
          const slot=slots.current.get(row.scene.id),isFolded=folded.has(row.scene.id),bodyId=tabId+':body:'+row.scene.id;
          return <div key={row.scene.id} className="book-position" style={{top:row.top}}><BookCard sceneId={row.scene.id} folded={isFolded} measure={measureCard} onFocus={()=>{if(!isFolded&&focused.current!==row.scene.id){focused.current=row.scene.id;setVersion(value=>value+1);}}}>
            <div className="book-scene-heading"><span className="book-chapter" dir="auto"><span>{chapters.get(row.scene.chapterId)}</span>{isFolded&&<strong dir="auto">{slot?.record.scene.title??row.scene.title}</strong>}</span><button ref={element=>{if(element)foldButtons.current.set(row.scene.id,element);else foldButtons.current.delete(row.scene.id);}} type="button" aria-expanded={!isFolded} aria-controls={bodyId} aria-label={(isFolded?'Unfold ':'Fold ')+(slot?.record.scene.title??row.scene.title)} onClick={()=>changeFold(row.scene.id)}>{isFolded?'Unfold':'Fold'}</button></div>
            <div id={bodyId} hidden={isFolded}>{slot?<Writer initial={slot.initial} repository={slot.repository} flushRef={slot.flush} onTitle={()=>{}} projectId={index.project.id} sceneId={slot.record.id} writingSystem={selectedDefinition(alphabets,slot.record.id)} performanceProbe={false} closeProbe={false} editorSessionId={tabId+':book:'+slot.record.id} tabActive={active&&!paused.has(slot.record.id)} keyboardActive={active&&focused.current===slot.record.id} externalScroll={host} onGeometry={()=>{const element=cardElements.current.get(slot.record.id);if(!element||element.isConnected===false||element.clientWidth===0)return;const first=!slot.measuredReady;slot.measuredReady=true;measureHeight(slot.record.id,element);if(first)setVersion(value=>value+1);}} onEditorReady={()=>{slot.ready=true;setVersion(value=>value+1);if(!ready.current&&[...requested.current].every(id=>slots.current.get(id)?.ready)){ready.current=true;callbacks.current.onReady();}}} onEditorError={fail}/>:!isFolded&&<div className="book-placeholder" style={{height:Math.max(0,row.height-46)}}><span dir="auto">{row.scene.title}</span><span role="status">Opening scene…</span></div>}</div>
          </BookCard></div>;
        })}
      </div>
    </div>
  </section>;
}
function BookCard({sceneId,folded,onFocus,measure,children}:{sceneId:string;folded:boolean;measure:(id:string,element:HTMLElement|null)=>()=>void;onFocus:()=>void;children:React.ReactNode}){
  const element=useRef<HTMLDivElement>(null),latest=useRef(measure);latest.current=measure;
  useLayoutEffect(()=>latest.current(sceneId,element.current),[sceneId]);
  return <div ref={element} className={'book-scene'+(folded?' book-folded':'')} data-book-scene={sceneId} onFocusCapture={onFocus} onPointerDownCapture={onFocus}>{children}</div>;
}
