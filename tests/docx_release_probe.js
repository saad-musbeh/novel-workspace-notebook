// External probe for a real native Word export against an isolated synthetic project.
const report={checks:[]};window.__novelReleaseProgress=report;
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args);
const rpc=(operation,args={})=>invoke('workspace_command',{operation,args});
const ready=async predicate=>{const deadline=performance.now()+20000;while(performance.now()<deadline){if(predicate())return;await sleep(40);}throw new Error('DOCX integration timed out at '+report.stage);};
const check=(condition,message)=>{if(!condition)throw new Error(message);report.checks.push(message);};
const scope=()=>document.querySelector('.window-workspace > [role=tabpanel]:not([hidden])')??document;
const text=document=>document.content.map(block=>(block.content??[]).map(node=>node.text??'').join('')).join('\n\n');
const archiveData=value=>JSON.parse(value.entries.find(entry=>entry.path==='project.json').content);
try {
 report.stage='Load synthetic project';
 await ready(()=>scope().querySelector('.native-input'));
 const projects=await rpc('list_projects');
 check(projects.length===1&&projects[0].title==='Synthetic Word Export Integration','Only the synthetic project is open in the isolated store');
 const project=projects[0],bundle=await rpc('open_project',{projectId:project.id});
 const scene=bundle.scenes[0],before=await rpc('load_scene',{sceneId:scene.id});
 const originalText=text(before.scene.document);
 report.projectId=project.id;report.sceneId=scene.id;report.beforeRevision=before.revisionId;
 const archiveBefore=archiveData(JSON.parse(await rpc('export_archive',{projectId:project.id})));
 const oldRevisionIds=(archiveBefore.revisions??[]).map(revision=>revision.id);
 check(oldRevisionIds.includes(before.revisionId),'Initial manuscript revision is present in the project archive');

 report.stage='Stage unsaved manuscript edit';
 const sink=scope().querySelector('.native-input');
 check(Boolean(sink),'Native manuscript editor is mounted');
 sink.focus();sink.dispatchEvent(new KeyboardEvent('keydown',{key:'End',code:'End',ctrlKey:true,bubbles:true,cancelable:true}));
 const setter=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set;
 const suffix=' Pending Word export drain مِ 中文 👩🏽‍🚀';
 setter.call(sink,suffix);sink.dispatchEvent(new Event('input',{bubbles:true}));
 report.expectedEditedText=originalText+suffix;
 report.stage='Open Project export controls';
 const projectButton=[...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent.trim()==='Project');
 check(Boolean(projectButton),'Project workspace control is available');
 projectButton.click();
 await ready(()=>[...scope().querySelectorAll('button')].some(button=>button.textContent.trim()==='Export Word (.docx)'));
 const exportButton=[...scope().querySelectorAll('button')].find(button=>button.textContent.trim()==='Export Word (.docx)');
 check(Boolean(exportButton),'Clearly labelled Word export action is visible');

 report.stage='Invoke native project export and save barrier';
 const started=performance.now();exportButton.click();
 await ready(()=>[...scope().querySelectorAll('[role=alert],.workspace-notice')].some(node=>/Word manuscript exported|error|failed/i.test(node.textContent)));
 const notice=[...scope().querySelectorAll('.workspace-notice')].map(node=>node.textContent).find(value=>/Word manuscript exported/i.test(value));
 check(Boolean(notice),'Native export completed after project-wide save acknowledgement');
 report.exportMilliseconds=performance.now()-started;report.receipt=notice;
 check(report.exportMilliseconds<25000,'Project-wide export acknowledgement completed without timeout');

 report.stage='Verify saved snapshot and retained archive history';
 const saved=await rpc('load_scene',{sceneId:scene.id});
 const savedBundle=await rpc('open_project',{projectId:project.id});
 const archiveAfter=archiveData(JSON.parse(await rpc('export_archive',{projectId:project.id})));
 const newRevisionIds=new Set((archiveAfter.revisions??[]).map(revision=>revision.id));
 check(saved.revisionId!==before.revisionId,'Pending editor text was saved as a new immutable revision before export');
 check(text(saved.scene.document)===report.expectedEditedText,'The saved project snapshot contains the exact pending multilingual edit');
 check(oldRevisionIds.every(id=>newRevisionIds.has(id)),'DOCX export preserves every pre-existing project history revision');
 check((archiveAfter.revisions??[]).length>=(archiveBefore.revisions??[]).length,'Project archive history remains available after manuscript export');
 report.savedRevision=saved.revisionId;report.bundle=savedBundle;report.outputName='synthetic-export.docx';
 report.archiveRevisionCountBefore=oldRevisionIds.length;report.archiveRevisionCountAfter=(archiveAfter.revisions??[]).length;
} catch(error){report.error=String(error);report.alerts=[...document.querySelectorAll('[role=alert]')].map(node=>node.textContent);}
setTimeout(()=>void invoke('workspace_request_quit'),1500);
return JSON.stringify(report);
