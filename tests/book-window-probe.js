// External synthetic fixture only; never bundled into the product.
const invoke=(command,args)=>new Promise((resolve,reject)=>{const timeout=setTimeout(()=>reject(new Error('RPC timed out: '+command+' at '+report.stage)),10000);window.__TAURI_INTERNALS__.invoke(command,args).then(resolve,reject).finally(()=>clearTimeout(timeout));});
const rpc=(operation,args={})=>invoke('workspace_command',{operation,args});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const until=async (predicate,timeout=30000)=>{const deadline=performance.now()+timeout;while(performance.now()<deadline){if(await predicate())return;await sleep(50);}throw new Error('Book window fixture timed out at '+report.stage);};
const scope=()=>document.querySelector('.window-workspace > [role=tabpanel]:not([hidden])')??document;
const ui=selector=>scope().querySelector(selector);
const text=doc=>doc.content.map(block=>(block.content??[]).map(node=>node.text??(node.type==='hardBreak'?'\n':'')).join('')).join('\n\n');
const report={mode:'book-transfer',checks:[],stage:'Initializing window'};
const check=(condition,message)=>{if(!condition)throw new Error(message);report.checks.push(message);};
const selected=()=>document.querySelector('[role=tab][aria-selected=true]');
const menu=async()=>{selected().dispatchEvent(new KeyboardEvent('keydown',{key:'F10',code:'F10',shiftKey:true,bubbles:true,cancelable:true}));await until(()=>document.querySelector('[role=menu][aria-label="Tab actions"]'));return document.querySelector('[role=menu][aria-label="Tab actions"]');};
const action=async label=>{report.stage='Tab action: '+label;const root=await menu(),button=[...root.querySelectorAll('[role=menuitem]')].find(item=>item.textContent===label);check(button&&!button.disabled,'Available production tab action: '+label);button.click();};
const ready=()=>until(()=>ui('.book-scroll')&&ui('.native-input')&&!ui('.native-input').readOnly&&!ui('[aria-label="Undo"]')?.disabled&&!document.querySelector('.window-workspace').inert&&!ui('.app')?.inert);
const save=async()=>{await action('Save tab');await until(()=>!document.querySelector('.window-workspace').inert);await ready();check(!ui('.save-error'),'Book save barrier succeeds');};
const folds=()=>JSON.parse(ui('.book-workspace')?.dataset.bookFoldedSceneIds??'[]');
const position=()=>{const host=ui('.book-scroll');return {character:ui('[aria-label="Book selection"]').value,scene:host.dataset.bookSceneId,offset:Number(host.dataset.bookSceneOffset??0),scrollTop:host.scrollTop};};
try{
 await until(async()=>{try{return Boolean((await invoke('workspace_window_state')).windowLabel);}catch{return false;}});
 report.windowLabel=(await invoke('workspace_window_state')).windowLabel;
 const projects=await rpc('list_projects'),project=await rpc('open_project',{projectId:projects[0].id}),first=project.scenes[0],second=project.scenes[1];
 const route={projectId:project.project.id,view:'book'};
 if(report.windowLabel==='main'){
  await until(()=>ui('.native-input')&&!ui('.native-input').readOnly);
  // Retain a different normal draft so moving Book cannot auto-close main.
  [...scope().querySelectorAll('.manuscript-tree button')].find(button=>button.textContent.includes(second.scene.title)).click();
  await until(()=>ui('.scene-title')?.value===second.scene.title&&!ui('.native-input')?.readOnly);
  const sourceDraft=selected().dataset.tabId;
  const planning={schemaVersion:1,fields:[],records:[{id:'book-window-hero',kind:'character',name:'نور / 明',aliases:[],notes:'',values:{},links:[{kind:'scene',id:first.id}]}],plotPoints:[],facts:[],boards:[{id:'book-window-board',name:'Story board',cards:[],edges:[],groups:[]}],lanes:[{id:'book-window-lane',name:'Main plot',kind:'main'}],events:[],constraints:[],issues:[]};
  await rpc('save_project_data',{projectId:project.project.id,kind:'planning',payload:planning,parent:null});
  [...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent==='Book').click();await ready();
  report.stage='Main folding the second scene';
  const foldHost=ui('.book-scroll');foldHost.scrollTop=foldHost.scrollHeight;foldHost.dispatchEvent(new Event('scroll'));
  await until(()=>{const card=ui(`[data-book-scene="${second.id}"]`);return card?.querySelector('.native-input')&&!card.querySelector('.native-input').readOnly&&!card.querySelector('[aria-label="Undo"]')?.disabled;});
  const foldButton=[...scope().querySelectorAll('.book-scene-heading button')].find(button=>button.getAttribute('aria-label')==='Fold '+second.scene.title);
  check(Boolean(foldButton),'Second scene has a production Fold control');foldButton.click();
  await until(()=>folds().includes(second.id)&&ui(`[data-book-scene="${second.id}"]`)?.classList.contains('book-folded')&&!ui('.book-workspace').inert);
  check(ui(`[data-book-scene="${second.id}"] button`).getAttribute('aria-expanded')==='false','Source second scene visibly folds before detach');
  foldHost.scrollTop=0;foldHost.dispatchEvent(new Event('scroll'));await ready();
  const selector=ui('[aria-label="Book selection"]');selector.value='book-window-hero';selector.dispatchEvent(new Event('change',{bubbles:true}));
  await until(()=>ui('.book-workspace')?.dataset.bookCharacter==='book-window-hero'&&ui('.book-workspace')?.dataset.loadedScenes==='1');await ready();
  const sink=ui('.native-input');sink.dispatchEvent(new KeyboardEvent('keydown',{key:'End',code:'End',ctrlKey:true,bubbles:true,cancelable:true}));await sleep(100);
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(sink,' BOOK MAIN مِ 中文');sink.dispatchEvent(new Event('input',{bubbles:true}));await save();
  report.beforeMoveRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
  report.sourceText=text((await rpc('load_scene',{sceneId:first.id})).scene.document);
  const host=ui('.book-scroll');host.scrollTop=Math.min(600,host.scrollHeight-host.clientHeight);host.dispatchEvent(new Event('scroll'));await sleep(200);
  report.sourcePosition=position();check(report.sourcePosition.scene===first.id&&report.sourcePosition.offset>100,'Filtered book records the long scene logical scroll offset');
  check(folds().includes(second.id)&&!folds().includes(first.id),'Filtering keeps the folded second scene without folding the active draft');
  report.sourceFolds=folds();
  const bookId=selected().dataset.tabId;report.bookId=bookId;
  await action('Open in new window');
  await until(async()=>{const owner=await invoke('workspace_find_tab',{route});return owner&&owner.ownerWindow!=='main';});
  await until(()=>!document.querySelector(`[data-tab-id="${bookId}"]`));
  check(Boolean(document.querySelector(`[data-tab-id="${sourceDraft}"]`)),'Source retains the independent normal draft after book detach');
  let refused=false;try{await invoke('editor_edit',{sessionId:bookId+':book:'+first.id,operation:{type:'text',text:'FOREIGN'}});}catch{refused=true;}
  check(refused,'Former source refuses writes to transferred book child session');
  report.stage='Main waiting for child return';await until(async()=>{const owner=await invoke('workspace_find_tab',{route});return owner?.ownerWindow==='main';},75000);
  document.querySelector(`[data-tab-id="${bookId}"]`).click();await ready();
  report.stage='Main waiting for returned filter and scene offset';await until(()=>position().character==='book-window-hero'&&position().scene===first.id&&Math.abs(position().offset-report.sourcePosition.offset)<4);
  report.returnPosition=position();report.returnFolds=folds();
  check(JSON.stringify(report.returnFolds)===JSON.stringify(report.sourceFolds),'Returned main window preserves exact Book folds');
  check(document.querySelectorAll(`[data-tab-id="${bookId}"]`).length===1,'Returned Book retains exactly one canonical tab');
  check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)===report.sourceText+' BOOK CHILD مِ 中文','Returning Book retains exact child Unicode edits');
  ui('[aria-label="Undo"]').click();await save();
  check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)===report.sourceText,'Child Undo history survives return to main');
  ui('[aria-label="Redo"]').click();await save();
  check(text((await rpc('load_scene',{sceneId:second.id})).scene.document)===text(second.scene.document),'Book Undo and transfer never change the independent draft');
  report.savedRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
 }else{
  report.stage='Child waiting for book editor readiness';await ready();report.stage='Child waiting for registered book tab';await until(async()=>Boolean((await invoke('workspace_window_state')).tabs.find(tab=>tab.route.view==='book')));
  const tab=(await invoke('workspace_window_state')).tabs.find(tab=>tab.route.view==='book');
  report.bookId=tab.id;report.incomingRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
  report.stage='Child waiting for restored filter and scene offset';await until(()=>position().character==='book-window-hero'&&position().scene===first.id&&position().offset>100);report.incomingPosition=position();
  report.incomingFolds=folds();check(report.incomingFolds.length===1&&report.incomingFolds[0]===second.id,'Detached child preserves the folded second scene');
  report.incomingText=text((await rpc('load_scene',{sceneId:first.id})).scene.document);
  ui('[aria-label="Undo"]').click();await save();
  check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)===text(first.scene.document).replace(/ BOOK MAIN مِ 中文$/,''),'Native book Undo survives first handoff');
  ui('[aria-label="Redo"]').click();await save();
  check(text((await rpc('load_scene',{sceneId:first.id})).scene.document)===report.incomingText,'Native book Redo survives first handoff');
  const sink=ui('.native-input');sink.dispatchEvent(new KeyboardEvent('keydown',{key:'End',code:'End',ctrlKey:true,bubbles:true,cancelable:true}));await sleep(100);
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(sink,' BOOK CHILD مِ 中文');sink.dispatchEvent(new Event('input',{bubbles:true}));await save();
  // Restore the captured reading passage after caret-following input.
  const host=ui('.book-scroll');host.scrollTop=report.incomingPosition.scrollTop;host.dispatchEvent(new Event('scroll'));await until(()=>Math.abs(host.scrollTop-report.incomingPosition.scrollTop)<4&&position().scene===first.id&&Math.abs(position().offset-report.incomingPosition.offset)<4);await sleep(200);
  // Keep a tool tab alive until this WebView returns its external report.
  [...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent==='Planning').click();
  report.stage='Child waiting for Planning tool';await until(()=>ui('.workspace-nav button[aria-pressed=true]')?.textContent==='Planning');
  document.querySelector(`[data-tab-id="${tab.id}"]`).click();await ready();
  await until(()=>Math.abs(ui('.book-scroll').scrollTop-report.incomingPosition.scrollTop)<4&&position().scene===first.id&&Math.abs(position().offset-report.incomingPosition.offset)<4);report.beforeReturnPosition=position();
  check(JSON.stringify(folds())===JSON.stringify(report.incomingFolds),'Child edits and tool navigation preserve Book folds before return');
  await action('Move back to main window');
  await until(async()=>!(await invoke('workspace_window_state')).tabs.some(candidate=>candidate.id===tab.id));
  report.returnCommitted=true;
 }
}catch(error){report.error=String(error);report.state=await invoke('workspace_window_state').catch(()=>null);report.alerts=[...document.querySelectorAll('[role=alert]')].map(node=>node.textContent);report.dom={tab:selected()?.dataset.tabId,book:Boolean(ui('.book-scroll')),input:Boolean(ui('.native-input')),inputReadOnly:ui('.native-input')?.readOnly,undoDisabled:ui('[aria-label="Undo"]')?.disabled,hostInert:document.querySelector('.window-workspace')?.inert,appInert:ui('.app')?.inert,heading:ui('.workspace-nav button[aria-pressed=true]')?.textContent,bookCharacter:ui('.book-workspace')?.dataset.bookCharacter,loadedScenes:ui('.book-workspace')?.dataset.loadedScenes,position:ui('.book-scroll')?position():null};}
if(report.windowLabel==='main')setTimeout(()=>void invoke('workspace_request_quit'),3000);
return JSON.stringify(report);
