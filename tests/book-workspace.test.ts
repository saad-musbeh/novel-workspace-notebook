import {beforeEach,afterEach,describe,expect,it,vi} from 'vitest';
import type {ProjectIndex,SceneRecord} from '../apps/writer/src/workspace';
import {createPlanningDocument} from '../apps/writer/src/planning/model';
import {BOOK_REVEAL_EVENT,BOOK_REVEAL_CANCEL_EVENT,BOOK_REVEAL_RESULT_EVENT} from '../apps/writer/src/book-model';
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[],dirty:false}));
const native=vi.hoisted(()=>({background:vi.fn(),repository:vi.fn(),invoke:vi.fn(),idle:vi.fn()}));
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react');const changed=(a:any[],b:any[])=>!a||a.length!==b.length||a.some((value,i)=>!Object.is(value,b[i]));const memo=(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,prior=hooks.slots[i];if(!prior||changed(prior.deps,deps))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;};
 const effect=(fn:()=>any,deps:any[])=>{const i=hooks.cursor++,prior=hooks.slots[i];if(!prior||changed(prior.deps,deps)){const entry={deps,cleanup:prior?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=fn();});}};
 return {...actual,useMemo:memo,useCallback:(fn:any,deps:any[])=>memo(()=>fn,deps),useEffect:effect,useLayoutEffect:effect,useRef:(initial:any)=>hooks.slots[hooks.cursor++]??={current:initial},useState:(initial:any)=>{const i=hooks.cursor++,slot=hooks.slots[i]??={value:typeof initial==='function'?initial():initial};return [slot.value,(value:any)=>{const next=typeof value==='function'?value(slot.value):value;if(!Object.is(next,slot.value)){slot.value=next;hooks.dirty=true;}}];}};
});
vi.mock('../apps/writer/src/workspace',()=>({workspaceBackgroundCommand:native.background,sceneRepository:native.repository}));
vi.mock('../apps/writer/src/Writer',()=>({Writer:function Writer(){return null;}}));
vi.mock('../apps/writer/src/typing-priority',()=>({waitForTypingIdle:native.idle}));
vi.mock('@tauri-apps/api/core',()=>({invoke:native.invoke}));
import {BookWorkspace} from '../apps/writer/src/BookWorkspace';
type BookProps=Parameters<typeof BookWorkspace>[0];
let props:BookProps,tree:any,records:Map<string,SceneRecord>,frames:FrameRequestCallback[],host:{scrollTop:number;clientHeight:number;clientWidth:number;dataset:Record<string,string>;dispatchEvent:(event:Event)=>boolean;addEventListener:EventTarget['addEventListener'];removeEventListener:EventTarget['removeEventListener']},previous:Map<string,any>,flushers:Map<string,ReturnType<typeof vi.fn>>,saveCalls:{id:string;parent:string|null}[],flush:(()=>Promise<void>)|null;
let frameId:number,cancelledFrames:Set<number>;
function nodes(node:any):any[]{return Array.isArray(node)?node.flatMap(nodes):node&&typeof node==='object'?[node,...nodes(node.props?.children)]:[];}
function text(node:any):string{return Array.isArray(node)?node.map(text).join(''):node&&typeof node==='object'?text(node.props?.children):node==null?'':String(node);}
function writers(){return nodes(tree).filter(node=>node.type?.name==='Writer');}
function render(beforeEffects?:()=>void){hooks.cursor=0;hooks.dirty=false;tree=BookWorkspace(props);const scroll=nodes(tree).find(node=>node.props?.className==='book-scroll');if(scroll)scroll.props.ref.current=host;const next=new Map(writers().map(writer=>[writer.props.sceneId,writer]));
 // Simulate the real Writer flush-ref cleanup when a mounted scene disappears.
 for(const [id,writer] of previous)if(!next.has(id))writer.props.flushRef.current=null;
 for(const [id,writer] of next){if(!previous.has(id)){const drain=vi.fn(async()=>{});flushers.set(id,drain);writer.props.flushRef.current=drain;writer.props.onEditorReady();}}
 previous=next;beforeEffects?.();for(const effect of hooks.effects.splice(0))effect();
}
async function settle(){for(let i=0;i<150;i++){await Promise.resolve();if(hooks.dirty)render();for(const frame of frames.splice(0))frame(i);}}
function fixture(count:number):ProjectIndex {return {chapterNumberStart:1,chapterNumberingRevisionId:null,chapterNumberingConflicted:false,project:{id:'project',title:'Book',createdAt:'fixture'},chapters:[{id:'chapter',projectId:'project',title:'Chapter',position:0}],scenes:Array.from({length:count},(_,i)=>({id:'scene-'+i,chapterId:'chapter',position:i,title:'Scene '+i+' مِ 中文',revisionId:'head-'+i}))};}
async function open(count=1000,extra:Partial<BookProps>={}){props={index:fixture(count),planning:null,alphabets:{schemaVersion:1,systems:[],assignments:[]},tabId:'book-tab',active:true,onSaved:vi.fn(),onReady:vi.fn(),onError:vi.fn(),onRegisterFlush:next=>{flush=next;},...extra};for(const summary of props.index.scenes)records.set(summary.id,{...summary,scene:{schemaVersion:1,title:summary.title,document:{type:'doc',content:[{type:'paragraph',attrs:{lang:'ar',dir:'rtl'},content:[{type:'text',text:'مِ 中文 繁體 👩🏽‍🚀 \uE000 '+summary.id}]}]}}});render();await settle();}
function scrollTo(top:number){host.scrollTop=top;nodes(tree).find(node=>node.props?.className==='book-scroll').props.onScroll();render();}
function selectCut(value:string){nodes(tree).find(node=>node.props?.['aria-label']==='Book selection').props.onChange({target:{value}});}
function foldButton(id:string){const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId===id);return nodes(card?.props.children).find(node=>node.type==='button'&&node.props['aria-expanded']!==undefined);}
beforeEach(()=>{hooks.slots=[];hooks.cursor=0;hooks.effects=[];hooks.dirty=false;vi.clearAllMocks();records=new Map();frames=[];frameId=0;cancelledFrames=new Set();const events=new EventTarget();host={scrollTop:0,clientHeight:800,clientWidth:850,dataset:{},addEventListener:events.addEventListener.bind(events),removeEventListener:events.removeEventListener.bind(events),dispatchEvent:event=>{if(event.type==='scroll')nodes(tree).find(node=>node.props?.className==='book-scroll')?.props.onScroll();return events.dispatchEvent(event);}};previous=new Map();flushers=new Map();saveCalls=[];flush=null;
 vi.stubGlobal('ResizeObserver',class {observe(){}disconnect(){}});vi.stubGlobal('requestAnimationFrame',(frame:FrameRequestCallback)=>{const id=++frameId;frames.push(time=>{if(!cancelledFrames.has(id))frame(time);});return id;});vi.stubGlobal('cancelAnimationFrame',(id:number)=>cancelledFrames.add(id));
 native.invoke.mockResolvedValue(null);native.idle.mockImplementation(async(signal:AbortSignal)=>{if(signal.aborted)throw new Error('Cancelled');});native.background.mockImplementation(async(operation,args,signal:AbortSignal)=>{if(signal.aborted)throw new Error('Cancelled');if(operation!=='load_project_scene')throw new Error('Unexpected background operation '+operation);const record=records.get(args.sceneId);if(!record)throw new Error('Missing scene');return structuredClone(record);});
 native.repository.mockImplementation((id:string)=>({load:vi.fn(async()=>{const record=records.get(id)!;return {scene:record.scene,revisionId:record.revisionId};}),save:vi.fn(async(scene:SceneRecord['scene'],parent:string|null)=>{const record=records.get(id)!;saveCalls.push({id,parent});if(parent!==record.revisionId)throw new Error('CAS refused for '+id);record.scene=structuredClone(scene);record.revisionId+='-saved';return {scene:structuredClone(record.scene),revisionId:record.revisionId};})}));
});
afterEach(()=>{for(const slot of hooks.slots)slot?.cleanup?.();vi.unstubAllGlobals();});
describe('whole book coordinator lazy ownership',()=>{
 it('drains exact pending text before parking a folded scene, then unfolds with its same secure session',async()=>{
  await open(10);const first=writers()[0],changed=structuredClone(first.props.initial.scene);changed.document.content![0].content![0].text+=' مِ / 繁體';
  let finish!:()=>void;const gate=new Promise<void>(resolve=>{finish=resolve;});
  flushers.get('scene-0')!.mockImplementation(async()=>{await gate;await first.props.repository.save(changed,first.props.initial.revisionId);});
  const focus=vi.fn();foldButton('scene-0').props.ref({focus});foldButton('scene-0').props.onClick();render();await settle();
  expect(writers().find(writer=>writer.props.sceneId==='scene-0')!.props.initial).toBe(first.props.initial);
  expect(writers()[0].props.tabActive).toBe(false);expect(native.invoke).not.toHaveBeenCalled();
  const barrier=flush!();finish();await settle();await barrier;
  expect(foldButton('scene-0').props['aria-expanded']).toBe(false);expect(foldButton('scene-0').props['aria-controls']).toBe('book-tab:body:scene-0');
  expect(writers().some(writer=>writer.props.sceneId==='scene-0')).toBe(false);
  expect(native.invoke).toHaveBeenCalledWith('editor_park',{sessionId:'book-tab:book:scene-0'});
  expect(saveCalls).toEqual([{id:'scene-0',parent:'head-0'}]);expect(focus).toHaveBeenCalledWith({preventScroll:true});
  foldButton('scene-0').props.onClick();render();await settle();
  const resumed=writers().find(writer=>writer.props.sceneId==='scene-0')!;
  expect(resumed.props.editorSessionId).toBe(first.props.editorSessionId);expect(resumed.props.initial.scene).toEqual(changed);
  expect(resumed.props.initial.revisionId).toBe('head-0-saved');expect(saveCalls).toHaveLength(1);
 });
 it.each(['Finish entering the current word','CAS conflict'])('a fold refused by %s leaves the exact mounted draft expanded',async message=>{
  await open(10);const source=writers()[0];flushers.get('scene-0')!.mockRejectedValue(new Error(message));
  foldButton('scene-0').props.onClick();render();await settle();
  expect(foldButton('scene-0').props['aria-expanded']).toBe(true);expect(writers()[0].props.initial).toBe(source.props.initial);
  expect(source.props.flushRef.current).not.toBeNull();expect(text(tree)).toContain(message);
  expect(native.invoke).not.toHaveBeenCalled();expect(saveCalls).toEqual([]);expect(tree.props.inert).toBe(false);
 });
 it('a failed native checkpoint leaves the editor mounted and the fold reversible on retry',async()=>{
  await open(10);const source=writers()[0];native.invoke.mockRejectedValueOnce(new Error('Checkpoint refused'));
  foldButton('scene-0').props.onClick();render();await settle();
  expect(foldButton('scene-0').props['aria-expanded']).toBe(true);expect(writers()[0].props.initial).toBe(source.props.initial);
  expect(text(tree)).toContain('Checkpoint refused');foldButton('scene-0').props.onClick();render();await settle();
  expect(foldButton('scene-0').props['aria-expanded']).toBe(false);expect(saveCalls).toEqual([]);
 });
 it('restored folded rows show only bounded headers and load no scene bodies or new revisions',async()=>{
  await open(1000,{initialPosition:{bookSceneId:'scene-0',bookSceneOffset:600,bookFoldedSceneIds:[...Array.from({length:1000},(_,i)=>'scene-'+i),'missing','scene-0']}});
  expect(writers()).toEqual([]);expect(native.background).not.toHaveBeenCalled();expect(props.onReady).toHaveBeenCalledTimes(1);
  expect(nodes(tree).filter(node=>node.type?.name==='BookCard')).toHaveLength(8);
  expect(JSON.parse(tree.props['data-book-folded-scene-ids'])).toHaveLength(1000);expect(host.dataset.bookSceneId).toBe('scene-0');expect(host.scrollTop).toBe(0);
  foldButton('scene-0').props.onClick();render();await settle();
  expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-0']);expect(native.background).toHaveBeenCalledTimes(1);expect(saveCalls).toEqual([]);
 });
 it('folding and unfolding an earlier scene retain the downstream passage and cached expanded geometry',async()=>{
  await open(10);const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0');
  card.props.measure('scene-0',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:3000})});render();writers()[0].props.onGeometry();await settle();
  card.props.onFocus();render();scrollTo(3208);await settle();
  foldButton('scene-0').props.onClick();render();await settle();
  expect(host.scrollTop).toBe(272);expect(host.dataset.bookSceneId).toBe('scene-1');expect(host.dataset.bookSceneOffset).toBe('200');
  foldButton('scene-0').props.onClick();render();await settle();
  expect(host.scrollTop).toBe(3208);expect(host.dataset.bookSceneId).toBe('scene-1');expect(host.dataset.bookSceneOffset).toBe('200');
  expect(saveCalls).toEqual([]);
 });
 it('reveals a distant folded scene only after measured native geometry restores its requested logical offset',async()=>{
  await open(1000,{initialPosition:{bookFoldedSceneIds:Array.from({length:1000},(_,i)=>'scene-'+i)}});
  const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
  host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'reveal',sceneId:'scene-500',offset:150}}));render();await settle();
  expect(replies).toEqual([]);expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-500']);
  const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-500');
  card.props.measure('scene-500',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:3000})});render();writers()[0].props.onGeometry();await settle();
  expect(replies).toEqual([{requestId:'reveal',ok:true}]);expect(host.dataset.bookSceneId).toBe('scene-500');expect(host.dataset.bookSceneOffset).toBe('150');
  expect(JSON.parse(tree.props['data-book-folded-scene-ids'])).not.toContain('scene-500');expect(saveCalls).toEqual([]);
 });
 it('a failed reveal drain reports refusal without unfolding its target or replacing the current draft',async()=>{
  await open(10,{initialPosition:{bookFoldedSceneIds:['scene-2']}});const source=writers()[0];
  flushers.get('scene-0')!.mockRejectedValue(new Error('Finish entering the current word'));const replies:any[]=[];
  host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
  host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'blocked',sceneId:'scene-2',offset:10}}));render();await settle();
  expect(replies).toEqual([{requestId:'blocked',ok:false,error:'Error: Finish entering the current word'}]);
  expect(JSON.parse(tree.props['data-book-folded-scene-ids'])).toContain('scene-2');expect(writers()[0].props.initial).toBe(source.props.initial);
  expect(native.invoke).not.toHaveBeenCalled();expect(saveCalls).toEqual([]);
 });
 it('a reveal outside the authored character cut is refused without silently changing the cut',async()=>{
  const planning=createPlanningDocument();planning.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'scene-0'}]}];
  await open(10,{planning,initialPosition:{bookCharacterId:'hero'}});const replies:any[]=[];
  host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
  host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'outside',sceneId:'scene-9'}}));render();await settle();
  expect(replies).toEqual([{requestId:'outside',ok:false,error:'This scene is outside the requested book selection.'}]);
  expect(tree.props['data-book-character']).toBe('hero');expect(native.background).toHaveBeenCalledTimes(1);expect(saveCalls).toEqual([]);
 });
 it('opens only the bounded viewport of a thousand-scene index and defers every other body',async()=>{await open();expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-0','scene-1']);expect(native.background.mock.calls.map(([,args])=>args.sceneId)).toEqual(['scene-0','scene-1']);expect(props.onReady).toHaveBeenCalledTimes(1);expect(saveCalls).toEqual([]);expect(writers()[0].props.editorSessionId).toBe('book-tab:book:scene-0');expect(writers()[0].props.externalScroll.current).toBe(host);});
 it('committed native geometry refreshes a growing card even without resize-observer delivery',async()=>{
  await open();const initial=writers()[0].props.initial;
  const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0');
  let height=200;
  const element={isConnected:true,clientWidth:978,getBoundingClientRect:()=>({height,width:978})} as HTMLElement;
  const dispose=card.props.measure('scene-0',element);render();
  // The native frame commits its actual height. This harness deliberately never delivers ResizeObserver.
  height=8289;writers().find(writer=>writer.props.sceneId==='scene-0')!.props.onGeometry();render();await settle();
  expect(nodes(tree).find(node=>node.props?.className==='book-pages').props.style.height).toBe(999*560+8297);
  scrollTo(1500);await settle();
  expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-0']);
  expect(writers()[0].props.initial).toBe(initial);
  expect(native.invoke.mock.calls.some(([,args])=>args.sessionId==='book-tab:book:scene-0')).toBe(false);
  expect(saveCalls).toEqual([]);dispose();
 });
 it('drains and parks old visible editors before loading distant bodies, retaining exact focused scene',async()=>{await open();const initial=writers()[0],before=JSON.stringify(initial.props.initial);nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0').props.onFocus();render();scrollTo(500*560);await settle();expect(writers().map(writer=>writer.props.sceneId).sort()).toEqual(['scene-0','scene-499','scene-500','scene-501']);expect(flushers.get('scene-1')).toHaveBeenCalledTimes(1);expect(native.invoke).toHaveBeenCalledWith('editor_park',{sessionId:'book-tab:book:scene-1'});expect(JSON.stringify(writers().find(writer=>writer.props.sceneId==='scene-0')!.props.initial)).toBe(before);expect(saveCalls).toEqual([]);});
 it.each(['Finish entering the current word','CAS conflict'])('failed offscreen drain retains the exact mounted source after %s',async message=>{await open();const source=writers()[0],initial=source.props.initial;flushers.get('scene-0')!.mockRejectedValue(new Error(message));scrollTo(500*560);await settle();expect(text(tree)).toContain(message);expect(writers().find(writer=>writer.props.sceneId==='scene-0')!.props.initial).toBe(initial);expect(source.props.flushRef.current).not.toBeNull();expect(native.invoke).not.toHaveBeenCalled();expect(native.background).toHaveBeenCalledTimes(2);expect(props.onError).toHaveBeenCalledWith('Error: '+message);});
 it.each(['Finish entering the current word','CAS conflict'])('refuses a character cut on %s and preserves current scene bodies',async message=>{const planning=createPlanningDocument();planning.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'scene-2'}]}];await open(10,{planning});const originals=writers().map(writer=>writer.props.initial);flushers.get('scene-0')!.mockRejectedValue(new Error(message));selectCut('hero');await settle();expect(tree.props['data-book-character']).toBe('');expect(writers().map(writer=>writer.props.initial)).toEqual(originals);expect(text(tree)).toContain(message);expect(native.invoke).not.toHaveBeenCalled();expect(saveCalls).toEqual([]);});
 it('successful character cut parks excluded scenes before their Writer cleanup and opens only authored ordered targets',async()=>{const planning=createPlanningDocument();planning.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'scene-5'},{kind:'scene',id:'scene-2'}]}];await open(10,{planning});selectCut('hero');await settle();expect(tree.props['data-book-character']).toBe('hero');expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-2','scene-5']);expect(native.invoke.mock.calls).toEqual(expect.arrayContaining([['editor_park',{sessionId:'book-tab:book:scene-0'}],['editor_park',{sessionId:'book-tab:book:scene-1'}]]));expect(props.onError).not.toHaveBeenCalled();expect(saveCalls).toEqual([]);});
 it('saves each scene through its own exact repository/head and never generates a merge/conflict resolution',async()=>{await open(10);const [a,b]=writers(),originalB=JSON.stringify(b.props.initial),sceneA=structuredClone(a.props.initial.scene),sceneB=structuredClone(b.props.initial.scene);sceneA.title='Saved Arabic مِ';sceneB.title='Saved 繁體';await Promise.all([a.props.repository.save(sceneA,a.props.initial.revisionId),b.props.repository.save(sceneB,b.props.initial.revisionId)]);expect(saveCalls).toEqual([{id:'scene-0',parent:'head-0'},{id:'scene-1',parent:'head-1'}]);expect(records.get('scene-0')!.scene.title).toBe('Saved Arabic مِ');expect(records.get('scene-1')!.scene.title).toBe('Saved 繁體');expect(props.onSaved).toHaveBeenCalledTimes(2);expect(JSON.stringify(b.props.initial)).toBe(originalB);expect(native.background.mock.calls.every(([op])=>op==='load_project_scene')).toBe(true);});
 it('rejects an external scene-head conflict without touching the other scene or resolving heads automatically',async()=>{await open(10);const [a,b]=writers(),beforeB=JSON.stringify(records.get('scene-1'));records.get('scene-0')!.revisionId='remote';await expect(a.props.repository.save(a.props.initial.scene,a.props.initial.revisionId)).rejects.toThrow('CAS refused');expect(JSON.stringify(records.get('scene-1'))).toBe(beforeB);expect(props.onSaved).not.toHaveBeenCalled();expect(native.invoke).not.toHaveBeenCalled();expect(b.props.initial.revisionId).toBe('head-1');});
 it('hidden incoming book loads only its initial viewport to acknowledge readiness, then defers new bodies until activated',async()=>{
  await open(1000,{active:false});
  expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-0','scene-1']);
  expect(writers().every(writer=>writer.props.tabActive===false)).toBe(true);
  expect(props.onReady).toHaveBeenCalledTimes(1);expect(native.background).toHaveBeenCalledTimes(2);
  scrollTo(500*560);await settle();
  expect(native.background).toHaveBeenCalledTimes(2);expect(native.invoke).not.toHaveBeenCalled();
  props={...props,active:true};render();await settle();
  expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-499','scene-500','scene-501']);
  expect(native.background).toHaveBeenCalledTimes(5);expect(props.onReady).toHaveBeenCalledTimes(1);
 });
 it('empty manuscript is ready without starting an editor or writing a database row',async()=>{await open(0);expect(writers()).toEqual([]);expect(native.background).not.toHaveBeenCalled();expect(props.onReady).toHaveBeenCalledTimes(1);expect(saveCalls).toEqual([]);});
});

