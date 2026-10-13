import {beforeEach,afterEach,describe,expect,it,vi} from 'vitest';
import type {Scene,Snapshot} from '../apps/writer/src/document';
import type {ProjectBundle} from '../apps/writer/src/workspace';
import type {DesktopWorkspaceProps} from '../apps/writer/src/DesktopWorkspace';
import type {WritingSystemsDocument} from '../apps/writer/src/writing-systems/model';

// A deterministic hook adapter exercises coordinator callbacks without a browser/native editor.
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[],dirty:false}));
const native=vi.hoisted(()=>({command:vi.fn(),save:vi.fn(),alphabetLoad:vi.fn(),alphabetSave:vi.fn()}));
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react');
 const changed=(a:any[],b:any[])=>!a||!b||a.length!==b.length||a.some((v,i)=>!Object.is(v,b[i]));
 const memo=(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;};
 return {...actual,useMemo:memo,useCallback:(fn:any,deps:any[])=>memo(()=>fn,deps),useRef:(value:any)=>{const i=hooks.cursor++;return hooks.slots[i]??=( {current:value});},useState:(initial:any)=>{const i=hooks.cursor++;const state=hooks.slots[i]??=( {value:typeof initial==='function'?initial():initial});return [state.value,(next:any)=>{const value=typeof next==='function'?next(state.value):next;if(!Object.is(value,state.value)){state.value=value;hooks.dirty=true;}}];},useEffect:(effect:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps)){const entry={deps,cleanup:old?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=effect();});}}};
});
vi.mock('../apps/writer/src/workspace',async()=>({...await vi.importActual<any>('../apps/writer/src/workspace'),workspaceCommand:native.command,sceneRepository:()=>({load:vi.fn(),save:native.save})}));
vi.mock('../apps/writer/src/writing-systems/native',()=>({nativeWritingSystemsAdapter:{load:native.alphabetLoad,save:native.alphabetSave,putAsset:vi.fn(),listAssets:vi.fn()}}));
vi.mock('../apps/writer/src/Writer',()=>({Writer:function Writer(){return null;}}));
import {DesktopWorkspace} from '../apps/writer/src/DesktopWorkspace';

