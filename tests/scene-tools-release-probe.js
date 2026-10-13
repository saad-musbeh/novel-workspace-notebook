// External instrumentation only; every project and input is synthetic.
const report={checks:[],timings:{}};window.__novelReleaseProgress=report;
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const scope=()=>document.querySelector('.window-workspace > [role=tabpanel]:not([hidden])')??document;
const ui=selector=>scope().querySelector(selector),all=selector=>[...scope().querySelectorAll(selector)];
const ready=async predicate=>{const end=performance.now()+10000;while(performance.now()<end){if(predicate())return;await sleep(30);}throw new Error('Timeout at '+report.stage);};
const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args),rpc=(operation,args={})=>invoke('workspace_command',{operation,args});
const check=(condition,label)=>{if(!condition)throw new Error(label);report.checks.push(label);};
const click=(selector,label)=>{const button=all(selector).find(node=>node.textContent===label);if(!button)throw new Error('Missing button '+label);button.click();};
const checkContrast=selectors=>{
 const previousTheme=document.documentElement.dataset.theme;
 const channels=color=>color.match(/[\d.]+/g).map(Number);
 const luminance=rgb=>rgb.slice(0,3).map(value=>value/255).map(value=>value<=.04045?value/12.92:((value+.055)/1.055)**2.4).reduce((sum,value,index)=>sum+value*[.2126,.7152,.0722][index],0);
 try{for(const theme of ['light','dark','system']){document.documentElement.dataset.theme=theme;
  for(const element of document.querySelectorAll(selectors)){
   if(!element.getClientRects().length)continue;
   const foreground=luminance(channels(getComputedStyle(element).color));let parent=element,background;
   while(parent){const rgb=channels(getComputedStyle(parent).backgroundColor);if(rgb.length===3||rgb[3]===1){background=luminance(rgb);break;}parent=parent.parentElement;}
   const ratio=(Math.max(foreground,background)+.05)/(Math.min(foreground,background)+.05);
   if(!(ratio>=4.5))throw new Error(theme+' contrast '+ratio.toFixed(2)+' for '+element.textContent.slice(0,50));
  }
 }}finally{if(previousTheme===undefined)delete document.documentElement.dataset.theme;else document.documentElement.dataset.theme=previousTheme;}
};
const select=(element,value)=>{Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(element,value);element.dispatchEvent(new Event('change',{bubbles:true}));};
const canonical=value=>Array.isArray(value)?value.map(canonical):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])])):value;
const equal=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
try{
 report.stage='Seed reference metadata';await ready(()=>ui('.native-input')&&!ui('.toolbar [aria-label="Bold"]').disabled);
 const project=(await rpc('list_projects'))[0],bundle=await rpc('open_project',{projectId:project.id}),first=bundle.scenes[0],second=bundle.scenes[1];
 const records=[{id:'author-character',kind:'character',name:'نور reference character',aliases:[],notes:'Explicit linked evidence مِ 中文',values:{},links:[{kind:'scene',id:first.id}]},{id:'author-place',kind:'location',name:'Archive room',aliases:[],notes:'Second scene only',values:{},links:[{kind:'scene',id:second.id}]}];
 const fact={id:'author-fact',subjectId:records[0].id,property:'Door code',value:'رمز 1234 中文',status:'fact',confirmed:true,observerId:null,source:'Author-approved synthetic fact',eventId:null,untilEventId:null};
 const event=(id,title,day,scene)=>({id,title,notes:'Authored occurrence',laneIds:['main-lane'],sceneIds:[scene],time:{kind:'fixed',day,label:'Day '+day}});
 const planning={schemaVersion:1,fields:[],records,plotPoints:[],facts:[fact],boards:[],lanes:[{id:'main-lane',name:'Main plot',kind:'main'}],events:[event('early','Earlier story event',1,second.id),event('late','Later story event',9,first.id)],constraints:[],issues:[],readerReveals:[{id:'reveal-first',eventId:'late',factId:null,scene:{kind:'scene',id:first.id},position:0,notes:'Reader sees late event first'},{id:'reveal-second',eventId:'early',factId:null,scene:{kind:'scene',id:second.id},position:0,notes:'Reader sees earlier event later'}],knowledge:[{id:'knowledge',characterId:records[0].id,factId:fact.id,stance:'knows',factSnapshot:{subjectId:fact.subjectId,property:fact.property,value:fact.value},interpretation:'',source:'Explicit author scene scope',scope:{kind:'scenes',sceneIds:[first.id]}}]};
 const old=await rpc('load_project_data',{projectId:project.id,kind:'planning'}),savedBible=await rpc('save_project_data',{projectId:project.id,kind:'planning',payload:planning,parent:old?.revisionId??null});
 const points=await rpc('project_time_points',{projectId:project.id,beforeSequence:null,limit:100}),point=points.points.find(item=>item.revisionId===savedBible.revisionId);
 check(Boolean(point),'Native history exposes the authored bible revision as a recorded point');
 report.stage='Read-only references';click('.editor-navigation button','Reference');await ready(()=>ui('.scene-reference-pane')?.textContent.includes(records[0].name));
 check(ui('.scene-reference-pane').textContent.includes(records[0].name)&&!ui('.scene-reference-pane').textContent.includes(records[1].notes),'References derive from explicit links for the active scene');
 const originalSurface=ui('.native-scroll');click('.scene-reference-pane button','Pin scene');
 click('.scene-reference-pane .planning-tabs button','Facts');await ready(()=>ui('.scene-reference-pane').textContent.includes(fact.value));
 check(ui('.scene-reference-pane').textContent.includes('Approved fact'),'References distinguish approved facts');
 click('.scene-reference-pane .planning-tabs button','Knowledge');await ready(()=>ui('.scene-reference-pane').textContent.includes(fact.value));
 check(ui('.scene-reference-pane').textContent.includes('knows')&&ui('.scene-reference-pane').textContent.includes('Applies'),'Explicit scene knowledge appears in the reference pane');
 check(ui('.native-scroll')===originalSurface,'Opening and switching references preserves the existing native editor');
 const pinned=ui('[aria-label="Reference scene"]');check(pinned.value===first.id,'Pinned reference scene is visibly selected');
 // Native binder selectors differ across tree modes; use the acknowledged scene button label.
 const referenceTab=document.querySelector('[role=tab][aria-selected=true]');
 select(ui('[aria-label="Reference scene"]'),second.id);await ready(()=>ui('.scene-reference-title')?.textContent===second.scene.title);
 const secondButton=all('.manuscript-tree button').find(node=>node.textContent.includes(second.scene.title));
 check(Boolean(secondButton),'Second scene remains accessible in the manuscript binder');
 secondButton.click();await ready(()=>all('.manuscript-tree button').some(button=>button.textContent===second.scene.title&&button.getAttribute('aria-current')==='page')&&ui('.native-input')&&!ui('.toolbar [aria-label="Bold"]').disabled);
 check(!ui('.scene-reference-pane'),'A separate draft tab has independent reference controls');
 referenceTab.click();await ready(()=>ui('[aria-label="Reference scene"]')?.value===second.id);
 check(ui('.scene-reference-title').textContent===second.scene.title,'Pinned references survive navigation to another draft tab and back');
 click('.scene-reference-pane button','Follow current scene');await ready(()=>ui('.scene-reference-title')?.textContent===first.scene.title);
 check(ui('[aria-label="Reference scene"]').value==='','Unpinning resumes the active manuscript scene');
 checkContrast('.scene-reference-pane button,.scene-reference-pane select,.scene-reference-pane p,.scene-reference-pane small,.scene-reference-pane label span');check(true,'Reference labels and controls remain readable in light, dark and system themes');
 click('.scene-reference-pane button','Close');await ready(()=>!ui('.scene-reference-pane'));
 report.stage='Separate timelines';[...all('.workspace-nav button')].find(button=>button.textContent==='Planning').click();await ready(()=>ui('[aria-label="Planning destinations"]'));
 click('[aria-label="Planning destinations"] button','Chronology');await ready(()=>ui('[aria-label="Timeline order"]'));
 const story=ui('.planning-panel').textContent;check(story.indexOf('Earlier story event')<story.indexOf('Later story event'),'Story timeline orders the fictional occurrence dates');
 click('[aria-label="Timeline order"] button','Reader learns');await ready(()=>ui('.planning-reader-timeline'));
 const reader=ui('.planning-reader-timeline').textContent;check(reader.indexOf('Later story event')<reader.indexOf('Earlier story event'),'Reader timeline follows manuscript reveals independently of story dates');
 const before=(await rpc('load_project_data',{projectId:project.id,kind:'planning'}));check(equal(before.payload,planning),'Read-only references and timeline switches preserve the exact planning revision');
 report.stage='Past manuscript and bible';[...all('.workspace-nav button')].find(button=>button.textContent==='Project').click();await ready(()=>ui('.project-time-machine select')?.options.length>1);
 const changed=structuredClone(planning);changed.records[0].notes='Newer exact notes';const changedBible=await rpc('save_project_data',{projectId:project.id,kind:'planning',payload:changed,parent:savedBible.revisionId});
 click('.project-time-machine button','Reload history');await ready(()=>[...ui('.project-time-machine select').options].some(option=>option.value===String(point.sequence)));
 select(ui('.project-time-machine select'),String(point.sequence));await ready(()=>ui('.time-machine-bible')?.textContent.includes(records[0].notes)&&ui('.project-time-machine').getAttribute('aria-busy')==='false'&&ui('.project-time-machine select').value===String(point.sequence));
 check(ui('.time-machine-manuscript')?.textContent.includes('سلام مِمِمِ 中文'),'History previews the exact multilingual manuscript at the same point as the old bible');
 check(ui('.time-machine-bible').textContent.includes(records[0].notes)&&!ui('.time-machine-bible').textContent.includes('Newer exact notes'),'History displays past bible content instead of the current projection');
 const detail=all('.project-time-machine details').find(node=>node.querySelector('summary')?.textContent==='Recover the whole story bible');detail.open=true;
 click('.project-time-machine button','Recover this whole story bible');await ready(()=>ui('.project-time-machine [role=status]')?.textContent.includes('Recovered')&&ui('.project-time-machine').getAttribute('aria-busy')==='false');check(!ui('.project-time-machine [role=alert]'),'Recovery refresh completed without a hidden saved-but-refresh-failed error');
 const recovered=await rpc('load_project_data',{projectId:project.id,kind:'planning'});
 check(recovered.revisionId!==savedBible.revisionId&&recovered.revisionId!==changedBible.revisionId&&equal(recovered.payload,planning),'Selected historical bible is recovered exactly as a new revision');
 const newer=await rpc('load_entity_revision',{projectId:project.id,entityId:project.id+':planning',entityType:'projectData',revisionId:changedBible.revisionId});check(equal(newer.payload,changed),'Recovery retains the newer bible revision in immutable history');
 const imported=await rpc('import_archive',{archive:await rpc('export_archive',{projectId:project.id})}),importedBible=await rpc('load_project_data',{projectId:imported.project.id,kind:'planning'});
 check(importedBible.payload.readerReveals[0].scene.id===imported.scenes[0].id&&importedBible.payload.knowledge[0].scope.sceneIds[0]===imported.scenes[0].id,'Archive import remaps reveal and knowledge scene references');
 check(importedBible.payload.knowledge[0].factSnapshot.value===fact.value&&importedBible.payload.readerReveals[0].notes===planning.readerReveals[0].notes,'Archive import preserves literal multilingual facts and reveal notes');
 report.stage='New pane and multiple-tab contrast';
 checkContrast('.window-tabs [role=tab],.project-time-machine select,.time-machine-explanation');check(true,'Active/inactive tabs and time-machine controls remain readable in light, dark and system themes');
 ui('.project-time-machine').scrollIntoView({block:'start'});
 report.savedRevision=(await rpc('load_scene',{sceneId:first.id})).revisionId;
}catch(error){report.error=String(error);report.alerts=[...document.querySelectorAll('[role=alert]')].map(node=>node.textContent);report.visibleHeading=ui('.workspace-nav button[aria-pressed=true]')?.textContent;report.machineBusy=ui('.project-time-machine')?.getAttribute('aria-busy');report.machineButtons=all('.project-time-machine button').map(button=>({label:button.textContent,disabled:button.disabled}));report.machineStatuses=all('.project-time-machine [role=status]').map(node=>node.textContent);}
setTimeout(()=>void invoke('workspace_request_quit'),1000);return JSON.stringify(report);