it('a suspended screen-frame callback cannot strand a character-cut save and parking barrier',async()=>{
 vi.useFakeTimers();
 try{
  const planning=createPlanningDocument();planning.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'scene-2'}]}];
  await open(10,{planning});
  frames=[];vi.stubGlobal('requestAnimationFrame',()=>++frameId);
  selectCut('hero');
  for(let i=0;i<20;i++){await Promise.resolve();if(hooks.dirty)render();}
  expect(tree.props['data-book-character']).toBe('');
  expect(native.invoke).not.toHaveBeenCalled();
  for(let i=0;i<12;i++){await vi.advanceTimersByTimeAsync(50);for(let j=0;j<30;j++){await Promise.resolve();if(hooks.dirty)render();}}
  expect(tree.props['data-book-character']).toBe('hero');
  expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-2']);
  expect(native.invoke).toHaveBeenCalledWith('editor_park',{sessionId:'book-tab:book:scene-0'});
  expect(flushers.get('scene-0')).toHaveBeenCalledTimes(1);
  await expect(flush!()).resolves.toBeUndefined();
  expect(cancelledFrames.size).toBeGreaterThan(0);expect(props.onError).not.toHaveBeenCalled();
 }finally{vi.useRealTimers();}
});

 it('retains a measured logical scene anchor while inactive or zero width, then restores after activation',async()=>{
  await open(10,{initialScrollTop:600,initialPosition:{bookSceneId:'scene-0',bookSceneOffset:600}});
  props={...props,active:false};render();
  const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0');
  card.props.measure('scene-0',{getBoundingClientRect:()=>({height:3000})});
  render();await settle();expect(host.scrollTop).toBe(0);
  host.clientWidth=0;props={...props,active:true};render();await settle();expect(host.scrollTop).toBe(0);
  host.clientWidth=850;props={...props,active:false};render();props={...props,active:true};render();writers().find(writer=>writer.props.sceneId==='scene-0')!.props.onGeometry();await settle();
  expect(host.scrollTop).toBe(600);expect(host.dataset.bookSceneId).toBe('scene-0');expect(host.dataset.bookSceneOffset).toBe('600');
  expect(props.onError).not.toHaveBeenCalled();expect(saveCalls).toEqual([]);
 });

