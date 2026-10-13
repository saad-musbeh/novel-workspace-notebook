import React, { forwardRef, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { plainText, type DocumentNode, type LineSpacing, type Scene } from './document';
import { mapInput, type WritingSystemDefinition } from './writing-systems/model';
import {observeEditorSize} from './editor-resize';
import { EditorQueue } from './editor-queue';
import type {NativeCaretBookmark} from './navigation-trail';
import type {ParagraphCapture} from './paragraph-alternatives';
import {noteTyping,setTypingPending,setTypingComposing,releaseTyping} from './typing-priority';
import {bookViewport} from './book-model';
import {nativeViewport,editorRequestViewport} from './editor-geometry';
import {matchShortcut,editorShortcutOperation,mouseClickCount} from './shortcuts';
import {commandRecordingGeneration,recordCommand,sequenceActionFromOperation,validateSequenceAction,type CommandSequenceAction} from './command-sequences';
export interface EditorSearchResult {found:boolean;wrapped?:boolean;replaced?:boolean;message?:string}
interface DeferredInput { deferred:true; timing:{processingMs:number;outputMs:number} }
interface Frame {
  navigationCapture?:NativeCaretBookmark;
  paragraphCapture?:ParagraphCapture;
  missingGlyphs?:number;
  unicodeResult?:{converted:boolean;message?:string};
  searchResult?:EditorSearchResult;
  document?: DocumentNode; image: string; width: number; height: number; totalHeight: number; top: number;
  timing?: { processingMs: number; outputMs: number };
  origin: number[]; cursor: number[]; selection: number[][]; selectedText: string; states: Record<string, string>;
}
export type EditorCommand = 'bold' | 'italic' | 'underline' | 'clearFormatting' | 'unicodeToggle' | 'undo' | 'redo' | 'ltr' | 'rtl';
export interface NativeEditorHandle { focus():void; lockSequenceInput(locked:boolean):void; executeSequenceAction(action:CommandSequenceAction):Promise<void>; captureNavigation():Promise<NativeCaretBookmark>;restoreNavigation(bookmark:NativeCaretBookmark):Promise<void>; captureParagraph():Promise<ParagraphCapture>; swapParagraph(anchor:ParagraphCapture,replacement:DocumentNode):Promise<ParagraphCapture>; command(name: EditorCommand): void; lineSpacing(value: LineSpacing): void; find(query:string):void; replace(query:string,replacement:string):void; paragraph(options:{style:'paragraph'|'heading';language:string;direction:'auto'|'ltr'|'rtl'}):void; flush(): Promise<void>; prepareClose(): Promise<void>; resume(): void }
interface Props { keyboardActive?:boolean; onGeometry?:()=>void; externalScroll?:React.RefObject<HTMLElement|null>; inputLocked?:boolean; editorSessionId?:string; tabActive?:boolean; onReady?:()=>void; projectId?:string; writingSystem?:WritingSystemDefinition|null; performanceProbe?: boolean; closeProbe?: boolean; initial: Scene; onSearch?(result:EditorSearchResult|null):void; onShortcut?(action:string):void; onChange(document: DocumentNode): void; onPending(active: boolean): void; onError(message: string): void }
export const NativeEditor = forwardRef<NativeEditorHandle, Props>(function NativeEditor({ keyboardActive, onGeometry, externalScroll, inputLocked=false, editorSessionId, tabActive=true, onReady, projectId, writingSystem, performanceProbe, closeProbe, initial, onChange, onPending, onError, onSearch, onShortcut }, ref) {
  const [frame, setFrame] = useState<Frame | null>(null);
  const [error, setError] = useState('');
  const [unicodeStatus,setUnicodeStatus]=useState('');
  const [pendingText, setPendingText] = useState('');
  const [clipboardMessage,setClipboardMessage]=useState('');
  const [contextMenu,setContextMenu]=useState<{x:number;y:number}|null>(null);
  const contextRef=useRef<HTMLDivElement>(null);
  const [surfaceWidth, setSurfaceWidth] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const viewportVersion = useRef(0);
  const manualScrollVersion = useRef(0);
  const observedScrollTop = useRef(0);
  const displayedViewportVersion = useRef(0);
  const layoutWidth = useRef(0);
  const layoutHeight = useRef(0);
  const observedWidth = useRef(0);
  const observedHeight = useRef(0);
  const measureSize=useRef<()=>void>(()=>{});
  const scrollAnchor = useRef(0);
  const resizeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const container = useRef<HTMLDivElement>(null);
  const sink = useRef<HTMLTextAreaElement>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const serial = useRef<Promise<void>>(Promise.resolve());
  const queue = useRef(new EditorQueue()); const draining = useRef(false);
  const dirty = useRef(false); const epoch = useRef(0);
  const checkpointTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const checkpointDeadline = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const failed = useRef(false); const count = useRef(0); const composing = useRef(false);
  const failureReason=useRef('');
  const requestResults=useRef(new Map<number,{resolve:(frame:Frame)=>void;reject:(error:unknown)=>void}>());
  const acceptedSequenceCommands=useRef(new Map<number,{action:CommandSequenceAction;generation:number}>());
  const pending = useRef(new Map<number, string>()); const id = useRef(0);
  const frameRef = useRef<Frame | null>(null);
  const followPaint=useRef(false);
  const geometryCallback=useRef(onGeometry);geometryCallback.current=onGeometry;
  const lastDocument = useRef(JSON.stringify(initial.document));
  const callbacks = useRef({ onChange, onPending, onError, onSearch, onShortcut }); callbacks.current = { onChange, onPending, onError, onSearch, onShortcut };
  const alive = useRef(true);
  const paused = useRef(!tabActive);
  const sequenceInputLocked=useRef(false);
  const inputBlocked=()=>inputLockedRef.current||sequenceInputLocked.current;
  const inputLockedRef=useRef(inputLocked);inputLockedRef.current=inputLocked;
  const activeRef=useRef(tabActive);activeRef.current=tabActive;
  const readyCallback=useRef(onReady);readyCallback.current=onReady;
  const readyNotified=useRef(false);
  useEffect(()=>{if(frame&&!readyNotified.current){readyNotified.current=true;readyCallback.current?.();}},[Boolean(frame)]);
  useLayoutEffect(()=>{paused.current=!tabActive;if(sink.current)sink.current.readOnly=!tabActive||((inputLocked||sequenceInputLocked.current)&&!composing.current);if(tabActive)measureSize.current();},[tabActive,inputLocked]);
  const probeStarted = useRef(false);
  const probeTiming = useRef({ invokeMs: 0, syncMs: 0, processingMs: 0, outputMs: 0, handlerMs: 0 });
  const probeRequests = useRef(0);
  function accept(next: Frame, render = true) {
    if (!alive.current) return;
    frameRef.current = next; if (render) setFrame(next);
    if(next.unicodeResult)setUnicodeStatus(next.unicodeResult.message??(next.unicodeResult.converted?'Unicode converted.':'Select a character or hexadecimal code, then try again.'));
    callbacks.current.onSearch?.(next.searchResult??null);
    if (next.document) {
      const document = JSON.stringify(next.document);
      if (document !== lastDocument.current) { lastDocument.current = document; callbacks.current.onChange(next.document); }
    }
  }
  function liveViewport(pointerWidth?:number){
    const current=frameRef.current;
    const width=surface.current?.clientWidth??0,nativeWidth=current?.width??640;
    const host=externalScroll?.current;
    if(host&&surface.current){
      const surfaceRect=surface.current.getBoundingClientRect(),hostRect=host.getBoundingClientRect();
      const visible=bookViewport(surfaceRect,{top:hostRect.top+host.clientTop,height:host.clientHeight});
      const scale=width/nativeWidth;
      const top=Math.min(visible.top,Math.max(0,(current?.totalHeight??0)*scale-120));
      return nativeViewport(width,nativeWidth,top,visible.height,scrollAnchor.current,pointerWidth);
    }
    return nativeViewport(width,nativeWidth,scroll.current?.scrollTop??0,scroll.current?(parseFloat(getComputedStyle(scroll.current).maxHeight)||scroll.current.clientHeight):620,scrollAnchor.current,pointerWidth);
  }
  useEffect(()=>{
    const host=externalScroll?.current;if(!host)return;
    const changed=()=>{if(readManualScroll()&&activeRef.current&&resizeTimer.current===undefined)operation({type:'refresh'});};
    host.addEventListener('scroll',changed,{passive:true});return()=>host.removeEventListener('scroll',changed);
  },[externalScroll,Boolean(frame)]);
  useLayoutEffect(() => {
    if (!frame || !scroll.current || !surface.current) return;
    const width=surface.current.clientWidth,scale=width/frame.width;
    const height=externalScroll?.current?.clientHeight??scroll.current.clientHeight;
    if(width<=0||height<=0)return;
    if(externalScroll?.current){
      const host=externalScroll.current,rect=surface.current.getBoundingClientRect(),hostRect=host.getBoundingClientRect();
      const resized=layoutWidth.current>0&&layoutWidth.current!==width;
      if(frame.width===Math.max(240,Math.min(4096,Math.round(width)))){
        layoutWidth.current=width;layoutHeight.current=height;
        if(displayedViewportVersion.current===viewportVersion.current&&(followPaint.current||resized)){
          const top=Math.max(0,hostRect.top+host.clientTop-rect.top);
          if(top>0||frame.top>0)host.scrollTop+=frame.top*scale-top;
          if(followPaint.current&&frame.cursor.length){
            const updated=surface.current.getBoundingClientRect(),cursorTop=updated.top+(frame.cursor[1]-frame.origin[1])/15*scale,cursorBottom=cursorTop+frame.cursor[3]/15*scale;
            if(cursorTop<hostRect.top+4)host.scrollTop-=hostRect.top+4-cursorTop;
            else if(cursorBottom>hostRect.top+height-4)host.scrollTop+=cursorBottom-(hostRect.top+height-4);
          }
        }
      }
      const viewport=liveViewport();observedScrollTop.current=viewport.top;scrollAnchor.current=viewport.top;followPaint.current=false;
      if(tabActive&&frame.width===Math.max(240,Math.min(4096,Math.round(width))))geometryCallback.current?.();
      return;
    }
    const resized=layoutWidth.current>0&&(layoutWidth.current!==width||layoutHeight.current!==height);
    layoutWidth.current=width;layoutHeight.current=height;
    if (!resized&&displayedViewportVersion.current !== viewportVersion.current) return;
    scroll.current.scrollTop=(resized?scrollAnchor.current:frame.top)*scale;
    // The browser may clamp the requested position at the document's end.
    observedScrollTop.current=scroll.current.scrollTop;
    scrollAnchor.current=scroll.current.scrollTop/scale;
  }, [frame,surfaceWidth,viewportHeight,tabActive]);

  function readManualScroll() {
    const element=scroll.current,current=frameRef.current;
    if(externalScroll?.current&&current&&surface.current){
      const top=liveViewport().top;if(Math.abs(top-observedScrollTop.current)<=1)return false;
      observedScrollTop.current=top;viewportVersion.current++;manualScrollVersion.current++;scrollAnchor.current=top;return true;
    }
    if(!surface.current?.clientWidth||!element?.clientHeight)return false;
    if(!element||!current||Math.abs(element.scrollTop-observedScrollTop.current)<=1)return false;
    observedScrollTop.current=element.scrollTop;
    viewportVersion.current++;manualScrollVersion.current++;
    scrollAnchor.current=element.scrollTop/((surface.current?.clientWidth||current.width)/current.width);
    return true;
  }

  function problem(message: string) { failed.current = true; failureReason.current=message; setError(message); callbacks.current.onError(message); }
  function checkpoint() {
    clearTimeout(checkpointTimer.current); clearTimeout(checkpointDeadline.current);
    checkpointTimer.current = undefined; checkpointDeadline.current = undefined;
    if (dirty.current && !failed.current) operation({ type: 'snapshot' });
  }
  function scheduleCheckpoint() {
    clearTimeout(checkpointTimer.current);
    checkpointTimer.current = setTimeout(checkpoint, 300);
    checkpointDeadline.current ??= setTimeout(checkpoint, 2000);
  }
  function operation(value: Record<string, unknown>, edited = false, record = false) {
    const handlerStart = performance.now();
    if (!alive.current || failed.current || !frameRef.current || (paused.current && value.type !== 'snapshot'&&!['captureNavigation','restoreNavigation'].includes(String(value.command)))) return;
    readManualScroll();
    if(['text','key','command'].includes(String(value.type))&&!['captureNavigation','restoreNavigation'].includes(String(value.command)))value={...value,followCursorGeneration:manualScrollVersion.current};
    const ticket = ++id.current;
    if (value.type === 'text') pending.current.set(ticket, String(value.text));
    count.current++;
    if (edited) { dirty.current = true; epoch.current++; callbacks.current.onPending(true); scheduleCheckpoint(); }
    if(edited)noteTyping(queue.current);
    setTypingPending(queue.current,true);
    if(!queue.current.push(value, ticket, edited, epoch.current)){
      setPendingText([...pending.current.values()].join(''));
      setTypingPending(queue.current,false);
      problem('Typing is arriving faster than the editor can confirm. Your unconfirmed input is shown below; copy it before reopening this scene.');
      return;
    }
    if(record){const action=sequenceActionFromOperation(value),generation=commandRecordingGeneration();if(action&&generation!==null)acceptedSequenceCommands.current.set(ticket,{action,generation});}
    probeTiming.current.handlerMs = performance.now() - handlerStart;
    if (draining.current) return ticket;
    draining.current = true;
    serial.current = serial.current.then(async () => {
      try {
        while (alive.current && !failed.current) {
          const entry = queue.current.shift(); if (!entry) break;
          try {
            readManualScroll();
            const current = frameRef.current!;
            const viewport=liveViewport();
            const start = performance.now();
            const version=viewportVersion.current,manualVersion=manualScrollVersion.current;
            const {followCursorGeneration,...payload}=entry.operation;
            const {width,top,viewportHeight}=editorRequestViewport(payload,viewport);
            const followCursor=followCursorGeneration===manualVersion;
            const deferRender=payload.type==='text'&&payload.inputBoundary===undefined&&queue.current.nextIsOrdinaryText;
            const next = await invoke<Frame|DeferredInput>('editor_edit', { sessionId:editorSessionId??null, operation: { ...payload, width,top,viewportHeight,followCursor,deferRender, preview: payload.type !== 'snapshot' } });
            const returned = performance.now(); if (performanceProbe) probeRequests.current++;
            // Scroll events can arrive after a queued operation has started.
            if(readManualScroll())operation({type:'refresh'});
            if('deferred' in next){
              probeTiming.current={invokeMs:returned-start,syncMs:performance.now()-returned,processingMs:next.timing.processingMs,outputMs:0,handlerMs:probeTiming.current.handlerMs};
              continue;
            }
            const currentViewport=version===viewportVersion.current;
            if(currentViewport)displayedViewportVersion.current=version;
            // Retry a resize-invalidated caret paint without replaying the edit.
            // Any manual scroll since this request takes precedence.
            if(!currentViewport&&followCursor&&manualVersion===manualScrollVersion.current)operation({type:'refresh',followCursorGeneration:manualVersion});
            followPaint.current=currentViewport&&followCursor;
            accept(next, currentViewport && !(entry.operation.type==='text'&&queue.current.nextIsOrdinaryText) && (entry.operation.type !== 'snapshot' || !queue.current.length));
            probeTiming.current = { invokeMs: returned - start, syncMs: performance.now() - returned, processingMs: next.timing?.processingMs ?? 0, outputMs: next.timing?.outputMs ?? 0, handlerMs: probeTiming.current.handlerMs };
            if (entry.operation.type === 'snapshot') {
              if (!next.document) throw new Error('The editor checkpoint did not include the scene.');
              for (const ticket of pending.current.keys()) if (ticket <= entry.tickets.at(-1)!) pending.current.delete(ticket);
              if (entry.epoch === epoch.current) dirty.current = false;
            }
            for(const ticket of entry.tickets){const recorded=acceptedSequenceCommands.current.get(ticket);if(recorded)recordCommand(recorded.action,recorded.generation);acceptedSequenceCommands.current.delete(ticket);requestResults.current.get(ticket)?.resolve(next);requestResults.current.delete(ticket);}
          } catch (error) {
            const guarded=['captureParagraph','swapParagraph','captureNavigation','restoreNavigation'].includes(String(entry.operation.command));
            for(const ticket of entry.tickets){acceptedSequenceCommands.current.delete(ticket);requestResults.current.get(ticket)?.reject(error);requestResults.current.delete(ticket);}
            if(!guarded){setPendingText([...pending.current.values()].join(''));problem(String(error));}
          }
          finally {
            count.current -= entry.tickets.length;
            if(alive.current)setTypingPending(queue.current,count.current>0&&!failed.current);
            else releaseTyping(queue.current);
            if (!count.current && !dirty.current && !failed.current && !composing.current) callbacks.current.onPending(false);
          }
        }
      } finally { draining.current = false; }
    });
    return ticket;
  }
  useEffect(() => {
    alive.current = true;
    callbacks.current.onPending(true);
    serial.current = invoke<Frame>('editor_open', { sessionId:editorSessionId??null, scene: initial, projectId:projectId??null, writingSystem:writingSystem?{definitionId:writingSystem.id,mappingVersion:writingSystem.mappingVersion,fontAssetId:writingSystem.font?.fontAssetId,fontChecksum:writingSystem.font?.checksum,fontFamily:writingSystem.font?.family,direction:writingSystem.direction}:null, width: Math.max(240, Math.min(4096, Math.round(container.current?.clientWidth || 640))) }).then(next => { accept(next); callbacks.current.onPending(false); }).catch(error => { problem(String(error)); callbacks.current.onPending(false); });
    return () => { for(const waiter of requestResults.current.values())waiter.reject(new Error('The writing scene closed before the paragraph request finished.'));requestResults.current.clear();releaseTyping(queue.current); alive.current = false; clearTimeout(checkpointTimer.current); clearTimeout(checkpointDeadline.current); clearTimeout(resizeTimer.current); };
  }, []);
  useEffect(() => {
    if (!surface.current) return;
    const element = surface.current;
    const observer=observeEditorSize(element,externalScroll?.current??scroll.current,(width,height)=>{
      if(observedWidth.current>0)viewportVersion.current++;
      observedWidth.current=width;observedHeight.current=height;
      setSurfaceWidth(width);setViewportHeight(height);
      // Window animations can report many intermediate sizes; paint the final one.
      clearTimeout(resizeTimer.current);
      resizeTimer.current=setTimeout(()=>{resizeTimer.current=undefined;operation({type:'refresh'});},100);
    });
    measureSize.current=observer.measure;
    return () => {observer.dispose();if(measureSize.current===observer.measure)measureSize.current=()=>{};clearTimeout(resizeTimer.current);};
  }, [Boolean(frame)]);
  useEffect(() => {
    if (!performanceProbe || !frame || probeStarted.current) return;
    probeStarted.current = true;
    void (async () => {
      const samples = [];
      const viewportFailuresDetail:Record<string,number>[]=[];
      let animationFrameTimeouts=0;
      // A background window can still update layout/IPC while screen frames are suspended.
      // Record that separately; the fixture checks exact text and viewport geometry.
      const settleFrame=()=>new Promise<void>(resolve=>{
        const animation=requestAnimationFrame(()=>{clearTimeout(timer);resolve();});
        const timer=setTimeout(()=>{cancelAnimationFrame(animation);animationFrameTimeouts++;resolve();},100);
      });
      const visible=()=>{
        const current=frameRef.current!,scale=(surface.current?.clientWidth??current.width)/current.width;
        const y=(current.cursor[1]-current.origin[1])/15,bottom=y+current.cursor[3]/15;
        const top=(scroll.current?.scrollTop??0)/scale,height=(scroll.current?.clientHeight??0)/scale;
        const shown=y>=current.top-1&&bottom<=current.top+current.height+1&&y>=top-1&&bottom<=top+height+1;
        if(!shown)viewportFailuresDetail.push({y,bottom,top,height,paintedTop:current.top,paintedHeight:current.height,totalHeight:current.totalHeight,scale});
        return shown;
      };
      let viewportFailures=0;
      let maxTimerDelayMs=0,lastTick=performance.now();
      const heartbeat=setInterval(()=>{const now=performance.now();maxTimerDelayMs=Math.max(maxTimerDelayMs,now-lastTick-10);lastTick=now;},10);
      if(await invoke<boolean>('resize_probe_enabled')) {
        const pause=(ms:number)=>new Promise(resolve=>setTimeout(resolve,ms));
        const resizeSamples=[];
        operation({type:'key',key:'End',ctrl:true});await serial.current;
        await settleFrame();
        if(scroll.current){scroll.current.scrollTop=Math.floor(scroll.current.scrollHeight/2);scroll.current.dispatchEvent(new Event('scroll'));}
        await pause(150);await serial.current;
        const initialTop=frameRef.current!.top;
        let expected=plainText(initial.document);
        for(const mode of ['760','1280','maximized','fullscreen','900','760','fullscreen','1280']) {
          const started=performance.now(),requests=probeRequests.current,expectedTop=scrollAnchor.current;
          await invoke('change_editor_probe_window',{mode});await pause(300);await serial.current;
          const current=frameRef.current!,scale=surface.current!.clientWidth/current.width;
          const paintedTop=current.top*scale,paintedBottom=(current.top+current.height)*scale;
          const viewportTop=scroll.current!.scrollTop,viewportBottom=viewportTop+scroll.current!.clientHeight;
          const covered=paintedTop<=viewportTop+2&&paintedBottom>=viewportBottom-2;
          const documentTop=viewportTop/scale;
          const documentMax=(scroll.current!.scrollHeight-scroll.current!.clientHeight)/scale;
          const anchorPreserved=Math.abs(documentTop-Math.min(expectedTop,documentMax))<4;
          const controlsVisible=[...document.querySelectorAll('.toolbar,.paragraph-settings')].every(element=>{const rect=element.getBoundingClientRect();return rect.top>=-1&&rect.bottom<=innerHeight+1;});
          const viewport=surface.current!.querySelector<HTMLElement>('.native-viewport')!;
          const boundedPaint=viewport.clientHeight<=Math.max(2160,scroll.current!.clientHeight*2);
          const beforeIdle=probeRequests.current;await pause(200);await serial.current;
          const idleRequests=probeRequests.current-beforeIdle;
          const inputStart=performance.now();insert(' مِ中文');expected+=' مِ中文';await serial.current;
          await settleFrame();
          resizeSamples.push({mode,windowWidth:innerWidth,windowHeight:innerHeight,nativeSizeApplied:['760','900','1280'].includes(mode)?Math.abs(innerWidth-Number(mode))<3:null,settleMs:performance.now()-started,resizeRequests:beforeIdle-requests,idleRequests,covered,anchorPreserved,controlsVisible,boundedPaint,documentTop,expectedTop,initialTop,inputMs:performance.now()-inputStart,timing:{...probeTiming.current},caretVisible:visible(),surfaceWidth:surface.current!.clientWidth,scrollHeight:scroll.current!.scrollHeight});
        }
        const rapidStart=probeRequests.current;
        for(let i=0;i<18;i++){await invoke('change_editor_probe_window',{mode:['760','900','1280'][i%3]});await pause(12);}
        await pause(350);await serial.current;
        const rapidRequests=probeRequests.current-rapidStart;
        const rapidFrame=frameRef.current!,rapidScale=surface.current!.clientWidth/rapidFrame.width;
        const rapidCovered=rapidFrame.top*rapidScale<=scroll.current!.scrollTop+2&&(rapidFrame.top+rapidFrame.height)*rapidScale>=scroll.current!.scrollTop+scroll.current!.clientHeight-2;
        checkpoint();await serial.current;await pause(900);clearInterval(heartbeat);
        const persisted=await invoke<{scene:Scene}|null>('load_scene');
        await invoke('finish_performance_probe',{report:{resizeSamples,rapidRequests,rapidCovered,animationFrameTimeouts,maxTimerDelayMs,pendingOperations:count.current,dirty:dirty.current,failed:failed.current,status:document.querySelector('.save-status')?.textContent,error:document.querySelector('.save-error')?.textContent,exact:plainText(JSON.parse(lastDocument.current))===expected,persistedExact:Boolean(persisted&&plainText(persisted.scene.document)===expected)}});
        return;
      }
      operation({ type: 'key', key: 'End', ctrl: true }); await serial.current;await settleFrame();if(!visible())viewportFailures++;
      if (closeProbe) {
        // Deliberately request close before the queued edits or idle checkpoint finish.
        for (const char of ' CLOSE سلام مِمِ中文👩🏽‍🚀') insert(char);
        await invoke('request_probe_close'); return;
      }
      const endY=frameRef.current!.cursor[1];
      sink.current!.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowUp',code:'ArrowUp',bubbles:true,cancelable:true}));await serial.current;
      const upY=frameRef.current!.cursor[1];
      sink.current!.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',bubbles:true,cancelable:true}));await serial.current;
      const downY=frameRef.current!.cursor[1];
      const verticalNavigationCorrect=upY<endY&&downY>upY&&downY===endY;
      operation({type:'key',key:'End',ctrl:true});await serial.current;
      const text = ' سلام مِمِمِ中文';
      let failures = 0;
      for (const char of text) {
        const start = performance.now(); insert(char); await serial.current;
        const timing = { ...probeTiming.current };
        await settleFrame();
        const preview=surface.current?.querySelector('img');
        if(preview&&(!preview.complete||!preview.naturalWidth))await new Promise<void>((resolve,reject)=>{
          const done=(error?:Error)=>{clearTimeout(timer);preview.removeEventListener('load',loaded);preview.removeEventListener('error',broken);error?reject(error):resolve();};
          const loaded=()=>done(),broken=()=>done(new Error('Synthetic editor image failed to load.'));
          const timer=setTimeout(()=>done(new Error('Synthetic editor image load timed out.')),3000);
          preview.addEventListener('load',loaded);preview.addEventListener('error',broken);
        });
        await settleFrame();
        samples.push({ ...timing, totalMs: performance.now() - start });
        if (failed.current) failures++;if(!visible())viewportFailures++;
      }
      checkpoint(); await serial.current;
      const burst = ' سلام مِمِمِ中文'.repeat(4);

      let framesWhileTyping = 0; const image = surface.current?.querySelector('img');
      const painted = () => framesWhileTyping++; image?.addEventListener('load', painted);
      const beforeRequests = probeRequests.current; const burstStart = performance.now();
      for (const char of burst) { insert(char); await new Promise(resolve => setTimeout(resolve, 5)); }
      image?.removeEventListener('load', painted);
      await serial.current; checkpoint(); await serial.current;
      const drainMs = performance.now() - burstStart;
      const expected = plainText(initial.document) + text + burst;
      operation({type:'key',key:'Home',ctrl:true});await serial.current;
      await settleFrame();if(!visible())viewportFailures++;
      operation({type:'key',key:'End',ctrl:true});
      await new Promise(resolve=>setTimeout(resolve,5));
      const manualTop=Math.round((scroll.current?.scrollHeight??0)/2);
      if(scroll.current){scroll.current.scrollTop=manualTop;scroll.current.dispatchEvent(new Event('scroll'));}
      await settleFrame();await serial.current;
      await settleFrame();
      const manualScrollPreserved=Math.abs((scroll.current?.scrollTop??0)-manualTop)<2;
      const current=frameRef.current!,scale=(surface.current?.clientWidth??current.width)/current.width;
      const displayedTop=(scroll.current?.scrollTop??0)/scale;
      const manualViewportPainted=Math.abs(current.top-displayedTop)<2&&current.height*scale>=(scroll.current?.clientHeight??0)-2;
      const exact = plainText(JSON.parse(lastDocument.current)) === expected;
      await new Promise(resolve => setTimeout(resolve, 900));
      const characterCount=document.querySelector<HTMLElement>('[data-character-count]')?.dataset.characterCount;
      clearInterval(heartbeat);
      const persisted = await invoke<{ scene: Scene } | null>('load_scene');
      const persistedExact = Boolean(persisted && plainText(persisted.scene.document) === expected);
      await invoke('finish_performance_probe', { report: { samples,verticalNavigationCorrect, failures,viewportFailures,viewportFailuresDetail,animationFrameTimeouts,manualScrollPreserved,manualViewportPainted,characterCount,initialCharacters:plainText(initial.document).length,maxTimerDelayMs, burst: { characters: [...burst].length, requests: probeRequests.current - beforeRequests, drainMs, maxTimerDelayMs, framesWhileTyping, exact, persistedExact } } });
    })().catch(error => { void invoke('finish_performance_probe', { report: { error: String(error) } }); });
  }, [Boolean(frame), performanceProbe, closeProbe]);
  async function flush() {
    if (composing.current) throw new Error('Finish entering the current word, then try again.');
    await serial.current; checkpoint(); await serial.current;
    if (failed.current) throw new Error('The editor could not confirm the latest changes. '+failureReason.current+' The window will stay open.');
  }
  async function paragraphRequest(command:'captureParagraph'|'swapParagraph',extra:Record<string,unknown>={}) {
    await flush();
    const ticket=operation({type:'command',command,...extra},command==='swapParagraph');
    if(ticket===undefined)throw new Error('The writing area is not ready for paragraph alternatives.');
    const result=await new Promise<Frame>((resolve,reject)=>{requestResults.current.set(ticket,{resolve,reject});});
    if(!result.paragraphCapture)throw new Error('The editor did not confirm the exact paragraph.');
    return result.paragraphCapture;
  }
  async function navigationRequest(command:'captureNavigation'|'restoreNavigation',bookmark?:NativeCaretBookmark){
    await flush();
    const ticket=operation({type:'command',command,...(bookmark?{bookmark}:{})});
    if(ticket===undefined)throw new Error('The writing area is not ready for navigation.');
    const result=await new Promise<Frame>((resolve,reject)=>requestResults.current.set(ticket,{resolve,reject}));
    if(!result.navigationCapture)throw new Error('The editor did not confirm the reading location.');
    return result.navigationCapture;
  }
  async function executeSequenceAction(value:CommandSequenceAction){
    const action=validateSequenceAction(value);
    if(composing.current)throw new Error('Finish entering the current word before playing a sequence.');
    if(!sequenceInputLocked.current||!alive.current||!activeRef.current||failed.current||!frameRef.current)throw new Error('The active writing area is not ready.');
    const payload=action.kind==='format'?{type:'command',command:action.command}:action.kind==='spacing'?{type:'command',command:'lineSpacing',lineSpacing:action.value}:{type:'command',command:'blockStyle',style:action.style,language:action.language,direction:action.direction};
    const ticket=operation(payload,true);
    if(ticket===undefined)throw new Error('The editor refused the sequence command.');
    await new Promise<Frame>((resolve,reject)=>requestResults.current.set(ticket,{resolve,reject}));
  }
  useImperativeHandle(ref, () => ({
    lockSequenceInput(locked){
      if(locked&&composing.current)throw new Error('Finish entering the current word before playing a sequence.');
      sequenceInputLocked.current=locked;if(sink.current)sink.current.readOnly=paused.current||inputBlocked();
    },
    focus(){if(activeRef.current&&!failed.current&&!inputBlocked())sink.current?.focus({preventScroll:true});},
    executeSequenceAction,
    captureNavigation(){return navigationRequest('captureNavigation');},
    async restoreNavigation(bookmark){await navigationRequest('restoreNavigation',bookmark);},
    flush,
    captureParagraph(){return paragraphRequest('captureParagraph');},
    swapParagraph(anchor,replacement){return paragraphRequest('swapParagraph',{anchor:{blockIndex:anchor.blockIndex,documentHash:anchor.documentHash},replacement});},
    async prepareClose() {
      if (composing.current) throw new Error('Finish entering the current word, then close again.');
      paused.current = true; if (sink.current) sink.current.readOnly = true;
      // An initial conversion refusal has no accepted edits to checkpoint.
      // The original scene can still be saved and the window closed normally.
      if (failed.current && !dirty.current && !count.current && !pending.current.size) return;
      await flush();
    },
    resume() { paused.current = !activeRef.current; if (sink.current) sink.current.readOnly = paused.current||inputBlocked(); },
    find(query) { if(inputBlocked())return; operation({type:'command',command:'find',query});sink.current?.focus({preventScroll:true}); },
    replace(query,replacement) { if(inputBlocked())return; operation({type:'command',command:'replace',query,replacement},true);sink.current?.focus({preventScroll:true}); },
    lineSpacing(value) { if(inputBlocked())return; operation({type:'command',command:'lineSpacing',lineSpacing:value},true,true);sink.current?.focus({preventScroll:true}); },
    paragraph(options) { if(inputBlocked())return; operation({type:'command',command:'blockStyle',...options},true,true);sink.current?.focus({preventScroll:true}); },
    command(name) { if(inputBlocked())return; operation({ type: 'command', command: name }, true,true); sink.current?.focus({ preventScroll: true }); }
  }));
  function insert(text: string,inputBoundary?:'composition'|'paste') { if (!inputBlocked()&&!paused.current&&text) operation({ type: 'text', text,...(inputBoundary?{inputBoundary}:{}) }, true); }
  function clipboardAction(action:'copy'|'cut'|'paste') {
    if(paused.current||failed.current||inputBlocked())return;
    setContextMenu(null);setClipboardMessage('');sink.current?.focus({preventScroll:true});
    if(count.current||(action!=='paste'&&!frameRef.current?.selectedText)){setClipboardMessage(count.current?'Wait for the current edit, then try again.':'Select text first.');return;}
    if(!document.execCommand(action))setClipboardMessage('The clipboard action was unavailable. Try again using the editor’s context menu or system shortcut.');
  }
  useEffect(()=>{
    if(!contextMenu)return;
    contextRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const dismiss=(event:PointerEvent)=>{if(!contextRef.current?.contains(event.target as Node))setContextMenu(null);};
    document.addEventListener('pointerdown',dismiss);return()=>document.removeEventListener('pointerdown',dismiss);
  },[contextMenu]);
  function key(event: React.KeyboardEvent<HTMLTextAreaElement>) {
    if(composing.current||paused.current||inputBlocked())return;
    const action=matchShortcut(event.nativeEvent,'editor');if(!action)return;
    const value=editorShortcutOperation(action);
    if(value){
      event.preventDefault();
      const edited=value.type==='key'?['Enter','Backspace','Delete','Tab'].includes(value.key):value.command!=='selectAll';
      operation(value,edited,true);
    }else if(['editor.copy','editor.cut','editor.paste'].includes(action)){
      event.preventDefault();clipboardAction(action.slice('editor.'.length) as 'copy'|'cut'|'paste');
    }else if(['editor.find','editor.replace','editor.findNext'].includes(action)){
      event.preventDefault();callbacks.current.onShortcut?.(action);
    }
  }
  const clickSequence=useRef({at:0,x:0,y:0,count:0});
  const pointerClickCount=useRef<1|2|3>(1);
  function point(event: React.PointerEvent, phase: 'down' | 'move' | 'up') {
    const current = frame; if (!current || !surface.current || failed.current || inputBlocked()) return;
    if (phase !== 'move' && event.button!==0) return;
    if (phase === 'move' && !(event.buttons&1)) return;
    event.preventDefault();
    const rect = surface.current.getBoundingClientRect(); const scale = rect.width / current.width;
    const x = Math.max(0, Math.round((event.clientX - rect.left) / scale * 15) + current.origin[0]);
    const y = Math.max(0, Math.round((event.clientY - rect.top) / scale * 15) + current.origin[1]);
    if (phase === 'down') {
      surface.current.setPointerCapture(event.pointerId);
      const previous=clickSequence.current,now=performance.now();
      const count=now-previous.at<=500&&Math.hypot(event.clientX-previous.x,event.clientY-previous.y)<=5?previous.count%3+1:1;
      clickSequence.current={at:now,x:event.clientX,y:event.clientY,count};pointerClickCount.current=mouseClickCount(count);
    }
    const pointerViewport=liveViewport(current.width);
    operation({ type: 'mouse', phase, x, y, width:pointerViewport.width,top:pointerViewport.top,viewportHeight:pointerViewport.viewportHeight, count: pointerClickCount.current, buttons: Boolean(event.buttons), shift: event.shiftKey });
    // A resize refresh may already have run before this old-bitmap pointer event.
    // Follow the hit test with reflow so its captured width cannot become final.
    if(pointerViewport.width!==Math.max(240,Math.min(4096,Math.round(rect.width))))operation({type:'refresh'});
    sink.current?.focus({ preventScroll: true });
  }
  const width = surfaceWidth || frame?.width || 854;
  const scale = frame ? width / frame.width : 1;
  const cursor = frame?.cursor ?? [];
  return <div className="native-editor" ref={container} onContextMenu={event=>{if(paused.current||failed.current||inputBlocked())return;event.preventDefault();const rect=sink.current?.getBoundingClientRect();setContextMenu({x:Math.min(innerWidth-180,Math.max(4,event.clientX||rect?.left||4)),y:Math.min(innerHeight-130,Math.max(4,event.clientY||rect?.top||4))});}}>
    {Boolean(frame?.missingGlyphs)&&<div role="status">Some characters need a supporting font.</div>}
    {clipboardMessage&&<div role="status">{clipboardMessage}</div>}
    {unicodeStatus&&<div className="native-status" role="status">{unicodeStatus}</div>}
    {contextMenu&&<div ref={contextRef} role="menu" aria-label="Editing actions" className="native-context-menu" style={{left:contextMenu.x,top:contextMenu.y}} onKeyDown={event=>{if(event.key==='Escape'){event.preventDefault();setContextMenu(null);sink.current?.focus({preventScroll:true});}if(['ArrowUp','ArrowDown'].includes(event.key)){event.preventDefault();const buttons=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('button')],index=buttons.indexOf(document.activeElement as HTMLButtonElement);buttons[(index+(event.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length]?.focus();}}}>{(['copy','cut','paste'] as const).map(action=><button key={action} role="menuitem" onClick={()=>clipboardAction(action)}>{action[0].toUpperCase()+action.slice(1)}</button>)}</div>}
    {error && <div className="native-error" role="alert">{error}<p>The original scene and confirmed revisions are preserved.</p>{pendingText && <label>Input awaiting confirmation<textarea readOnly value={pendingText} aria-label="Input awaiting confirmation"/></label>}</div>}
    {!frame && !error && <p role="status">Opening writing area…</p>}
    {frame && <div className="native-scroll" ref={scroll} onScroll={() => {
      if(!readManualScroll())return;
      if(resizeTimer.current===undefined)operation({type:'refresh'});
    }}>
      <div className="native-surface" ref={surface} style={{ aspectRatio: `${frame.width} / ${frame.totalHeight}` }} onPointerDown={event => point(event, 'down')} onPointerMove={event => point(event, 'move')} onPointerUp={event => point(event, 'up')}>
        <div className="native-viewport" style={{top:frame.top*scale,height:frame.height*scale}}>
        <img src={frame.image} alt="" draggable={false}/>
        {frame.selection.map((rect,i)=>{
          const top=(rect[1]-frame.origin[1])/15-frame.top,bottom=top+rect[3]/15;
          if(bottom<=0||top>=frame.height)return null;
          return <span key={i} className="native-selection" style={{left:(rect[0]-frame.origin[0])/15*scale,top:Math.max(0,top)*scale,width:rect[2]/15*scale,height:(Math.min(frame.height,bottom)-Math.max(0,top))*scale}}/>;
        })}
        {cursor.length === 4 && <span className="native-caret" style={{ left: `${(cursor[0] - frame.origin[0]) / 15 * scale}px`, top: `${((cursor[1] - frame.origin[1]) / 15-frame.top) * scale}px`, height: `${cursor[3] / 15 * scale}px` }}/>}
        <textarea ref={sink} className="native-input" aria-label="Scene text" disabled={Boolean(error)} spellCheck={false} style={{ left: `${((cursor[0] ?? frame.origin[0]) - frame.origin[0]) / 15 * scale}px`, top: `${(((cursor[1] ?? frame.origin[1]) - frame.origin[1]) / 15-frame.top) * scale}px`, height: `${(cursor[3] ?? 300) / 15 * scale}px` }}
          onKeyDown={key}
          onChange={event => { if (!composing.current) { insert(mapInput(event.currentTarget.value,writingSystem??null)); event.currentTarget.value = ''; } }}
          onCompositionStart={() => { composing.current = true; setTypingComposing(queue.current,true); callbacks.current.onPending(true); }}
          onCompositionEnd={event => { composing.current = false; setTypingComposing(queue.current,false); insert(mapInput(event.currentTarget.value,writingSystem??null),'composition'); event.currentTarget.value = ''; if (!count.current && !dirty.current) callbacks.current.onPending(false); }}
          onPaste={event => { event.preventDefault(); insert(event.clipboardData.getData('text/plain'),'paste'); }}
          onCopy={event => { event.preventDefault(); if (!count.current) event.clipboardData.setData('text/plain', frameRef.current?.selectedText ?? ''); }}
          onCut={event => { event.preventDefault(); if (!count.current) { event.clipboardData.setData('text/plain', frameRef.current?.selectedText ?? ''); operation({ type: 'key', key: 'Delete' }, true); } }}/>
        </div>
      </div>
    </div>}
  </div>;
});
