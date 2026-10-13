import {beforeEach,afterEach,describe,expect,it,vi} from 'vitest';
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[],dirty:false}));
const rpc=vi.hoisted(()=>vi.fn());
vi.hoisted(()=>{Object.defineProperty(globalThis,'window',{value:{__TAURI_INTERNALS__:{}},configurable:true,writable:true});});
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react');
 const changed=(a:any[],b:any[])=>!a||!b||a.length!==b.length||a.some((v,i)=>!Object.is(v,b[i]));
 const memo=(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;};
 return {...actual,useMemo:memo,useCallback:(fn:any,deps:any[])=>memo(()=>fn,deps),useRef:(value:any)=>{const i=hooks.cursor++;return hooks.slots[i]??={current:value};},useState:(initial:any)=>{const i=hooks.cursor++,state=hooks.slots[i]??={value:typeof initial==='function'?initial():initial};return [state.value,(next:any)=>{const value=typeof next==='function'?next(state.value):next;if(!Object.is(value,state.value)){state.value=value;hooks.dirty=true;}}];},useEffect:(effect:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps)){const entry={deps,cleanup:old?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=effect();});}}};
});
vi.mock('../apps/writer/src/NativeEditor',()=>({NativeEditor:function NativeEditor(){return null;}}));
vi.mock('../apps/writer/src/workspace',()=>({workspaceCommand:rpc}));
vi.mock('../apps/writer/src/use-character-count',()=>({useCharacterCount:()=>10}));
import {Writer} from '../apps/writer/src/Writer';
import {ParagraphAlternatives} from '../apps/writer/src/ParagraphAlternatives';
import type {ParagraphCapture} from '../apps/writer/src/paragraph-alternatives';
let panelProps:any;
let tree:any,props:Parameters<typeof Writer>[0],head:string,editor:any,save:any;
function nodes(node:any):any[]{if(!node||typeof node!=='object')return [];if(Array.isArray(node))return node.flatMap(nodes);return [node,...nodes(node.props?.children)];}
function content(node:any):string{return Array.isArray(node)?node.map(content).join(''):typeof node==='object'?content(node?.props?.children):node==null?'':String(node);}
function component(name:string){return nodes(tree).find(node=>typeof node.type!=='string'&&node.type?.name===name);}
function render(){hooks.cursor=0;hooks.dirty=false;tree=panelProps?ParagraphAlternatives(panelProps):Writer(props);for(const effect of hooks.effects.splice(0))effect();}
function panel(){component('NativeEditor').props.onPending(false);render();nodes(tree).find(node=>node.type==='button'&&content(node)==='Paragraph alternatives').props.onClick();render();return component('ParagraphAlternatives');}
beforeEach(()=>{
 panelProps=undefined;hooks.slots=[];hooks.effects=[];hooks.cursor=0;hooks.dirty=false;head='r1';vi.clearAllMocks();
 vi.stubGlobal('window',{__TAURI_INTERNALS__:{},addEventListener:vi.fn(),removeEventListener:vi.fn()});vi.stubGlobal('document',{addEventListener:vi.fn(),removeEventListener:vi.fn(),visibilityState:'visible'});
 const initial={revisionId:'r1',scene:{schemaVersion:1 as const,title:'Synthetic',document:{type:'doc',content:[{type:'paragraph',attrs:{lang:'ar',dir:'rtl'},content:[{type:'text',text:'مِ 中文',marks:[{type:'bold'}]}]}]}}};
 save=vi.fn(async(scene:any,parent:string)=>{expect(parent).toBe(head);head='r2';return {scene,revisionId:head};});
 props={initial,projectId:'project',sceneId:'scene',editorSessionId:'tab',onTitle:vi.fn(),performanceProbe:false,closeProbe:false,flushRef:{current:null},repository:{load:vi.fn(async()=>initial),save}};
 const capture:ParagraphCapture={blockIndex:0,documentHash:'a'.repeat(64),block:structuredClone(initial.scene.document.content[0])};
 editor={flush:vi.fn(async()=>{}),captureParagraph:vi.fn(async()=>capture),swapParagraph:vi.fn(async()=>({...capture,documentHash:'b'.repeat(64)})),prepareClose:vi.fn(async()=>{}),resume:vi.fn()};
 rpc.mockImplementation(async(op:string,args:any)=>{expect(op).toBe('load_project_scene');expect(args).toEqual({projectId:'project',sceneId:'scene'});return {id:'scene',revisionId:head,scene:initial.scene};});
 render();component('NativeEditor').props.ref.current=editor;
});
afterEach(()=>{for(const slot of hooks.slots)slot?.cleanup?.();vi.unstubAllGlobals();});
describe('paragraph alternative writing barriers',()=>{
 it('captures the exact native paragraph after confirming the scoped saved head without remount',async()=>{
  const native=component('NativeEditor'),alternatives=panel();const receipt=await alternatives.props.capture();
  expect(receipt).toEqual({capture:await editor.captureParagraph.mock.results[0].value,revisionId:'r1'});expect(editor.flush).toHaveBeenCalledOnce();expect(editor.captureParagraph).toHaveBeenCalledOnce();expect(component('NativeEditor').key).toBe(native.key);expect(save).not.toHaveBeenCalled();
 });
 it('refuses a saved paragraph anchor after another window advances the scene',async()=>{
  const alternatives=panel();head='received-r2';
  await expect(alternatives.props.apply({blockIndex:0,documentHash:'a'.repeat(64),block:{type:'paragraph'}},{type:'paragraph'},'r1')).rejects.toThrow('scene changed');
  expect(editor.swapParagraph).not.toHaveBeenCalled();expect(save).not.toHaveBeenCalled();expect(props.flushRef.current).toBeTypeOf('function');
 });
 it('checkpoints and saves a swapped paragraph through the exact parent, then returns its acknowledged revision',async()=>{
  const alternatives=panel(),replacement={type:'paragraph',attrs:{lang:'zh-Hant',dir:'auto'},content:[{type:'text',text:'替代 مِ 中文'}]},anchor={blockIndex:0,documentHash:'a'.repeat(64),block:props.initial.scene.document.content![0]};let swapped=false;
  editor.swapParagraph.mockImplementation(async()=>{swapped=true;return {blockIndex:0,documentHash:'b'.repeat(64),block:replacement};});
  editor.flush.mockImplementation(async()=>{if(swapped){component('NativeEditor').props.onChange({type:'doc',content:[replacement]});swapped=false;}});
  const receipt=await alternatives.props.apply(anchor,replacement,'r1');expect(receipt.revisionId).toBe('r2');expect(save).toHaveBeenCalledExactlyOnceWith({...props.initial.scene,document:{type:'doc',content:[replacement]}},'r1');expect(editor.swapParagraph).toHaveBeenCalledWith(anchor,replacement);
 });
 it('failed alternative metadata or composition blocks close before releasing the editor session',async()=>{
  const alternatives=panel();alternatives.props.onRegisterFlush(async()=>{throw new Error('Conflict: pending alternatives retained');});
  await expect(props.flushRef.current!()).rejects.toThrow('pending alternatives retained');expect(editor.prepareClose).not.toHaveBeenCalled();expect(save).not.toHaveBeenCalled();expect(editor.resume).toHaveBeenCalledOnce();
  alternatives.props.onRegisterFlush(async()=>{});const title=nodes(tree).find(node=>node.props?.className==='scene-title');title.props.onCompositionStart();
  await expect(props.flushRef.current!()).rejects.toThrow('Finish entering');expect(editor.prepareClose).not.toHaveBeenCalled();
 });
 it('keeps the old Writer API usable without sceneId and does not create a metadata controller',()=>{
  props={...props,sceneId:undefined};render();expect(component('ParagraphAlternatives')).toBeUndefined();expect(nodes(tree).some(node=>node.type==='button'&&content(node)==='Paragraph alternatives')).toBe(false);
 });
});