it('does not consume a logical offset against loading-card height or stale rendered rows',async()=>{
 await open(2,{initialScrollTop:600,initialPosition:{bookSceneId:'scene-0',bookSceneOffset:600}});
 let height=400;
 const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0');
 card.props.measure('scene-0',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height})});
 render();await settle();expect(host.scrollTop).toBe(0);
 // The child geometry callback runs before the parent layout effect in this
 // commit. Its measurement is new, while that parent's rendered rows are old.
 height=3000;render(()=>writers()[0].props.onGeometry());
 expect(host.scrollTop).toBe(0);
 await settle();expect(host.scrollTop).toBe(600);expect(host.dataset.bookSceneOffset).toBe('600');
 expect(props.onError).not.toHaveBeenCalled();expect(saveCalls).toEqual([]);
});
it('clamps a formerly longer scene only after real short native geometry has committed',async()=>{
 await open(2,{initialScrollTop:600,initialPosition:{bookSceneId:'scene-0',bookSceneOffset:600}});
 const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0');
 card.props.measure('scene-0',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:400})});render();await settle();
 expect(host.scrollTop).toBe(0);writers()[0].props.onGeometry();await settle();
 expect(host.scrollTop).toBe(288);expect(host.dataset.bookSceneOffset).toBe('288');
});

