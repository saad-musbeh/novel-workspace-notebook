// External UI probe for profile persistence and native Word export. Run only
// against an extracted release seeded with synthetic project data and a private
// NOVEL_FILE_PROBE_DIRECTORY.
const report={checks:[]};window.__novelReleaseProgress=report;
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args);
const rpc=(operation,args={})=>invoke('workspace_command',{operation,args});
const scope=()=>document.querySelector('.window-workspace > [role=tabpanel]:not([hidden])')??document;
const ready=async predicate=>{const until=performance.now()+20000;while(performance.now()<until){if(await predicate())return;await sleep(40);}throw new Error('Timed out at '+report.stage);};
const check=(condition,message)=>{if(!condition)throw new Error(message);report.checks.push(message);};
const canonical=value=>Array.isArray(value)?value.map(canonical):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])])):value;
const equal=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
const setValue=(element,value)=>{const proto=element instanceof HTMLSelectElement?HTMLSelectElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(element,String(value));element.dispatchEvent(new Event('input',{bubbles:true}));element.dispatchEvent(new Event('change',{bubbles:true}));};
const setChecked=(element,value)=>{if(element.checked!==Boolean(value))element.click();};
try{
 report.stage='Open synthetic project';await ready(()=>scope().querySelector('.native-input'));
 const projects=await rpc('list_projects');check(projects.length>0,'Synthetic seeded project is available');
 const project=projects[0],initial=await rpc('open_project',{projectId:project.id});
 const sceneHeads=initial.scenes.map(scene=>[scene.id,scene.revisionId]);
 const staleProfile=await rpc('load_project_data',{projectId:project.id,kind:'exportProfile'});
 check(staleProfile===null,'Synthetic project starts with the default Word profile');
 report.stage='Change Word export style controls';
 const projectTab=[...scope().querySelectorAll('.workspace-nav button')].find(button=>button.textContent.trim()==='Project');
 if(projectTab)projectTab.click();
 await ready(()=>scope().querySelector('.export-styles[aria-busy=\"false\"]'));
 const panel=scope().querySelector('.export-styles[aria-busy=\"false\"]');
 const summary=panel.querySelector('summary');if(!panel.open)summary.click();
 const paper=panel.querySelector('.export-page-style select');
 const margins=panel.querySelector('[aria-label="Page margins in millimeters"]');
 const lineSpacing=[...panel.querySelectorAll('.export-page-style select')][1];
 check(Boolean(paper&&margins&&lineSpacing),'Word page and spacing controls are available');
 setValue(paper,'a4');setValue(margins,20);setValue(lineSpacing,'2');
 const values={title:{font:'Aptos Display',size:24,bold:false,italic:false},chapter:{font:'Georgia',size:18,bold:true,italic:false},scene:{font:'Aptos',size:14,bold:false,italic:true},body:{font:'Times New Roman',size:12,bold:false,italic:false}};
 for(const [part,legend] of Object.entries({'title':'Manuscript title','chapter':'Chapter title','scene':'Scene heading','body':'Body text'})){
  const fieldset=[...panel.querySelectorAll('fieldset')].find(field=>field.querySelector('legend')?.textContent.trim()===legend);
  check(Boolean(fieldset),'Style controls are available for '+legend);
  const labels=[...fieldset.querySelectorAll('label')];
  const fontLabel=labels.find(label=>label.textContent.includes('Font name'));
  const sizeLabel=labels.find(label=>label.textContent.includes('Size (pt)'));
  const boldLabel=labels.find(label=>label.textContent.includes('Bold'));
  const italicLabel=labels.find(label=>label.textContent.includes('Italic'));
  setValue(fontLabel.querySelector('input'),values[part].font);
  setValue(sizeLabel.querySelector('input'),values[part].size);
  setChecked(boldLabel.querySelector('input[type=checkbox]'),values[part].bold);
  setChecked(italicLabel.querySelector('input[type=checkbox]'),values[part].italic);
 }
 const expectedProfile={schemaVersion:1,pageSize:'a4',marginMm:20,lineSpacing:'2',...values};
 const activePanel=()=>scope().querySelector('.export-styles[aria-busy="false"]');
 const writeProfileControls=(profile,target)=>{
  setValue(target.querySelector('.export-page-style select'),profile.pageSize);
  setValue(target.querySelector('[aria-label="Page margins in millimeters"]'),profile.marginMm);
  setValue([...target.querySelectorAll('.export-page-style select')][1],profile.lineSpacing);
  for(const [part,legend] of Object.entries({'title':'Manuscript title','chapter':'Chapter title','scene':'Scene heading','body':'Body text'})){
   const fieldset=[...target.querySelectorAll('fieldset')].find(field=>field.querySelector('legend')?.textContent.trim()===legend);
   const labels=[...fieldset.querySelectorAll('label')];
   setValue(labels.find(label=>label.textContent.includes('Font name')).querySelector('input'),profile[part].font);
   setValue(labels.find(label=>label.textContent.includes('Size (pt)')).querySelector('input'),profile[part].size);
   setChecked(labels.find(label=>label.textContent.includes('Bold')).querySelector('input[type=checkbox]'),profile[part].bold);
   setChecked(labels.find(label=>label.textContent.includes('Italic')).querySelector('input[type=checkbox]'),profile[part].italic);
  }
 };
 const save=[...panel.querySelectorAll('button')].find(button=>button.textContent.trim()==='Save Word export styles');
 await ready(()=>save&&!save.disabled);
 report.stage='Save profile';save.click();
 await ready(async()=>{const saved=await rpc('load_project_data',{projectId:project.id,kind:'exportProfile'});return saved&&equal(saved.payload,expectedProfile);});
 const saved=await rpc('load_project_data',{projectId:project.id,kind:'exportProfile'});
 check(equal(saved.payload,expectedProfile),'Accessible style controls saved the requested A4, spacing, and four style settings');
 report.profile= saved.payload;report.initialProfileRevisionId=saved.revisionId;
 report.stage='Confirm saved-writing heads';
 const afterProfile=await rpc('open_project',{projectId:project.id});
 check(equal(afterProfile.scenes.map(scene=>[scene.id,scene.revisionId]),sceneHeads),'Saving export styles did not create manuscript scene revisions');
 report.stage='Refuse stale export profile';
 const competingProfile=structuredClone(expectedProfile);competingProfile.title.size=25;
 const competing=await rpc('save_project_data',{projectId:project.id,kind:'exportProfile',payload:competingProfile,parent:saved.revisionId});
 await ready(()=>[...scope().querySelectorAll('button')].some(button=>button.textContent.trim()==='Export Word (.docx)'&&!button.disabled));
 const exportButton=[...scope().querySelectorAll('button')].find(button=>button.textContent.trim()==='Export Word (.docx)');
 exportButton.click();
 await ready(()=>[...scope().querySelectorAll('[role=alert]')].some(node=>/Word export styles changed\. Reload the saved styles before exporting\./i.test(node.textContent)));
 const staleError=[...scope().querySelectorAll('[role=alert]')].map(node=>node.textContent).find(text=>/Word export styles changed\. Reload the saved styles before exporting\./i.test(text));
 check(Boolean(staleError),'Native export refused a stale Word style profile revision');report.staleRefused=true;report.staleError=staleError;
 report.stage='Reload latest profile and restore requested styles';
 const reload=[...activePanel().querySelectorAll('button')].find(button=>button.textContent.trim()==='Reload saved styles');
 check(Boolean(reload),'Reload saved styles control is available after a stale export refusal');reload.click();
 await ready(async()=>{
  const current=await rpc('load_project_data',{projectId:project.id,kind:'exportProfile'}),target=activePanel();
  const titleField=[...(target?.querySelectorAll('fieldset')??[])].find(field=>field.querySelector('legend')?.textContent.trim()==='Manuscript title');
  const size=titleField?.querySelector('label input[type=number]');
  return target&&scope().querySelectorAll('.export-styles').length===1&&current?.revisionId===competing.revisionId&&size?.value==='25';
 });
 const reloadedPanel=activePanel();
 check(scope().querySelectorAll('.export-styles').length===1,'Reload leaves exactly one Word export style panel mounted');
 writeProfileControls(expectedProfile,reloadedPanel);
 await ready(()=>{const button=[...activePanel().querySelectorAll('button')].find(item=>item.textContent.trim()==='Save Word export styles');return button&&!button.disabled;});
 [...activePanel().querySelectorAll('button')].find(button=>button.textContent.trim()==='Save Word export styles').click();
 await ready(async()=>{const current=await rpc('load_project_data',{projectId:project.id,kind:'exportProfile'});return current&&equal(current.payload,expectedProfile)&&current.revisionId!==competing.revisionId;});
 const finalProfile=await rpc('load_project_data',{projectId:project.id,kind:'exportProfile'});
 check(equal(finalProfile.payload,expectedProfile),'Reloaded profile controls saved the final requested layout');
 report.profile=finalProfile.payload;report.profileRevisionId=finalProfile.revisionId;report.reloadedProfileRevisionId=competing.revisionId;
 report.stage='Export Word copy with the current profile head';
 const currentExportButton=[...scope().querySelectorAll('button')].find(button=>button.textContent.trim()==='Export Word (.docx)');
 check(Boolean(currentExportButton&&!currentExportButton.disabled),'Export action is enabled after profile save');
 currentExportButton.click();
 await ready(()=>[...scope().querySelectorAll('[role=status],[role=alert]')].some(node=>/Word manuscript exported|error|failed/i.test(node.textContent)));
 const notice=[...scope().querySelectorAll('[role=status],[role=alert]')].map(node=>node.textContent).find(text=>/Word manuscript exported/i.test(text));
 check(Boolean(notice),'Native Word export completed with the saved profile');report.receipt=notice;
 const afterExport=await rpc('open_project',{projectId:project.id});
 check(equal(afterExport.scenes.map(scene=>[scene.id,scene.revisionId]),sceneHeads),'Export did not add or rewrite manuscript revisions');
 report.sceneHeadsBefore=sceneHeads;report.sceneHeadsAfter=afterExport.scenes.map(scene=>[scene.id,scene.revisionId]);
 report.bundle=afterExport;report.outputName='synthetic-export.docx';
 const finalPanel=scope().querySelector('.export-styles');finalPanel.open=true;finalPanel.scrollIntoView({block:'start'});await sleep(200);
 check(finalPanel.scrollWidth<=finalPanel.clientWidth+1,'Style controls fit the desktop panel without horizontal overflow');
}catch(error){report.error=String(error);report.alerts=[...document.querySelectorAll('[role=alert]')].map(node=>node.textContent);report.panels=[...document.querySelectorAll('.export-styles')].map(n=>({connected:n.isConnected,busy:n.getAttribute('aria-busy'),parent:n.closest('[role=tabpanel]')?.id,hidden:n.closest('[role=tabpanel]')?.hidden,html:n.outerHTML.slice(0,1800)}));}
setTimeout(()=>void invoke('workspace_request_quit'),500);
return JSON.stringify(report);