async function settlePanel(predicate:()=>boolean){for(let i=0;i<100;i++){await new Promise(resolve=>setTimeout(resolve,0));if(hooks.dirty)render();if(predicate())return;}throw new Error('Alternatives panel did not settle');}
function button(label:string){return nodes(tree).find(node=>node.type==='button'&&content(node)===label);}
async function openPanel(){
 for(const slot of hooks.slots)slot?.cleanup?.();hooks.slots=[];hooks.effects=[];hooks.cursor=0;
 const original=props.initial.scene.document.content![0],capture={blockIndex:0,documentHash:'a'.repeat(64),block:structuredClone(original)};
 let saved:any=null,revision:string|null=null,sequence=0,fail=false;const order:string[]=[];
 rpc.mockImplementation(async(op:string,args:any)=>{if(op==='load_project_data')return saved?{payload:structuredClone(saved),revisionId:revision}:null;expect(op).toBe('save_project_data');expect(args.parent).toBe(revision);if(fail)throw new Error('Connection failed: pending alternative retained');saved=structuredClone(args.payload);revision='metadata-'+(++sequence);order.push('save:'+saved.scenes[0].groups[0].versions.length);return {payload:structuredClone(saved),revisionId:revision};});
 panelProps={projectId:'project',sceneId:'scene',capture:vi.fn(async()=>({capture,revisionId:'r1'})),apply:vi.fn(async(anchor:any,replacement:any)=>{order.push('swap');return {capture:{...anchor,documentHash:'b'.repeat(64),block:replacement},revisionId:'r2'};}),onRegisterFlush:vi.fn(),onBusy:vi.fn(),onClose:vi.fn()};
 render();await settlePanel(()=>Boolean(button('Capture current paragraph')&&!button('Capture current paragraph').props.disabled));
 return {original,order,saved:()=>saved,fail:(next:boolean)=>{fail=next;},drain:()=>panelProps.onRegisterFlush.mock.calls.at(-1)[0]()};
}
describe('paragraph alternatives panel lifecycle',()=>{
 it('persists the original and edited rich alternative before any explicit swap',async()=>{
  const state=await openPanel();button('Capture current paragraph').props.onClick();await settlePanel(()=>Boolean(nodes(tree).find(node=>node.type==='textarea')));
  const textarea=nodes(tree).find(node=>node.type==='textarea');textarea.props.onChange({target:{value:'Alternative مِ 中文'}});render();
  button('Save as new alternative').props.onClick();await settlePanel(()=>state.saved()?.scenes[0].groups[0].versions.length===2&&!button('Swap saved version into draft').props.disabled);
  expect(state.order).toEqual(['save:1','save:2']);expect(panelProps.apply).not.toHaveBeenCalled();
  expect(state.saved().scenes[0].groups[0].versions[0].block).toEqual(state.original);
  button('Swap saved version into draft').props.onClick();await settlePanel(()=>state.saved()?.scenes[0].groups[0].sceneRevisionId==='r2');
  expect(state.order).toEqual(['save:1','save:2','swap','save:2']);expect(panelProps.apply).toHaveBeenCalledWith(expect.objectContaining({documentHash:'a'.repeat(64)}),expect.objectContaining({attrs:state.original.attrs}),'r1');
  const group=state.saved().scenes[0].groups[0];expect(group.activeId).toBe(group.versions[1].id);expect(group.versions[0].block).toEqual(state.original);
 });
 it('saves a locally edited alternative before close and retains it through failed save and retry without duplicate versions',async()=>{
  const state=await openPanel();button('Capture current paragraph').props.onClick();await settlePanel(()=>Boolean(nodes(tree).find(node=>node.type==='textarea')));
  nodes(tree).find(node=>node.type==='textarea').props.onChange({target:{value:'Unsaved مِ 中文'}});render();state.fail(true);
  button('Close alternatives').props.onClick();await settlePanel(()=>content(tree).includes('Connection failed'));
  expect(panelProps.onClose).not.toHaveBeenCalled();expect(state.saved().scenes[0].groups[0].versions).toHaveLength(1);
  await expect(state.drain()).rejects.toThrow('Connection failed');expect(nodes(tree).find(node=>node.type==='textarea').props.value).toBe('Unsaved مِ 中文');
  state.fail(false);button('Retry alternatives save').props.onClick();await settlePanel(()=>state.saved()?.scenes[0].groups[0].versions.length===2&&!content(tree).includes('Connection failed'));
  button('Close alternatives').props.onClick();await settlePanel(()=>panelProps.onClose.mock.calls.length===1);
  expect(state.saved().scenes[0].groups[0].versions).toHaveLength(2);expect(state.saved().scenes[0].groups[0].versions[0].block).toEqual(state.original);
 });
 it('refuses variant composition and a capture response from a departed scene',async()=>{
  const state=await openPanel();tree.props.onCompositionStart();await expect(state.drain()).rejects.toThrow('Finish entering');tree.props.onCompositionEnd();
  let release!:(value:any)=>void;panelProps.capture.mockImplementation(()=>new Promise(resolve=>{release=resolve;}));button('Capture current paragraph').props.onClick();await settlePanel(()=>Boolean(release));
  panelProps={...panelProps,sceneId:'other-scene'};render();release({capture:{blockIndex:0,documentHash:'a'.repeat(64),block:state.original},revisionId:'r1'});await settlePanel(()=>content(tree).includes('scene changed'));
  expect(state.saved()).toBeNull();expect(rpc.mock.calls.filter(([op])=>op==='save_project_data')).toHaveLength(0);
 });
});

