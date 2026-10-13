// External test hook, never bundled. Runs in each owned synthetic WebView.
const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args);
const rpc=(operation,args={})=>invoke('workspace_command',{operation,args});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const until=async predicate=>{for(let i=0;i<400;i++){if(await predicate())return;await sleep(50);}throw new Error('Multi-window fixture timed out');};
const scope=()=>document.querySelector('.window-workspace > [role=tabpanel]:not([hidden])')??document;
const ui=selector=>scope().querySelector(selector);
const hash=async value=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(value)))].map(byte=>byte.toString(16).padStart(2,'0')).join('');
const text=doc=>doc.content.map(block=>(block.content??[]).map(node=>node.text??'').join('')).join('\n\n');
const closeOriginal=Boolean(globalThis.__novelCloseOriginalFixture);
const report={checks:[],mode:closeOriginal?'close-original':'return-transfer'};
const check=(condition,message)=>{if(!condition)throw new Error(message);report.checks.push(message);};
const ready=()=>until(()=>ui('.native-input')&&!ui('.native-input').readOnly&&!ui('.toolbar [aria-label="Bold"]')?.disabled&&!document.querySelector('.window-workspace').inert&&!ui('.app')?.inert);
const tabMenu=async()=>{
  const tab=document.querySelector('[role=tab][aria-selected=true]');
  tab.dispatchEvent(new KeyboardEvent('keydown',{key:'F10',code:'F10',shiftKey:true,bubbles:true,cancelable:true}));
  await until(()=>document.querySelector('[role=menu][aria-label="Tab actions"]'));
  return document.querySelector('[role=menu][aria-label="Tab actions"]');
};
const flush=async()=>{
  const menu=await tabMenu();
  [...menu.querySelectorAll('[role=menuitem]')].find(button=>button.textContent==='Save tab').click();
  await sleep(50);
  await until(()=>!document.querySelector('.window-workspace').inert&&ui('.save-status')?.textContent.includes('Saved'));
  await ready();check(!document.querySelector('.save-error'),'Production tab save barrier acknowledges its scene');
};
const insert=async value=>{
  const sink=ui('.native-input');
  sink.dispatchEvent(new KeyboardEvent('keydown',{key:'End',code:'End',ctrlKey:true,bubbles:true,cancelable:true}));
  await sleep(100);
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(sink,value);
  sink.dispatchEvent(new Event('input',{bubbles:true}));
  await flush();
};
const command=async label=>{await ready();ui(`[aria-label="${label}"]`).click();await flush();};
const move=async label=>{
  const menu=await tabMenu();
  const action=[...menu.querySelectorAll('[role=menuitem]')].find(button=>button.textContent===label);
  check(Boolean(action)&&!action.disabled,'Tab context action is available: '+label);action.click();
};
try{
 await until(async()=>{try{return Boolean((await invoke('workspace_window_state')).windowLabel);}catch{return false;}});
 report.windowLabel=(await invoke('workspace_window_state')).windowLabel;
 const projects=await rpc('list_projects'),project=await rpc('open_project',{projectId:projects[0].id});
 const first=project.scenes[0],second=project.scenes[1];
 const originalText=text(first.scene.document).replace(/ MAIN مِ 中文$/,'');
 if(report.windowLabel==='main'){
  await ready();const original=await rpc('load_scene',{sceneId:first.id});
  await sleep(600);check((await rpc('load_scene',{sceneId:first.id})).revisionId===original.revisionId,'Opening a writing tab creates no revision');
  report.stage='Refusing handoff while synthetic composition is active';
  const compositionSink=ui('.native-input');
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(compositionSink,'');
  compositionSink.dispatchEvent(new CompositionEvent('compositionstart',{data:'',bubbles:true}));
  await move('Open in new window');
  await until(()=>!document.querySelector('.window-workspace').inert&&[...document.querySelectorAll('[role=alert]')].some(element=>element.textContent.includes('Finish entering')));
  const refusalState=await invoke('workspace_window_state');
  const refusalOwner=await invoke('workspace_find_tab',{route:{projectId:project.project.id,sceneId:first.id,view:'write'}});
  check(refusalState.tabs.length===1&&!refusalState.incoming.length&&refusalOwner?.ownerWindow==='main','Active composition refuses handoff and retains the sole source tab owner');
  check((await rpc('load_scene',{sceneId:first.id})).revisionId===original.revisionId,'Refused composition handoff creates no manuscript revision');
  compositionSink.dispatchEvent(new CompositionEvent('compositionend',{data:'',bubbles:true}));
  await ready();
  await insert(' MAIN مِ 中文');
  const source=await rpc('load_scene',{sceneId:first.id});
  check(text(source.scene.document)===text(original.scene.document)+' MAIN مِ 中文','Long Arabic/kasra/CJK input acknowledged in the source tab');
  await command('Undo');report.sourceUndoHash=await hash(text((await rpc('load_scene',{sceneId:first.id})).scene.document));
  await command('Redo');check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)===text(source.scene.document),'Source undo/redo baseline preserves exact mixed-script input');
  report.beforeMoveRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
  const sourceId=document.querySelector('[role=tab][aria-selected=true]').dataset.tabId;report.sourceTabId=sourceId;
  const backgroundScroll=ui('.native-scroll');
  backgroundScroll.scrollTop=Math.min(400,backgroundScroll.scrollHeight-backgroundScroll.clientHeight);
  backgroundScroll.dispatchEvent(new Event('scroll'));await sleep(150);await flush();
  report.backgroundSourceScrollTop=backgroundScroll.scrollTop;
  report.stage='Opening Project tab';[...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent==='Project').click();
  await until(()=>ui('.workspace-nav button[aria-pressed=true]')?.textContent==='Project');
  report.stage='Returning to source tab';document.querySelector(`[data-tab-id="${sourceId}"]`).click();await ready();
  await until(()=>Math.abs(ui('.native-scroll').scrollTop-report.backgroundSourceScrollTop)<2);
  check(Math.abs(ui('.native-scroll').scrollTop-report.backgroundSourceScrollTop)<2,'Background tool navigation and drain preserve the source viewport');
  check((await rpc('load_scene',{sceneId:first.id})).revisionId===report.beforeMoveRevision,'Background tool navigation creates no writing revision');
  const sink=ui('.native-input');sink.dispatchEvent(new KeyboardEvent('keydown',{key:'Home',code:'Home',ctrlKey:true,bubbles:true,cancelable:true}));await sleep(100);sink.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',shiftKey:true,bubbles:true,cancelable:true}));
  await sleep(100);const sourceFrame=await invoke('editor_edit',{sessionId:sourceId,operation:{type:'snapshot',preview:false,followCursor:false}});report.sourceFrame={position:sourceFrame.position,selectedText:sourceFrame.selectedText};check(sourceFrame.selection.length>0&&sourceFrame.selectedText.length>0,'Source selection is nonempty before handoff');
  const scroll=ui('.native-scroll');scroll.scrollTop=Math.min(400,scroll.scrollHeight-scroll.clientHeight);scroll.dispatchEvent(new Event('scroll'));await sleep(150);report.sourceScrollTop=scroll.scrollTop;
  report.stage='Detaching source';await move('Open in new window');
  await until(async()=>{const found=await invoke('workspace_find_tab',{route:{projectId:project.project.id,sceneId:first.id,view:'write'}});return found&&found.ownerWindow!=='main';});
  await until(()=>!document.querySelector(`[data-tab-id="${sourceId}"]`));check(!document.querySelector(`[data-tab-id="${sourceId}"]`),'Source tab disappears only after the destination acknowledges readiness');
  let rejected=false;try{await invoke('editor_edit',{sessionId:sourceId,operation:{type:'text',text:'WRONG WINDOW'}});}catch{rejected=true;}
  check(rejected,'Former source window cannot edit the transferred session');
  if(closeOriginal){
   const surviving=(await invoke('workspace_window_state')).tabs.find(tab=>tab.route.view==='library');
   check(Boolean(surviving),'Original window retains a registered Project tab until native close');
   report.originalToolTabId=surviving.id;
   report.originalCloseReady=true;
   report.stage='Ready for fixture native original-window close';
  }else{
  // The project tool tab survives the move and can open a different scene.
  [...scope().querySelectorAll('.manuscript-tree button')].find(button=>button.textContent.includes(second.scene.title)).click();
  await ready();await insert(' SECOND مستقل 中文');
  check(text((await rpc('load_scene',{sceneId:second.id})).scene.document)===text(second.scene.document)+' SECOND مستقل 中文','Another writing session stays independent while the first is detached');
  await until(async()=>{const found=await invoke('workspace_find_tab',{route:{projectId:project.project.id,sceneId:first.id,view:'write'}});return found?.ownerWindow==='main';});
  report.stage='Returning to source tab';document.querySelector(`[data-tab-id="${sourceId}"]`).click();await ready();
  const back=await rpc('load_scene',{sceneId:first.id});
  check(text(back.scene.document)===text(original.scene.document)+' MAIN مِ 中文 CHILD مِ 中文','Returned tab retains exact child-window edits');
  check(document.querySelectorAll(`[data-tab-id="${sourceId}"]`).length===1,'Moving back retains one canonical writing tab');
  await command('Undo');
  report.returnUndoHash=await hash(text((await rpc('load_scene',{sceneId:first.id})).scene.document));check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)!==text(back.scene.document),'Undo remains available after returning to the main window');
  await command('Redo');
  check(text((await rpc('load_scene',{sceneId:second.id})).scene.document)===text(second.scene.document)+' SECOND مستقل 中文','Undo and redo never cross scene sessions');
  report.savedRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
  const childPlanning=await invoke('workspace_find_tab',{route:{projectId:project.project.id,view:'plan'}});
  check(Boolean(childPlanning)&&childPlanning.ownerWindow!=='main','Child retains its Planning tool until the last-tab move is tested');
  report.autoCloseSourceWindow=childPlanning.ownerWindow;
  await invoke('workspace_request_tab_move',{tabId:childPlanning.tab.id});
  await until(async()=>{const found=await invoke('workspace_find_tab',{route:{projectId:project.project.id,view:'plan'}});return found?.ownerWindow==='main';});
  check((await invoke('workspace_window_state')).tabs.some(tab=>tab.id===childPlanning.tab.id),'Last child tab commits to the existing destination before source closure');
  check((await rpc('load_scene',{sceneId:first.id})).revisionId===report.savedRevision,'Moving the final tool tab creates no manuscript revision');
  // Keep the destination alive for two external GTK sampling intervals.
  await sleep(400);
  }

 }else{
  await ready();await until(async()=>Boolean((await invoke('workspace_window_state')).tabs.find(tab=>tab.route.view==='write')));
  const tab=(await invoke('workspace_window_state')).tabs.find(tab=>tab.route.view==='write');
  report.incomingRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
  const incomingFrame=await invoke('editor_edit',{sessionId:tab.id,operation:{type:'snapshot',preview:false,followCursor:false}});report.incomingFrame={position:incomingFrame.position,selectedText:incomingFrame.selectedText};
  await until(()=>ui('.native-scroll'));report.incomingScrollTop=ui('.native-scroll').scrollTop;
  await command('Undo');report.incomingUndoHash=await hash(text((await rpc('load_scene',{sceneId:first.id})).scene.document));check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)!==originalText+' MAIN مِ 中文','Native undo remains available after its first handoff');
  await command('Redo');
  check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)===originalText+' MAIN مِ 中文','Native redo survives its first handoff');
  await insert(' CHILD مِ 中文');
  await command('Undo');report.childUndoHash=await hash(text((await rpc('load_scene',{sceneId:first.id})).scene.document));await command('Redo');
  if(closeOriginal){
   report.stage='Waiting for native original-window destruction';
   await until(async()=>!(await invoke('workspace_find_tab',{route:{projectId:project.project.id,view:'library'}})));
   report.originalWindowClosed=true;
   check((await invoke('workspace_window_state')).windowLabel!=='main','Detached native window remains available after original window destruction');
   await insert(' AFTER CLOSE مِ 中文');
   const afterClose=await rpc('load_scene',{sceneId:first.id});
   check(text(afterClose.scene.document)===originalText+' MAIN مِ 中文 CHILD مِ 中文 AFTER CLOSE مِ 中文','Detached window acknowledges exact multilingual input after the original window closes');
   check(text((await rpc('load_scene',{sceneId:second.id})).scene.document)===text(second.scene.document),'Closing the original window leaves the second scene unchanged');
   report.savedRevision=afterClose.revisionId;
   report.afterOriginalCloseHash=await hash(text(afterClose.scene.document));
   report.stage='Detached acknowledged save ready for whole-app guarded quit';
  }else{
   report.stage='Waiting for independent second-scene save before return';
   await until(async()=>text((await rpc('load_scene',{sceneId:second.id})).scene.document)===text(second.scene.document)+' SECOND مستقل 中文');
  [...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent==='Planning').click();
  await until(()=>ui('.workspace-nav button[aria-pressed=true]')?.textContent==='Planning'&&!document.querySelector('.window-workspace').inert);
  const childPlanning=(await invoke('workspace_window_state')).tabs.find(candidate=>candidate.route.view==='plan');
  check(Boolean(childPlanning),'Child keeps a registered Planning tool while its writing tab returns');report.childPlanningTabId=childPlanning.id;
  document.querySelector(`[data-tab-id="${tab.id}"]`).click();await ready();
  await move('Move back to main window');
  await until(async()=>!(await invoke('workspace_window_state')).tabs.some(candidate=>candidate.id===tab.id));
  report.childToolTabs=(await invoke('workspace_window_state')).tabs.length;
  check(report.childToolTabs===1&&(await invoke('workspace_window_state')).tabs[0].id===childPlanning.id,'Returning a writing tab preserves the remaining child tool tab');
  }

 }
}catch(error){report.error=String(error);report.nativeState=await invoke('workspace_window_state').catch(()=>null);report.uiErrors=[...document.querySelectorAll('[role=alert]')].map(element=>element.textContent);report.visibleHeading=ui('.workspace-nav button[aria-pressed=true]')?.textContent;report.panels=[...document.querySelectorAll('.window-workspace > [role=tabpanel]')].map(panel=>({id:panel.id,hidden:panel.hidden,inputs:panel.querySelectorAll('.native-input').length,scrolls:panel.querySelectorAll('.native-scroll').length,heading:panel.querySelector('.topbar h1')?.textContent,status:panel.querySelector('.save-status')?.textContent}));}
// Child returns its report while still open. Main requests the real all-window guard.
if((!closeOriginal&&report.windowLabel==='main')||(closeOriginal&&report.windowLabel!=='main'&&report.originalWindowClosed&&!report.error))setTimeout(()=>void invoke('workspace_request_quit'),1000);
return JSON.stringify(report);