it('records committed native height even when it equals the initial 560-pixel estimate',async()=>{
 await open(2,{initialPosition:{bookSceneId:'scene-0',bookSceneOffset:300}});
 const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-0');
 card.props.measure('scene-0',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:552})});render();await settle();
 expect(host.scrollTop).toBe(0);writers()[0].props.onGeometry();await settle();expect(host.scrollTop).toBe(300);
});

it('cancels a reveal during its save barrier and ignores late drain completion, while a later retry succeeds',async()=>{
 await open(10,{initialPosition:{bookFoldedSceneIds:['scene-2']}});
 let release!:()=>void;flushers.get('scene-0')!.mockImplementationOnce(()=>new Promise<void>(resolve=>{release=resolve;}));
 const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'cancelled',sceneId:'scene-2',offset:150}}));render();await settle();
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_CANCEL_EVENT,{detail:{requestId:'cancelled'}}));release();await settle();
 expect(replies).toEqual([{requestId:'cancelled',ok:false,error:'Reading restoration was cancelled.'}]);
 expect(JSON.parse(tree.props['data-book-folded-scene-ids'])).toContain('scene-2');expect(writers().some(writer=>writer.props.sceneId==='scene-2')).toBe(false);
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'retry',sceneId:'scene-2',offset:150}}));render();await settle();
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_CANCEL_EVENT,{detail:{requestId:'cancelled'}}));
 const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-2');card.props.measure('scene-2',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:3000})});render();writers().find(writer=>writer.props.sceneId==='scene-2')!.props.onGeometry();await settle();
 expect(replies.at(-1)).toEqual({requestId:'retry',ok:true});expect(host.dataset.bookSceneId).toBe('scene-2');expect(host.dataset.bookSceneOffset).toBe('150');expect(saveCalls).toEqual([]);
});
it('cancellation of a queued geometry restoration cannot later apply its stale offset',async()=>{
 await open(10,{initialPosition:{bookFoldedSceneIds:['scene-2']}});
 const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'geometry',sceneId:'scene-2',offset:150}}));render();await settle();
 expect(replies).toEqual([]);host.dispatchEvent(new CustomEvent(BOOK_REVEAL_CANCEL_EVENT,{detail:{requestId:'geometry'}}));const top=host.scrollTop;
 const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-2');card.props.measure('scene-2',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:3000})});render();writers().find(writer=>writer.props.sceneId==='scene-2')!.props.onGeometry();await settle();
 expect(host.scrollTop).toBe(top);expect(replies).toEqual([{requestId:'geometry',ok:false,error:'Reading restoration was cancelled.'}]);
});