describe('native navigation adapter barriers',()=>{
 it('confirms latest exact writing before capturing selection and reading position without a remount',async()=>{
  const order:string[]=[],scroll={scrollTop:420,dispatchEvent:vi.fn()},nativeNode=component('NativeEditor');
  tree.props.ref.current={querySelector:()=>scroll};
  const bookmark={documentHash:'c'.repeat(64),cursor:[0,2],anchor:[0,0]};
  editor.flush.mockImplementation(async()=>{order.push('checkpoint');nativeNode.props.onChange({...props.initial.scene.document,content:[{type:'paragraph',content:[{type:'text',text:'يس مِ 中文 saved'}]}]});});
  editor.captureNavigation=vi.fn(async()=>{order.push('capture');return bookmark;});
  save.mockImplementation(async(scene:any,parent:string)=>{order.push('save');return {scene,revisionId:'r2'};});
  const registered=vi.fn();props={...props,onRegisterNavigation:registered};render();
  const controller=registered.mock.calls.at(-1)![0];const result=await controller.capture();
  expect(order).toEqual(['checkpoint','save','capture']);expect(result).toEqual({native:bookmark,reading:{scrollTop:420}});expect(component('NativeEditor').key).toBe(nativeNode.key);expect(editor.prepareClose).not.toHaveBeenCalled();
 });
 it('restores through the existing native handle and notifies manual scrolling, while stale refusal preserves the viewport',async()=>{
  const scroll={scrollTop:100,dispatchEvent:vi.fn()};tree.props.ref.current={querySelector:()=>scroll};
  editor.restoreNavigation=vi.fn(async()=>{});const registered=vi.fn();props={...props,onRegisterNavigation:registered};render();
  const controller=registered.mock.calls.at(-1)![0],bookmark={native:{documentHash:'d'.repeat(64),cursor:[0,2],anchor:[0,1]},reading:{scrollTop:450}};
  await controller.restore(bookmark);expect(editor.restoreNavigation).toHaveBeenCalledWith(bookmark.native);expect(scroll.scrollTop).toBe(450);expect(scroll.dispatchEvent).toHaveBeenCalledOnce();expect(save).not.toHaveBeenCalled();
  editor.restoreNavigation.mockRejectedValue(new Error('This scene changed'));await expect(controller.restore({...bookmark,reading:{scrollTop:999}})).rejects.toThrow('scene changed');expect(scroll.scrollTop).toBe(450);expect(save).not.toHaveBeenCalled();
 });
 it('active title composition refuses capture before any native or save action',async()=>{
  editor.captureNavigation=vi.fn();const registered=vi.fn();props={...props,onRegisterNavigation:registered};render();
  nodes(tree).find(node=>node.props?.className==='scene-title').props.onCompositionStart();
  await expect(registered.mock.calls.at(-1)![0].capture()).rejects.toThrow('Finish');expect(editor.flush).not.toHaveBeenCalled();expect(editor.captureNavigation).not.toHaveBeenCalled();expect(save).not.toHaveBeenCalled();
 });
});