function fixture():ProjectBundle {
 return {chapterNumberStart:1,chapterNumberingRevisionId:null,chapterNumberingConflicted:false,project:{id:'project',title:'Synthetic project',createdAt:'fixture'},chapters:[{id:'chapter',projectId:'project',title:'Chapter',position:0}],scenes:['first','second'].map((id,position)=>({id,chapterId:'chapter',position,revisionId:'head-'+id,scene:{schemaVersion:1,title:id+' نور 中文',document:{type:'doc',content:[{type:'paragraph',attrs:{lang:'ar',dir:'rtl'},content:[{type:'text',text:'مِ 中文 繁體 👩🏽‍🚀 \uE000'}]}]}}}))};
}
let data:ProjectBundle,props:DesktopWorkspaceProps,tree:any;
function render(){hooks.cursor=0;hooks.dirty=false;tree=DesktopWorkspace(props);const effects=hooks.effects.splice(0);for(const effect of effects)effect();return tree;}
function nodes(node:any):any[]{if(!node||typeof node!=='object')return [];if(Array.isArray(node))return node.flatMap(n=>nodes(n));return [node,...nodes(node.props?.children),...nodes(node.props?.deskTools),...nodes(node.props?.deskActions),...nodes(node.props?.pageActions)];}
function component(name:string){return nodes(tree).find(n=>typeof n.type!=='string'&&(n.type?.name===name||n.type?.type?.name===name));}
function content(node:any):string {return Array.isArray(node)?node.map(content).join(''):node==null?'':typeof node==='object'?content(node.props?.children):String(node);}
function button(label:string){return nodes(tree).find(n=>n.type==='button'&&(content(n)===label||n.props?.['aria-label']===label));}
async function settle(predicate:()=>boolean=()=>true){for(let i=0;i<100;i++){await new Promise(resolve=>setTimeout(resolve,0));if(hooks.dirty)render();if(predicate()&&!hooks.dirty){await Promise.resolve();return;}}throw new Error('Synthetic coordinator did not settle');}
async function open(route:DesktopWorkspaceProps['initialRoute'],extra:Partial<DesktopWorkspaceProps>={}) {props={flushRef:{current:null},tabId:'tab',initialRoute:route,onRouteReady:vi.fn(),onContentReady:vi.fn(),onOpenRoute:vi.fn(async()=>{}),onSharedChange:vi.fn(),...extra};render();await settle(()=>Boolean(component('Writer')||component('PlanningWorkspace')||component('NativeAssistanceWorkspace')||nodes(tree).some(n=>n.props?.className==='workspace-panel')||component('NativeWritingSystemsWorkspace')));}
beforeEach(()=>{
 hooks.slots=[];hooks.cursor=0;hooks.effects=[];hooks.dirty=false;vi.clearAllMocks();data=fixture();
 vi.stubGlobal('localStorage',{getItem:()=>null,setItem:vi.fn()});vi.stubGlobal('document',{documentElement:{dataset:{}},addEventListener:vi.fn(),removeEventListener:vi.fn()});vi.stubGlobal('window',{addEventListener:vi.fn(),removeEventListener:vi.fn(),dispatchEvent:vi.fn()});
 native.alphabetLoad.mockResolvedValue({document:{schemaVersion:1,systems:[],assignments:[]},revisionId:null});native.alphabetSave.mockImplementation(async(_pid,document)=>({document,revisionId:'alphabet-new'}));
 native.command.mockImplementation(async(op,args)=>{if(op==='metadata_heads')return {revisionIds:['head-'+args.entityId]};if(op==='runtime_info')return {ipcVersion:1,features:['private-sync']};if(op==='list_projects')return [data.project];if(op==='open_project')return structuredClone(data);if(op==='open_project_index')return {chapterNumberStart:data.chapterNumberStart,chapterNumberingRevisionId:data.chapterNumberingRevisionId,chapterNumberingConflicted:data.chapterNumberingConflicted,project:data.project,chapters:data.chapters,scenes:data.scenes.map(({id,chapterId,position,revisionId,scene})=>({id,chapterId,position,revisionId,title:scene.title}))};if(op==='load_project_scene'){const scene=data.scenes.find(scene=>scene.id===args.sceneId);if(!scene)throw new Error('This scene is no longer in the manuscript.');return structuredClone(scene);}if(op==='load_project_data')return null;if(op==='history'||op==='conflicts')return [];if(op==='save_project_data')return {revisionId:'planning-new',payload:args.payload};throw new Error('Unexpected fixture command '+op);});
 native.save.mockImplementation(async(scene:Scene,parent:string)=>{const record=data.scenes.find(s=>s.id===props.initialRoute?.sceneId)!;if(parent!==record.revisionId)throw new Error('Conflict: synthetic stale parent');record.scene=structuredClone(scene);record.revisionId+='-saved';return {scene:record.scene,revisionId:record.revisionId} satisfies Snapshot;});
});
afterEach(()=>{for(const slot of hooks.slots)slot?.cleanup?.();vi.unstubAllGlobals();});
describe('tab workspace coordinator callbacks',()=>{
 it('gives the Word export style panel a distinct key from Project Recovery',async()=>{
  await open({projectId:'project',view:'library'});
  const styles=component('ExportStyles'),recovery=component('ProjectRecovery');
  expect(styles).toBeDefined();expect(recovery).toBeDefined();
  expect(styles.key).not.toBe(recovery.key);
  expect(styles.key).toMatch(/^export-styles:/);
 });
 it('opens and navigates a thousand-scene Write project through summaries plus one exact document per selection',async()=>{
  const template=fixture().scenes[0];data.scenes=Array.from({length:1000},(_,position)=>({...template,id:'scene-'+position,position,revisionId:'head-'+position,scene:{...template.scene,title:'Scene '+position+' مِ 中文'}}));
  await open({projectId:'project',sceneId:'scene-999',view:'write'},{onOpenRoute:undefined});
  expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(0);expect(native.command.mock.calls.filter(([op])=>op==='open_project_index')).toHaveLength(1);expect(native.command.mock.calls.filter(([op])=>op==='load_project_scene')).toEqual([['load_project_scene',{projectId:'project',sceneId:'scene-999'}]]);
  const writer=component('Writer');expect(writer.props.initial).toEqual({revisionId:'head-999',scene:data.scenes[999].scene});expect(component('ManuscriptTree').props.index.byId.size).toBe(1000);expect(component('ManuscriptTree').props.index.byId.get('scene-1')).not.toHaveProperty('scene');
  const drain=vi.fn(async()=>{});writer.props.flushRef.current=drain;native.command.mockClear();
  await component('ManuscriptTree').props.onOpenScene('scene-0');await settle(()=>component('Writer')?.key!==writer.key);
  expect(drain).toHaveBeenCalledTimes(1);expect(native.command.mock.calls).toEqual([['open_project_index',{projectId:'project'}],['load_project_scene',{projectId:'project',sceneId:'scene-0'}]]);expect(component('Writer').props.initial).toEqual({revisionId:'head-0',scene:data.scenes[0].scene});
 });
 it('creates a durable scene only after the draft drain, then opens its exact canonical route without replacing the source editor',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');let release!:()=>void;
  const drain=vi.fn(()=>new Promise<void>(resolve=>{release=resolve;}));writer.props.flushRef.current=drain;
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>{if(op==='create_scene'){const created={...structuredClone(data.scenes[0]),id:'created',chapterId:args.chapterId,revisionId:'created-head',position:data.scenes.length,scene:args.scene};data.scenes.push(created);return created;}return original(op,args);});native.command.mockClear();
  button('New scene').props.onClick();await settle(()=>drain.mock.calls.length===1);expect(native.command).not.toHaveBeenCalled();expect(props.onOpenRoute).not.toHaveBeenCalled();
  release();await settle(()=>Boolean(vi.mocked(props.onOpenRoute!).mock.calls.length));
  expect(native.command).toHaveBeenCalledWith('create_scene',{chapterId:'chapter',scene:{schemaVersion:1,title:'Untitled scene',document:{type:'doc',content:[{type:'paragraph',attrs:{dir:'auto',lang:'en'},content:[]}]}}});
  expect(props.onOpenRoute).toHaveBeenCalledExactlyOnceWith({projectId:'project',sceneId:'created',view:'write'},'Untitled scene');
  expect(component('Writer').key).toBe(writer.key);expect(native.command.mock.calls.some(([op])=>op==='open_project')).toBe(false);expect(component('ManuscriptTree').props.index.byId.has('created')).toBe(true);
 });
 it.each(['New scene','New chapter'])('refuses %s after a failed draft drain, retaining the mounted exact source without creating objects',async action=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');writer.props.flushRef.current=async()=>{throw new Error('Composition or CAS refused');};native.command.mockClear();
  button(action).props.onClick();await settle(()=>content(tree).includes('Composition or CAS refused'));
  expect(native.command).not.toHaveBeenCalled();expect(props.onOpenRoute).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(writer.props.initial);
 });
 it('creates an empty chapter without remounting the draft, then uses that explicitly selected chapter for the next scene',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');writer.props.flushRef.current=vi.fn(async()=>{});
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>{if(op==='create_chapter'){const chapter={id:'new-chapter',projectId:args.projectId,title:args.title,position:data.chapters.length};data.chapters.push(chapter);return chapter;}if(op==='create_scene'){const created={...structuredClone(data.scenes[0]),id:'new-scene',chapterId:args.chapterId,revisionId:'new-head',scene:args.scene};data.scenes.push(created);return created;}return original(op,args);});native.command.mockClear();
  button('New chapter').props.onClick();await settle(()=>component('ManuscriptTree').props.selectedChapterId==='new-chapter');
  expect(native.command).toHaveBeenCalledWith('create_chapter',{projectId:'project',title:'Chapter 2'});expect(native.command.mock.calls.some(([op])=>op==='create_scene')).toBe(false);expect(props.onOpenRoute).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writer.key);
  button('New scene').props.onClick();await settle(()=>Boolean(vi.mocked(props.onOpenRoute!).mock.calls.length));
  expect(native.command).toHaveBeenCalledWith('create_scene',expect.objectContaining({chapterId:'new-chapter'}));expect(props.onOpenRoute).toHaveBeenCalledWith({projectId:'project',sceneId:'new-scene',view:'write'},'Untitled scene');
 });
 it('edits chapter name and number after draining, preserving exact draft identity and scene membership',async()=>{
  data.chapters.push({id:'later',projectId:'project',title:'Later',position:1});
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),before=JSON.stringify(data.scenes),drain=vi.fn(async()=>{});writer.props.flushRef.current=drain;
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>{if(op==='edit_chapter'){data.chapters.find(chapter=>chapter.id===args.chapterId)!.title=args.title;const ids=data.chapters.map(chapter=>chapter.id).filter(id=>id!==args.chapterId);ids.splice(args.number-1,0,args.chapterId);data.chapters=ids.map((id:string,position:number)=>({...data.chapters.find(chapter=>chapter.id===id)!,position}));return null;}return original(op,args);});
  component('ManuscriptTree').props.onEditChapter(data.chapters[0]);await settle(()=>Boolean(component('ManuscriptDialog')));native.command.mockClear();
  await component('ManuscriptDialog').props.onSave('بداية 中文',2);await settle();
  expect(drain).toHaveBeenCalledOnce();expect(data.chapters.map(chapter=>[chapter.id,chapter.title,chapter.position])).toEqual([['later','Later',0],['chapter','بداية 中文',1]]);expect(JSON.stringify(data.scenes)).toBe(before);expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(writer.props.initial);expect(native.command.mock.calls.some(([op])=>op==='save_scene')).toBe(false);
 });
 it('saves chapter zero through one checked operation and keeps numbering in navigation and project projections',async()=>{
  data.chapters.push({id:'later',projectId:'project',title:'Later',position:1});
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),before=JSON.stringify(data.scenes);writer.props.flushRef.current=vi.fn(async()=>{});
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>{if(op==='edit_chapter'){data.chapterNumberStart=0;data.chapterNumberingRevisionId='numbering-head';data.chapters=[{...data.chapters[1],title:args.title,position:0},{...data.chapters[0],position:1}];return null;}return original(op,args);});
  component('ManuscriptTree').props.onEditChapter(data.chapters[1]);await settle(()=>Boolean(component('ManuscriptDialog')));
  expect(component('ManuscriptDialog').props.chapterNumberStart).toBe(1);
  await component('ManuscriptDialog').props.onSave('Prologue مِ 中文',0);await settle();
  expect(native.command).toHaveBeenCalledWith('edit_chapter',{chapterId:'later',expectedChapterHead:'head-later',expectedProjectHead:'head-project',expectedOrder:['chapter','later'],numberingParent:null,title:'Prologue مِ 中文',number:0});
  expect(component('ManuscriptTree').props.chapterNumberStart).toBe(0);expect(JSON.stringify(data.scenes)).toBe(before);expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(writer.props.initial);
  component('ManuscriptDialog').props.onClose();await settle();component('ManuscriptTree').props.onEditChapter(data.chapters[0]);await settle(()=>Boolean(component('ManuscriptDialog')));
  expect(component('ManuscriptDialog').props.chapterNumber).toBe(0);expect(component('ManuscriptDialog').props.chapterNumberStart).toBe(0);
 });
 it.each(['drain','stale'] as const)('rejects chapter editing on %s failure without changing metadata or draft',async failure=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');writer.props.flushRef.current=async()=>{if(failure==='drain')throw new Error('Draft drain failed');};component('ManuscriptTree').props.onEditChapter({...data.chapters[0]});await settle(()=>Boolean(component('ManuscriptDialog')));if(failure==='stale')data.chapters[0].title='Another window';native.command.mockClear();
  await expect(component('ManuscriptDialog').props.onSave('Replace',1)).rejects.toThrow(failure==='drain'?'Draft drain failed':'Chapters changed');await settle();expect(native.command.mock.calls.some(([op])=>op==='edit_chapter')).toBe(false);expect(component('Writer').key).toBe(writer.key);expect(component('ManuscriptDialog')).toBeDefined();
 });
 it('opens the library without draining or replacing a draft, then refuses universe creation after a failed drain',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),drain=vi.fn(async()=>{throw new Error('Unsaved draft conflict');});writer.props.flushRef.current=drain;native.command.mockClear();button('New novel').props.onClick();await settle();expect(component('NovelLibrary')).toBeDefined();expect(drain).not.toHaveBeenCalled();
  await expect(component('NovelLibrary').props.onCreateUniverse('Shared world')).rejects.toThrow('Unsaved draft conflict');expect(native.command).not.toHaveBeenCalled();expect(props.onOpenRoute).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writer.key);
 });
 it('opens the exact atomic universe-novel receipt without replacing the source or rediscovering a mutable first scene',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');writer.props.flushRef.current=vi.fn(async()=>{});button('New novel').props.onClick();await settle();const original=native.command.getMockImplementation()!;const project={id:'novel-new',title:'رواية 中文',createdAt:'fixture'},scene={...structuredClone(data.scenes[0]),id:'receipt-scene',chapterId:'receipt-chapter'};
  native.command.mockImplementation(async(op,args)=>op==='create_universe_novel'?{project,scene,catalogueRevisionId:'new-catalogue'}:original(op,args));native.command.mockClear();
  const entry={project:{id:'world',title:'Universe',createdAt:'fixture'},catalogue:{schemaVersion:1,series:[{id:'series',title:'Series'}],novels:[]},revisionId:'catalogue-head'};
  await component('NovelLibrary').props.onCreateNovel(entry,'series',project.title);await settle();expect(native.command).toHaveBeenCalledWith('create_universe_novel',{universeId:'world',seriesId:'series',title:project.title,parent:'catalogue-head'});expect(props.onOpenRoute).toHaveBeenCalledWith({projectId:project.id,sceneId:scene.id,view:'write'},scene.scene.title);expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(writer.props.initial);
 });
 it('delegates navigation using summaries without reading the destination document in the source tab',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});component('Writer').props.flushRef.current=vi.fn(async()=>{});native.command.mockClear();
  await component('ManuscriptTree').props.onOpenScene('first');await settle();
  expect(native.command.mock.calls).toEqual([['open_project_index',{projectId:'project'}]]);expect(props.onOpenRoute).toHaveBeenCalledWith({projectId:'project',sceneId:'first',view:'write'},'first نور 中文');expect(component('Writer').props.initial.revisionId).toBe('head-second');
 });
 it.each(['navigation','refresh'] as const)('keeps the mounted draft and unsaved title when the scoped %s read fails after drain',async(action)=>{
  await open({projectId:'project',sceneId:'second',view:'write'},{onOpenRoute:undefined});const writer=component('Writer');writer.props.onTitle('Pending title مِ 中文');await settle();writer.props.flushRef.current=vi.fn(async()=>{});
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>{if(op==='load_project_scene')throw new Error('Synthetic scoped scene read failed');return original(op,args);});native.command.mockClear();
  if(action==='navigation')await component('ManuscriptTree').props.onOpenScene('first');else{props={...props,sharedChangeVersion:1};render();await settle();button('Refresh').props.onClick();}
  await settle(()=>content(tree).includes('Synthetic scoped scene read failed'));
  expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(writer.props.initial);expect(component('ManuscriptTree').props.index.byId.get('second').title).toBe('Pending title مِ 中文');expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(0);expect(native.save).not.toHaveBeenCalled();expect(props.onSharedChange).not.toHaveBeenCalled();if(action==='refresh')expect(button('Refresh')).toBeDefined();
 });
 it('opens the writing inspector on demand without full manuscript reads or editor remount',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),drain=vi.fn(async()=>{});writer.props.flushRef.current=drain;
  expect(component('SceneDetails')).toBeUndefined();expect(native.command.mock.calls.some(([op,args])=>op==='load_project_data'&&args.kind==='planning')).toBe(false);
  button('Scene inspector').props.onClick();await settle(()=>Boolean(component('SceneDetails')));
  expect(drain).toHaveBeenCalledOnce();expect(component('SceneDetails').props.sceneId).toBe('second');expect(component('SceneDetails').props.expanded).toBe(true);expect(component('Writer').key).toBe(writer.key);expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(0);
  expect(native.command).toHaveBeenCalledWith('load_project_data',{projectId:'project',kind:'planning'});
 });
 it.each([false,true])('opens read-only Reference in compact=%s without drain, manuscript reads or writer remount, then closes it safely',async compact=>{
  await open({projectId:'project',sceneId:'second',view:'write'},{compact});
  const writer=component('Writer'),initial=writer.props.initial,drain=vi.fn(async()=>{throw new Error('Reference must not request a writing drain');});writer.props.flushRef.current=drain;
  writer.props.onTitle('Unsaved title مِ 中文');await settle();native.command.mockClear();native.save.mockClear();
  const toggle=button('Reference');expect(toggle).toBeDefined();expect(toggle.props['aria-expanded']).toBe(false);
  toggle.props.onClick();await settle(()=>Boolean(component('SceneReferencePane')?.props.document));
  const reference=component('SceneReferencePane');expect(reference.props.sceneId).toBe('second');expect(reference.props.active).toBe(true);
  expect(native.command.mock.calls).toEqual([['load_project_data',{projectId:'project',kind:'planning'}]]);
  expect(drain).not.toHaveBeenCalled();expect(native.save).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(initial);
  expect(component('SceneDetails')).toBeUndefined();expect(button('Reference').props['aria-expanded']).toBe(true);
  if(!compact)expect(component('ManuscriptTree').props.index.byId.get('second').title).toBe('Unsaved title مِ 中文');
  native.command.mockClear();reference.props.onClose();await settle(()=>!component('SceneReferencePane'));
  expect(component('Writer').key).toBe(writer.key);expect(component('Writer').props.initial).toBe(initial);expect(native.command).not.toHaveBeenCalled();expect(drain).not.toHaveBeenCalled();expect(native.save).not.toHaveBeenCalled();expect(button('Reference').props['aria-expanded']).toBe(false);
 });
 it('does not publish an obsolete reference load after its panel closes',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),original=native.command.getMockImplementation()!;let release!:(value:null)=>void;
  native.command.mockImplementation(async(op,args)=>op==='load_project_data'?new Promise<null>(resolve=>{release=resolve;}):original(op,args));
  button('Reference').props.onClick();await settle(()=>Boolean(release));expect(component('SceneReferencePane')).toBeDefined();
  component('SceneReferencePane').props.onClose();await settle(()=>!component('SceneReferencePane'));release(null);await settle();
  expect(component('SceneReferencePane')).toBeUndefined();expect(component('Writer').key).toBe(writer.key);expect(native.save).not.toHaveBeenCalled();
 });
 it('Time Machine uses the supplied project-wide recovery barrier and propagates its rejection',async()=>{
  const barrier=vi.fn(async()=>{throw new Error('Sibling draft cannot checkpoint');});
  await open({projectId:'project',sceneId:'second',view:'library'},{beforeProjectRecover:barrier});
  const local=vi.fn(async()=>{});component('SceneDetails').props.onRegisterFlush(local);native.command.mockClear();
  await expect(component('ProjectTimeMachine').props.beforeRecover()).rejects.toThrow('Sibling draft cannot checkpoint');
  expect(barrier).toHaveBeenCalledExactlyOnceWith('project');expect(local).not.toHaveBeenCalled();expect(native.command).not.toHaveBeenCalled();expect(native.save).not.toHaveBeenCalled();
 });
 it('Time Machine falls back to the local pending controller drain when no project barrier is supplied',async()=>{
  await open({projectId:'project',sceneId:'second',view:'library'});const local=vi.fn(async()=>{});component('SceneDetails').props.onRegisterFlush(local);
  await component('ProjectTimeMachine').props.beforeRecover();expect(local).toHaveBeenCalledOnce();
 });
 it('Time Machine rejects completion when its saved recovery cannot refresh the exact project',async()=>{
  await open({projectId:'project',sceneId:'second',view:'library'});const timeMachine=component('ProjectTimeMachine'),details=component('SceneDetails'),original=native.command.getMockImplementation()!;
  data.scenes[1].revisionId='saved-recovery-head';data.scenes[1].scene.title='Recovered مِ 中文';
  native.command.mockImplementation(async(op,args)=>{if(op==='open_project')throw new Error('Recovered project read unavailable');return original(op,args);});
  await expect(timeMachine.props.onRecovered()).rejects.toThrow('Recovery was saved, but the workspace could not refresh');await settle();
  expect(props.onSharedChange).toHaveBeenCalledWith('project');expect(content(tree)).toContain('Recovered project read unavailable');expect(component('SceneDetails').key).toBe(details.key);expect(native.save).not.toHaveBeenCalled();
  native.command.mockImplementation(original);await component('ProjectTimeMachine').props.onRecovered();await settle(()=>content(tree).includes('Recovered مِ 中文'));
  expect(component('SceneDetails').key).not.toBe(details.key);expect(data.scenes[1].revisionId).toBe('saved-recovery-head');
 });
 it('refreshes a writing draft without loading planning or history while the inspector is closed',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});props={...props,sharedChangeVersion:1};render();await settle(()=>Boolean(button('Refresh')));native.command.mockClear();button('Refresh').props.onClick();await settle(()=>!button('Refresh'));
  expect(native.command.mock.calls.some(([op,args])=>op==='open_project'||op==='history'||op==='conflicts'||op==='load_project_data'&&args.kind==='planning')).toBe(false);expect(component('SceneDetails')).toBeUndefined();expect(component('Writer').props.initial.revisionId).toBe('head-second');
 });
 it('keeps the inspector and exact metadata controller when its close drain fails',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});button('Scene inspector').props.onClick();await settle(()=>Boolean(component('SceneDetails')));const details=component('SceneDetails'),writer=component('Writer');
  details.props.onRegisterFlush(async()=>{throw new Error('Synthetic metadata conflict preserves notes');});button('Close').props.onClick();await settle();
  expect(component('SceneDetails').key).toBe(details.key);expect(component('Writer').key).toBe(writer.key);expect(content(tree)).toContain('metadata conflict preserves notes');expect(button('Scene inspector').props['aria-expanded']).toBe(true);
 });
 it('refuses inspector initialization on planning read failure and preserves the writing session',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),original=native.command.getMockImplementation()!;
  native.command.mockImplementation(async(op,args)=>{if(op==='load_project_data'&&args.kind==='planning')throw new Error('Synthetic inspector planning failure');return original(op,args);});button('Scene inspector').props.onClick();await settle(()=>content(tree).includes('Synthetic inspector planning failure'));
  expect(component('SceneDetails')).toBeUndefined();expect(component('Writer').key).toBe(writer.key);expect(native.save).not.toHaveBeenCalled();
 });
 it('mounts a compact secondary draft without duplicate navigation or metadata controllers',async()=>{
  await open({projectId:'project',sceneId:'first',view:'write'},{compact:true,keyboardActive:false,tabActive:true});
  expect(component('Writer').props.tabActive).toBe(true);expect(component('ManuscriptTree')).toBeUndefined();expect(component('SceneDetails')).toBeUndefined();expect(button('Scene inspector')).toBeUndefined();expect(nodes(tree).some(node=>node.props?.className==='topbar')).toBe(false);expect(component('Writer').props.initial.scene.title).toBe('first نور 中文');
 });
 it('drains before opening a second draft and delegates only the exact scene route/index',async()=>{
  const beside=vi.fn(async()=>{});await open({projectId:'project',sceneId:'second',view:'write'},{onOpenBeside:beside});const writer=component('Writer'),order:string[]=[];writer.props.flushRef.current=async()=>{order.push('drain');};beside.mockImplementation(async()=>{order.push('beside');});native.command.mockClear();
  component('ManuscriptTree').props.onOpenBeside('first');await settle(()=>beside.mock.calls.length>0);
  expect(order).toEqual(['drain','beside']);expect(beside).toHaveBeenCalledWith({projectId:'project',sceneId:'first',view:'write'},'first نور 中文');expect(component('Writer').key).toBe(writer.key);expect(native.command.mock.calls.map(([op])=>op)).toEqual(['open_project_index']);
 });
 it('preserves the existing draft when the open-beside save barrier fails',async()=>{
  const beside=vi.fn(async()=>{});await open({projectId:'project',sceneId:'second',view:'write'},{onOpenBeside:beside});const writer=component('Writer');writer.props.flushRef.current=async()=>{throw new Error('Synthetic pending draft CAS failed');};native.command.mockClear();
  component('ManuscriptTree').props.onOpenBeside('first');await settle(()=>content(tree).includes('Synthetic pending draft CAS failed'));expect(beside).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writer.key);expect(native.command).not.toHaveBeenCalled();
 });
 it('uses the exact loaded head if it advances between the index and scoped document read',async()=>{
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>{if(op==='load_project_scene'){data.scenes[1].revisionId='advanced-head';data.scenes[1].scene.title='Advanced 中文 مِ';}return original(op,args);});
  await open({projectId:'project',sceneId:'second',view:'write'});expect(component('Writer').props.initial.revisionId).toBe('advanced-head');expect(component('ManuscriptTree').props.index.byId.get('second')).toMatchObject({title:'Advanced 中文 مِ',revisionId:'advanced-head'});expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(0);
 });
 it('rejects an incorrectly scoped scene without mounting a replacement document',async()=>{
  const original=native.command.getMockImplementation()!;native.command.mockImplementation(async(op,args)=>op==='load_project_scene'?structuredClone(data.scenes[0]):original(op,args));
  props={flushRef:{current:null},tabId:'tab',initialRoute:{projectId:'project',sceneId:'second',view:'write'},onContentReady:vi.fn()};render();await settle(()=>content(tree).includes('scene changed project or chapter'));expect(component('Writer')).toBeUndefined();expect(props.onContentReady).not.toHaveBeenCalled();
 });
 it('reports failed destination initialization to the handoff host for rollback without readiness or ownership publication',async()=>{
  const failed=vi.fn(),route=vi.fn(),ready=vi.fn(),original=native.command.getMockImplementation()!;
  native.command.mockImplementation(async(op,args)=>{if(op==='load_project_scene')throw new Error('Synthetic destination scoped read failed');return original(op,args);});
  props={flushRef:{current:null},tabId:'incoming-tab',initialRoute:{projectId:'project',sceneId:'second',view:'write'},onContentError:failed,onRouteReady:route,onContentReady:ready};render();await settle(()=>content(tree).includes('Synthetic destination scoped read failed'));
  expect(failed).toHaveBeenCalledExactlyOnceWith('Error: Synthetic destination scoped read failed');expect(route).not.toHaveBeenCalled();expect(ready).not.toHaveBeenCalled();expect(component('Writer')).toBeUndefined();expect(native.save).not.toHaveBeenCalled();
 });
 it('loads the exact requested write scene, forwards its session, and waits for editor readiness',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');
  expect(writer.props.initial.scene.title).toBe('second نور 中文');expect(writer.props.editorSessionId).toBe('tab');expect(writer.props.tabActive).toBe(true);expect(props.onContentReady).not.toHaveBeenCalled();
  expect(props.onRouteReady).toHaveBeenCalledWith({projectId:'project',sceneId:'second',view:'write'},'second نور 中文');writer.props.onEditorReady();expect(props.onContentReady).toHaveBeenCalledTimes(1);
  const ready=writer.props.onEditorReady,notify=vi.fn();props={...props,onContentReady:notify,onRouteReady:vi.fn()};render();expect(component('Writer').props.onEditorReady).toBe(ready);expect(props.onRouteReady).not.toHaveBeenCalled();ready();expect(notify).toHaveBeenCalledTimes(1);
 });
 it.each(['plan','review','library','alphabets'] as const)('loads %s tools without starting a hidden Writer',async view=>{
  await open({projectId:'project',sceneId:'second',view});expect(component('Writer')).toBeUndefined();expect(props.onContentReady).toHaveBeenCalledTimes(1);
  expect(native.command).toHaveBeenCalledWith('load_project_data',{projectId:'project',kind:'planning'});
  if(view==='library'){expect(native.command).toHaveBeenCalledWith('history',{sceneId:'second'});expect(native.command).toHaveBeenCalledWith('conflicts',{sceneId:'second'});}
 });
 it('writing tabs never create duplicate review controllers',async()=>{await open({projectId:'project',sceneId:'second',view:'write'});expect(component('NativeAssistanceWorkspace')).toBeUndefined();props={...props,tabActive:false};render();expect(component('NativeAssistanceWorkspace')).toBeUndefined();});
 it('does not substitute the first scene or report readiness when an exact scene is absent',async()=>{
  props={flushRef:{current:null},tabId:'tab',initialRoute:{projectId:'project',sceneId:'missing',view:'write'},onContentReady:vi.fn(),onRouteReady:vi.fn()};render();await settle(()=>content(tree).includes('no longer in the manuscript'));expect(component('Writer')).toBeUndefined();expect(props.onRouteReady).not.toHaveBeenCalled();expect(props.onContentReady).not.toHaveBeenCalled();
 });
 it('drains the current editor before delegating scene navigation and keeps the current session',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),order:string[]=[];writer.props.flushRef.current=async()=>{order.push('drain');};props.onOpenRoute=vi.fn(async()=>{order.push('delegate');});render();
  await component('ManuscriptTree').props.onOpenScene('first');await settle();expect(order).toEqual(['drain','delegate']);expect(props.onOpenRoute).toHaveBeenCalledWith({projectId:'project',sceneId:'first',view:'write'},'first نور 中文');expect(component('Writer').props.initial.scene.title).toBe('second نور 中文');expect(component('Writer').key).toBe(writer.key);
 });
 it('refuses navigation after a failed CAS drain, preserving the current writer and draft',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');writer.props.flushRef.current=async()=>{throw new Error('Conflict: dirty saved parent changed');};
  await component('ManuscriptTree').props.onOpenScene('first');await settle();expect(props.onOpenRoute).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writer.key);expect(content(tree)).toContain('dirty saved parent changed');
 });
 it('delegates tool navigation after draining without changing the writing tab view',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const flush=vi.fn(async()=>{});component('Writer').props.flushRef.current=flush;button('Planning').props.onClick();await settle(()=>Boolean((props.onOpenRoute as any).mock.calls.length));
  expect(flush).toHaveBeenCalledTimes(1);expect(props.onOpenRoute).toHaveBeenCalledWith({projectId:'project',sceneId:'second',view:'plan'},'Planning · Synthetic project');expect(component('PlanningWorkspace')).toBeUndefined();expect(component('Writer')).toBeDefined();
 });
 it('shows shared changes without reading or replacing the draft, and keeps it when Refresh cannot drain',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),reads=native.command.mock.calls.filter(([op])=>op==='open_project').length;
  props={...props,sharedChangeVersion:1};render();await settle();expect(content(tree)).toContain('Changed in another tab or window.');expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(reads);expect(component('Writer').key).toBe(writer.key);
  writer.props.flushRef.current=async()=>{throw new Error('Conflict: draft retained');};button('Refresh').props.onClick();await settle(()=>content(tree).includes('draft retained'));
  expect(button('Refresh')).toBeDefined();expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(reads);expect(component('Writer').key).toBe(writer.key);
 });
 it('Refresh reads current acknowledged data after draining and explicitly remounts the same scene',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),flush=vi.fn(async()=>{});writer.props.flushRef.current=flush;
  data.scenes[1].scene.title='Received مِ 中文';data.scenes[1].revisionId='remote-head';props={...props,sharedChangeVersion:1};render();await settle();button('Refresh').props.onClick();await settle(()=>component('Writer')?.key!==writer.key);
  expect(flush).toHaveBeenCalledTimes(2);expect(component('Writer').props.initial.revisionId).toBe('remote-head');expect(component('Writer').props.initial.scene.title).toBe('Received مِ 中文');expect(button('Refresh')).toBeUndefined();expect(props.onSharedChange).not.toHaveBeenCalled();
 });
 it('notifies shared saves and updates the tab title only after scene acknowledgement',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer'),registered=(props.onRouteReady as any).mock.calls.length;
  writer.props.onTitle('Dirty title');await settle();expect(props.onSharedChange).not.toHaveBeenCalled();expect(props.onRouteReady).toHaveBeenCalledTimes(registered);
  const saved={...writer.props.initial.scene,title:'Acknowledged مِ 中文'};await writer.props.repository.save(saved,writer.props.initial.revisionId);await settle();expect(props.onSharedChange).toHaveBeenCalledWith('project');expect(props.onRouteReady).toHaveBeenLastCalledWith({projectId:'project',sceneId:'second',view:'write'},'Acknowledged مِ 中文');
 });
 it('keeps empty-project tool tabs valid and refuses a writing route without a scene',async()=>{
  data.scenes=[];await open({projectId:'project',view:'library'});expect(component('Writer')).toBeUndefined();button('Write').props.onClick();await settle(()=>content(tree).includes('Create a scene in Project'));expect(props.onOpenRoute).not.toHaveBeenCalled();expect(props.onRouteReady).toHaveBeenLastCalledWith({projectId:'project',view:'library'},'Project · Synthetic project');
 });
 it('leaves shared changes pending when current reads fail after a successful drain',async()=>{
  await open({projectId:'project',sceneId:'second',view:'write'});const writer=component('Writer');writer.props.flushRef.current=vi.fn(async()=>{});props={...props,sharedChangeVersion:1};render();await settle();native.alphabetLoad.mockRejectedValueOnce(new Error('Synthetic read failure'));
  button('Refresh').props.onClick();await settle(()=>content(tree).includes('Synthetic read failure'));expect(component('Writer').key).toBe(writer.key);expect(button('Refresh')).toBeDefined();expect(component('Writer').props.initial.revisionId).toBe('head-second');
 });
 it('drains pending review controllers after writing before delegating to the host',async()=>{
  await open(undefined,{tabId:undefined,onOpenRoute:undefined});const order:string[]=[];component('Writer').props.flushRef.current=async()=>{order.push('writing');};component('NativeAssistanceWorkspace').props.onRegisterFlush(async()=>{order.push('review');});props={...props,onOpenRoute:vi.fn(async()=>{order.push('host');})};render();await component('ManuscriptTree').props.onOpenScene('second');expect(order).toEqual(['writing','review','host']);
 });
 it('library scene selection waits for the details drain, then loads the exact scene history and conflicts',async()=>{
  await open({projectId:'project',sceneId:'second',view:'library'});
  const original=native.command.getMockImplementation()!,order:string[]=[];
  let release!:()=>void;
  component('SceneDetails').props.onRegisterFlush(async()=>{order.push('drain-start');await new Promise<void>(resolve=>{release=resolve;});order.push('drain-end');});
  const revision=(sceneId:string,id:string)=>({id,projectId:'project',entityId:sceneId,entityType:'scene',parents:[],payload:data.scenes.find(scene=>scene.id===sceneId)!.scene,operation:'synthetic-'+id,checksum:'a'.repeat(64),createdAt:'fixture-'+id,sequence:1});
  native.command.mockImplementation(async(op,args)=>{if(op==='open_project')order.push('read-project');if(op==='history'){order.push('history-'+args.sceneId);return [revision(args.sceneId,'history-'+args.sceneId)];}if(op==='conflicts'){order.push('conflicts-'+args.sceneId);return [revision(args.sceneId,'conflict-'+args.sceneId)];}return original(op,args);});
  data.scenes[0].scene.title='Received first 中文 مِ';data.scenes[0].revisionId='first-current-head';
  const frozen=JSON.stringify(data),oldDetails=component('SceneDetails'),picker=nodes(tree).find(node=>node.props?.className==='tool-scene-picker');
  picker.props.onChange({target:{value:'first'}});await settle(()=>order.includes('drain-start'));
  expect(order).toEqual(['drain-start']);expect(component('SceneDetails').props.sceneId).toBe('second');expect(component('SceneDetails').key).toBe(oldDetails.key);
  release();await settle(()=>component('SceneDetails')?.props.sceneId==='first');
  expect(order).toEqual(['drain-start','drain-end','read-project','history-first','conflicts-first']);
  expect(component('SceneDetails').key).not.toBe(oldDetails.key);expect(nodes(tree).find(node=>node.props?.className==='tool-scene-picker').props.value).toBe('first');
  expect(content(tree)).toContain('Received first 中文 مِ');expect(content(tree)).toContain('synthetic-history-first');expect(content(tree)).toContain('fixture-conflict-first');
  expect(props.onRouteReady).toHaveBeenLastCalledWith({projectId:'project',sceneId:'first',view:'library'},'Project · Synthetic project');
  expect(props.onOpenRoute).not.toHaveBeenCalled();expect(props.onSharedChange).not.toHaveBeenCalled();expect(native.save).not.toHaveBeenCalled();expect(component('Writer')).toBeUndefined();expect(JSON.stringify(data)).toBe(frozen);
 });
 it('review scene selection drains pending review state and switches exact acknowledged source without fetching history',async()=>{
  await open({projectId:'project',sceneId:'second',view:'review'});await settle(()=>component('NativeAssistanceWorkspace')?.props.scene?.sceneId==='second');
  const original=native.command.getMockImplementation()!,order:string[]=[],frozen=JSON.stringify(data);
  component('NativeAssistanceWorkspace').props.onRegisterFlush(async()=>{order.push('review-drained');});
  native.command.mockImplementation(async(op,args)=>{if(op==='open_project')order.push('read-project');return original(op,args);});native.command.mockClear();
  nodes(tree).find(node=>node.props?.className==='tool-scene-picker').props.onChange({target:{value:'first'}});
  await settle(()=>component('NativeAssistanceWorkspace')?.props.scene?.sceneId==='first');
  expect(order).toEqual(['review-drained','read-project']);expect(component('NativeAssistanceWorkspace').props.scene).toMatchObject({sceneId:'first',revisionId:'head-first',text:'مِ 中文 繁體 👩🏽‍🚀 \uE000'});
  expect(props.onRouteReady).toHaveBeenLastCalledWith({projectId:'project',sceneId:'first',view:'review'},'Review · Synthetic project');
  expect(native.command.mock.calls.some(([op])=>op==='history'||op==='conflicts')).toBe(false);expect(props.onOpenRoute).not.toHaveBeenCalled();expect(props.onSharedChange).not.toHaveBeenCalled();expect(component('Writer')).toBeUndefined();expect(JSON.stringify(data)).toBe(frozen);
 });
 it.each(['library','review'] as const)('keeps the exact %s scene and mounted controller when its pending drain fails',async view=>{
  await open({projectId:'project',sceneId:'second',view});
  if(view==='review')await settle(()=>component('NativeAssistanceWorkspace')?.props.scene?.sceneId==='second');
  const controller=component(view==='library'?'SceneDetails':'NativeAssistanceWorkspace'),key=controller.key,frozen=JSON.stringify(data),registrations=(props.onRouteReady as any).mock.calls.length;
  controller.props.onRegisterFlush(async()=>{throw new Error('Conflict: pending '+view+' draft retained');});native.command.mockClear();
  nodes(tree).find(node=>node.props?.className==='tool-scene-picker').props.onChange({target:{value:'first'}});
  await settle(()=>content(tree).includes('pending '+view+' draft retained'));
  expect(nodes(tree).find(node=>node.props?.className==='tool-scene-picker').props.value).toBe('second');expect(component(view==='library'?'SceneDetails':'NativeAssistanceWorkspace').key).toBe(key);
  expect(native.command).not.toHaveBeenCalled();expect(props.onRouteReady).toHaveBeenCalledTimes(registrations);expect(props.onOpenRoute).not.toHaveBeenCalled();expect(props.onSharedChange).not.toHaveBeenCalled();expect(JSON.stringify(data)).toBe(frozen);
  if(view==='review')expect(component('NativeAssistanceWorkspace').props.scene.sceneId).toBe('second');else expect(component('SceneDetails').props.sceneId).toBe('second');
 });
 it('forwards editor content errors to the latest host callback without falsely reporting readiness',async()=>{
  const initialError=vi.fn();await open({projectId:'project',sceneId:'second',view:'write'},{onContentError:initialError});
  const failed=component('Writer').props.onEditorError;
  failed('Synthetic conversion failure 中文 مِ');expect(initialError).toHaveBeenCalledExactlyOnceWith('Synthetic conversion failure 中文 مِ');expect(props.onContentReady).not.toHaveBeenCalled();
  const latestError=vi.fn();props={...props,onContentError:latestError};render();expect(component('Writer').props.onEditorError).toBe(failed);
  failed('Synthetic relocated editor failure');expect(latestError).toHaveBeenCalledExactlyOnceWith('Synthetic relocated editor failure');expect(initialError).toHaveBeenCalledTimes(1);expect(props.onContentReady).not.toHaveBeenCalled();expect(props.onSharedChange).not.toHaveBeenCalled();
 });
 it('keeps the original library scene if the selected scene disappears during the drained read',async()=>{
  await open({projectId:'project',sceneId:'second',view:'library'});const old=component('SceneDetails');old.props.onRegisterFlush(vi.fn(async()=>{}));data.scenes=data.scenes.filter(scene=>scene.id!=='first');native.command.mockClear();
  nodes(tree).find(node=>node.props?.className==='tool-scene-picker').props.onChange({target:{value:'first'}});await settle(()=>content(tree).includes('no longer in the manuscript'));
  expect(component('SceneDetails').props.sceneId).toBe('second');expect(component('SceneDetails').key).toBe(old.key);expect(native.command.mock.calls.some(([op])=>op==='history'||op==='conflicts')).toBe(false);expect(props.onSharedChange).not.toHaveBeenCalled();
 });
 it('does not reload or remount the legacy editor for an unchanged alphabet-load callback',async()=>{
  await open(undefined,{tabId:undefined,onOpenRoute:undefined});button('Alphabets').props.onClick();await settle(()=>Boolean(component('NativeWritingSystemsWorkspace')));
  const writerKey=component('Writer').key,alphabetKey=component('NativeWritingSystemsWorkspace').key,frozen=JSON.stringify(data);
  const loaded=(await native.alphabetLoad.mock.results.at(-1)!.value).document;native.command.mockClear();
  for(let repeat=0;repeat<3;repeat++){component('NativeWritingSystemsWorkspace').props.onAssignmentsChanged(structuredClone(loaded));await settle();}
  expect(native.command).not.toHaveBeenCalled();expect(component('Writer').key).toBe(writerKey);expect(component('NativeWritingSystemsWorkspace').key).toBe(alphabetKey);expect(JSON.stringify(data)).toBe(frozen);expect(native.save).not.toHaveBeenCalled();
 });
 it('reloads the exact legacy scene once after a changed alphabet assignment, then ignores the matching load',async()=>{
  await open(undefined,{tabId:undefined,onOpenRoute:undefined});button('Alphabets').props.onClick();await settle(()=>Boolean(component('NativeWritingSystemsWorkspace')));
  const writerKey=component('Writer').key,frozen=JSON.stringify(data);
  const changed:WritingSystemsDocument={schemaVersion:1,systems:[{schemaVersion:1,id:'synthetic-alphabet',mappingVersion:1,name:'Synthetic PUA',scriptId:'custom',encoding:'private-use',direction:'ltr',mappings:[{input:'a',output:'\uE000',transliteration:'a'}],font:null,glossary:[],notes:''}],assignments:[{scene:{kind:'scene',id:'first'},definitionId:'synthetic-alphabet',mappingVersion:1}]};
  native.command.mockClear();component('NativeWritingSystemsWorkspace').props.onAssignmentsChanged(changed);await settle(()=>component('Writer')?.key!==writerKey);
  const updatedKey=component('Writer').key,alphabetKey=component('NativeWritingSystemsWorkspace').key;
  expect(native.command.mock.calls.filter(([op])=>op==='open_project_index')).toHaveLength(1);expect(native.command.mock.calls.filter(([op])=>op==='load_project_scene')).toHaveLength(1);expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(0);expect(component('Writer').props.initial).toMatchObject({revisionId:'head-first',scene:data.scenes[0].scene});expect(component('Writer').props.writingSystem).toEqual(changed.systems[0]);
  component('NativeWritingSystemsWorkspace').props.onAssignmentsChanged(structuredClone(changed));await settle();
  expect(native.command.mock.calls.filter(([op])=>op==='open_project_index')).toHaveLength(1);expect(native.command.mock.calls.filter(([op])=>op==='load_project_scene')).toHaveLength(1);expect(native.command.mock.calls.filter(([op])=>op==='open_project')).toHaveLength(0);expect(component('Writer').key).toBe(updatedKey);expect(component('NativeWritingSystemsWorkspace').key).toBe(alphabetKey);expect(JSON.stringify(data)).toBe(frozen);expect(native.save).not.toHaveBeenCalled();
 });
 it('updates alphabet tool-tab state without reloading scenes or remounting its workspace',async()=>{
  await open({projectId:'project',sceneId:'second',view:'alphabets'});const alphabetKey=component('NativeWritingSystemsWorkspace').key;
  const changed:WritingSystemsDocument={schemaVersion:1,systems:[{schemaVersion:1,id:'synthetic-alphabet',mappingVersion:1,name:'Synthetic PUA',scriptId:'custom',encoding:'private-use',direction:'ltr',mappings:[],font:null,glossary:[],notes:''}],assignments:[]};
  native.command.mockClear();component('NativeWritingSystemsWorkspace').props.onAssignmentsChanged(changed);await settle();
  expect(native.command).not.toHaveBeenCalled();expect(component('NativeWritingSystemsWorkspace').key).toBe(alphabetKey);expect(component('Writer')).toBeUndefined();
 });
 it('keeps the legacy no-tab editor mounted while navigating to a tool view',async()=>{
  await open(undefined,{tabId:undefined,onOpenRoute:undefined});expect(component('Writer').props.initial.scene.title).toBe('first نور 中文');button('Planning').props.onClick();await settle(()=>Boolean(component('PlanningWorkspace')));expect(component('Writer')).toBeDefined();
 });
});