it('cancelling a character restoration before its delayed drain cannot commit a late filter or offset, and retry is atomic',async()=>{
 const planning=createPlanningDocument();planning.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'scene-2'}]}];await open(10,{planning});
 let release!:()=>void;flushers.get('scene-0')!.mockImplementationOnce(()=>new Promise<void>(resolve=>{release=resolve;}));
 const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'cut-old',characterId:'hero',sceneId:'scene-2',offset:150}}));render();await settle();
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_CANCEL_EVENT,{detail:{requestId:'cut-old'}}));release();await settle();
 expect(tree.props['data-book-character']).toBe('');expect(host.scrollTop).toBe(0);expect(native.invoke).not.toHaveBeenCalled();expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-0','scene-1']);
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'cut-new',characterId:'hero',sceneId:'scene-2',offset:150}}));render();await settle();
 expect(tree.props['data-book-character']).toBe('hero');expect(writers().map(writer=>writer.props.sceneId)).toEqual(['scene-2']);
 const card=nodes(tree).find(node=>node.type?.name==='BookCard'&&node.props.sceneId==='scene-2');card.props.measure('scene-2',{clientWidth:850,isConnected:true,getBoundingClientRect:()=>({height:3000})});render();writers()[0].props.onGeometry();await settle();
 expect(replies.at(-1)).toEqual({requestId:'cut-new',ok:true});expect(host.scrollTop).toBe(150);expect(saveCalls).toEqual([]);
});
it('a target must belong to the requested character cut, rather than merely the currently visible book',async()=>{
 const planning=createPlanningDocument();planning.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'scene-2'}]}];await open(10,{planning});
 const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'wrong-cut',characterId:'hero',sceneId:'scene-0',offset:150}}));await settle();
 expect(replies).toEqual([{requestId:'wrong-cut',ok:false,error:'This scene is outside the requested book selection.'}]);expect(tree.props['data-book-character']).toBe('');expect(flushers.get('scene-0')).not.toHaveBeenCalled();
});

