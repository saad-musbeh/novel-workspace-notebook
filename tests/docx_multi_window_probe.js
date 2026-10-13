// Externally injected in every synthetic WebView to exercise a real project save barrier.
const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args);
const rpc=(operation,args={})=>invoke('workspace_command',{operation,args});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const until=async predicate=>{const deadline=performance.now()+25000;while(performance.now()<deadline){const value=await predicate();if(value)return value;await sleep(30);}throw new Error('DOCX multi-window probe timed out at '+report.stage);};
const scope=()=>document.querySelector('.window-workspace > [role=tabpanel]:not([hidden])')??document;
const text=doc=>doc.content.map(block=>(block.content??[]).map(node=>node.text??'').join('')).join('\n\n');
const report={checks:[]},channel=new BroadcastChannel('novel-docx-save-barrier');
window.__novelReleaseProgress=report;
const check=(condition,message)=>{if(!condition)throw new Error(message);report.checks.push(message);};
const waitMessage=(type)=>new Promise(resolve=>{const fn=event=>{if(event.data?.type===type){channel.removeEventListener('message',fn);resolve(event.data);}};channel.addEventListener('message',fn);});
const ready=()=>until(()=>scope().querySelector('.native-input')&&!scope().querySelector('.native-input').readOnly&&!document.querySelector('.window-workspace').inert);
const menu=async()=>{const tab=document.querySelector('[role=tab][aria-selected=true]');tab.dispatchEvent(new KeyboardEvent('keydown',{key:'F10',code:'F10',shiftKey:true,bubbles:true,cancelable:true}));await until(()=>document.querySelector('[role=menu][aria-label="Tab actions"]'));return document.querySelector('[role=menu][aria-label="Tab actions"]');};
const routeTabs=async()=> (await invoke('workspace_window_state')).tabs;
try {
 const initialState=await until(async()=>{const state=await invoke('workspace_window_state');return state.windowLabel?state:null;});
 report.windowLabel=initialState.windowLabel;
 const projects=await rpc('list_projects');
 check(projects.length===1&&projects[0].title==='Synthetic Word Export Integration','Only the synthetic project is open in this window');
 const project=await rpc('open_project',{projectId:projects[0].id}),scene=project.scenes[0];
 report.projectId=project.project.id;report.sceneId=scene.id;
 if(report.windowLabel==='main'){
  report.stage='Create Project tab and move writing tab';await ready();
  [...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent.trim()==='Project').click();
  await until(async()=> (await routeTabs()).some(tab=>tab.route?.view==='library'));
  const libraryTab=(await routeTabs()).find(tab=>tab.route?.view==='library');
  const writeTab=(await routeTabs()).find(tab=>tab.route?.view==='write'&&tab.route.sceneId===scene.id);
  check(Boolean(libraryTab)&&Boolean(writeTab),'A separate Project tab and writing tab are open');
  const dirtyPromise=waitMessage('dirty');
  const writeTabButton=document.querySelector(`[data-tab-id="${writeTab.id}"]`);
  writeTabButton.dispatchEvent(new KeyboardEvent('keydown',{key:'F10',code:'F10',shiftKey:true,bubbles:true,cancelable:true}));
  await until(()=>document.querySelector('[role=menu][aria-label="Tab actions"]'));
  const actions=document.querySelector('[role=menu][aria-label="Tab actions"]');
  const open=[...actions.querySelectorAll('[role=menuitem]')].find(button=>button.textContent==='Open in new window');
  check(Boolean(open)&&!open.disabled,'The synthetic writing tab can move to a second native window');open.click();
  await until(async()=>{const owner=await invoke('workspace_find_tab',{route:{projectId:project.project.id,sceneId:scene.id,view:'write'}});return owner&&owner.ownerWindow!=='main';});
  report.stage='Use retained main Project controls';
  await until(()=>[...scope().querySelectorAll('button')].some(button=>button.textContent.trim()==='Export Word (.docx)'));
  check(true,'The main Project tab remains ready to export');
  await until(()=>[...scope().querySelectorAll('button')].some(button=>button.textContent.trim()==='Export Word (.docx)'));
  report.stage='Wait for dirty child-window draft';
  const dirty=await dirtyPromise;
  check(dirty.beforeRevision===scene.revisionId,'Child confirms its pending text is not yet the saved scene head');
  report.expectedText=dirty.expectedText;report.childWindow=dirty.windowLabel;
  const button=[...scope().querySelectorAll('button')].find(item=>item.textContent.trim()==='Export Word (.docx)');
  const started=performance.now();button.click();
  await until(()=>[...scope().querySelectorAll('.workspace-notice,[role=alert]')].some(node=>/Word manuscript exported|error|failed/i.test(node.textContent)));
  const receipt=[...scope().querySelectorAll('.workspace-notice')].map(node=>node.textContent).find(value=>/Word manuscript exported/i.test(value));
  check(Boolean(receipt),'Project export completed after the second window acknowledged its pending draft');
  const saved=await rpc('load_scene',{sceneId:scene.id});
  check(saved.revisionId!==dirty.beforeRevision&&text(saved.scene.document)===report.expectedText,'The second window pending text was saved before the exported snapshot');
  report.savedRevision=saved.revisionId;report.exportMilliseconds=performance.now()-started;report.receipt=receipt;
  channel.postMessage({type:'export-saved',revisionId:saved.revisionId,expectedText:report.expectedText});
  report.bundle=await rpc('open_project',{projectId:project.project.id});
  report.checks.push('Native project export completed only after each project window returned its save acknowledgement');
  setTimeout(()=>void invoke('workspace_request_quit'),1800);
 }else{
  if(!scope().querySelector('.native-input')){
   report.stage='Open manuscript in transferred Project tab';
   await until(()=>[...scope().querySelectorAll('.manuscript-tree button')].some(button=>button.textContent.includes(scene.scene.title)));
   [...scope().querySelectorAll('.manuscript-tree button')].find(button=>button.textContent.includes(scene.scene.title)).click();
  }
  await until(async()=>{const state=await invoke('workspace_window_state');return state.incoming.length===0&&state.tabs.some(tab=>tab.route?.sceneId===scene.id&&tab.route?.view==='write');});
  report.stage='Stage pending child-window text';await ready();
  const before=await rpc('load_scene',{sceneId:scene.id}),suffix=' From second window مِ 中文 👩🏽‍🚀';
  const sink=scope().querySelector('.native-input');sink.focus();
  sink.dispatchEvent(new KeyboardEvent('keydown',{key:'End',code:'End',ctrlKey:true,bubbles:true,cancelable:true}));
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(sink,suffix);
  sink.dispatchEvent(new Event('input',{bubbles:true}));
  const expectedText=text(before.scene.document)+suffix;
  const immediate=await rpc('load_scene',{sceneId:scene.id});
  check(immediate.revisionId===before.revisionId,'Pending second-window edit has not been saved before export begins');
  report.beforeRevision=before.revisionId;report.expectedText=expectedText;
  const savedPromise=waitMessage('export-saved');
  channel.postMessage({type:'dirty',beforeRevision:before.revisionId,expectedText,windowLabel:report.windowLabel});
  report.stage='Wait for project export save acknowledgement';
  const done=await savedPromise;
  check(done.expectedText===expectedText,'Main window observed the exact second-window text');
  await until(async()=>{const saved=await rpc('load_scene',{sceneId:scene.id});return saved.revisionId===done.revisionId&&text(saved.scene.document)===expectedText;});
  report.savedRevision=done.revisionId;
  check(true,'Second window remained open until its project-wide save acknowledgement completed');
 }
}catch(error){report.error=String(error);report.nativeState=await invoke('workspace_window_state').catch(()=>null);report.uiErrors=[...document.querySelectorAll('[role=alert]')].map(node=>node.textContent);if(report.windowLabel==='main')setTimeout(()=>void invoke('workspace_request_quit'),1000);}
return JSON.stringify(report);