it('restores an empty character cut only after its drain and cancels a late filter commit',async()=>{
 const planning=createPlanningDocument();planning.records=[{id:'empty',kind:'character',name:'Empty',aliases:[],notes:'',values:{},links:[]}];await open(3,{planning});
 let release!:()=>void;flushers.get('scene-0')!.mockImplementationOnce(()=>new Promise<void>(resolve=>{release=resolve;}));
 const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'empty-old',characterId:'empty'}}));render();await settle();
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_CANCEL_EVENT,{detail:{requestId:'empty-old'}}));release();await settle();
 expect(tree.props['data-book-character']).toBe('');expect(native.invoke).not.toHaveBeenCalled();
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'empty-new',characterId:'empty'}}));render();await settle();
 expect(tree.props['data-book-character']).toBe('empty');expect(writers()).toEqual([]);expect(host.scrollTop).toBe(0);
 expect(replies.at(-1)).toEqual({requestId:'empty-new',ok:true});expect(saveCalls).toEqual([]);
});
it('refuses a scene-less request when the requested cut contains scenes',async()=>{
 await open(3);const replies:any[]=[];host.addEventListener(BOOK_REVEAL_RESULT_EVENT,event=>replies.push((event as CustomEvent).detail));
 host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId:'missing-target',characterId:''}}));await settle();
 expect(replies).toEqual([{requestId:'missing-target',ok:false,error:'This scene is outside the requested book selection.'}]);expect(flushers.get('scene-0')).not.toHaveBeenCalled();
});
