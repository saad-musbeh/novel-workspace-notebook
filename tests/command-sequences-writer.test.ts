import {beforeEach,afterEach,describe,expect,it,vi} from 'vitest';
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[],dirty:false}));
const sequenceHook=vi.hoisted(()=>({executor:null as any,enabled:false}));
vi.hoisted(()=>{Object.defineProperty(globalThis,'window',{value:{__TAURI_INTERNALS__:{}},configurable:true,writable:true});});
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react'),changed=(a:any[],b:any[])=>!a||!b||a.length!==b.length||a.some((v,i)=>!Object.is(v,b[i]));
 const memo=(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;};
 return {...actual,useMemo:memo,useCallback:(fn:any,deps:any[])=>memo(()=>fn,deps),useRef:(value:any)=>hooks.slots[hooks.cursor++]??={current:value},useState:(initial:any)=>{const i=hooks.cursor++,state=hooks.slots[i]??={value:typeof initial==='function'?initial():initial};return [state.value,(next:any)=>{state.value=typeof next==='function'?next(state.value):next;hooks.dirty=true;}];},useEffect:(effect:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps)){const entry={deps,cleanup:old?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=effect();});}}};
});
vi.mock('../apps/writer/src/use-command-sequences',()=>({useCommandSequenceExecutor:(executor:any,enabled:boolean)=>{sequenceHook.executor=executor;sequenceHook.enabled=enabled;return {error:''};}}));
vi.mock('../apps/writer/src/NativeEditor',()=>({NativeEditor:function NativeEditor(){return null;}}));
vi.mock('../apps/writer/src/use-character-count',()=>({useCharacterCount:()=>12}));
import {Writer} from '../apps/writer/src/Writer';
import {cancelRecording,playCommandSequence,recordCommand,saveRecording,startRecording} from '../apps/writer/src/command-sequences';
let props:Parameters<typeof Writer>[0],tree:any,editor:any,save:any,order:string[],head:string;
function nodes(node:any):any[]{return Array.isArray(node)?node.flatMap(nodes):node&&typeof node==='object'?[node,...nodes(node.props?.children)]:[];}
function native(){return nodes(tree).find(node=>node.type?.name==='NativeEditor');}
function render(){hooks.cursor=0;hooks.dirty=false;tree=Writer(props);for(const effect of hooks.effects.splice(0))effect();native().props.ref.current=editor;}
beforeEach(()=>{
 hooks.slots=[];hooks.effects=[];hooks.cursor=0;hooks.dirty=false;order=[];head='r1';vi.clearAllMocks();
 const stored=new Map<string,string>();vi.stubGlobal('localStorage',{getItem:(key:string)=>stored.get(key)??null,setItem:(key:string,value:string)=>stored.set(key,value)});
 vi.stubGlobal('window',{__TAURI_INTERNALS__:{},addEventListener:vi.fn(),removeEventListener:vi.fn(),dispatchEvent:vi.fn()});vi.stubGlobal('document',{addEventListener:vi.fn(),removeEventListener:vi.fn(),visibilityState:'visible'});cancelRecording();
 const initial={revisionId:'r1',scene:{schemaVersion:1 as const,title:'Synthetic',document:{type:'doc',content:[{type:'paragraph',attrs:{lang:'ar',dir:'rtl'},content:[{type:'text',text:'مِ 中文 👩🏽‍🚀'}]}]}}};
 save=vi.fn(async(scene:any,parent:string)=>{expect(parent).toBe(head);order.push('save');head+='-saved';return {scene,revisionId:head};});
 props={initial,repository:{load:vi.fn(async()=>initial),save},flushRef:{current:null},onTitle:vi.fn(),performanceProbe:false,closeProbe:false,keyboardActive:true};
 editor={flush:vi.fn(async()=>{order.push('flush');}),lockSequenceInput:vi.fn((locked:boolean)=>order.push(locked?'lock':'unlock')),executeSequenceAction:vi.fn(async(action:any)=>{order.push(action.command??action.kind);const next=structuredClone(props.initial.scene.document);next.content![0].content![0].marks=[{type:action.command}];native().props.onChange(next);}),prepareClose:vi.fn(),resume:vi.fn()};
 render();startRecording(1,'Formatting');recordCommand({kind:'format',command:'bold'});recordCommand({kind:'format',command:'italic'});saveRecording();
});
afterEach(()=>{for(const slot of hooks.slots)slot?.cleanup?.();cancelRecording();vi.unstubAllGlobals();});
describe('Writer command sequence save barriers',()=>{
 it('uses acknowledged native actions and saves their exact rich documents independently before the next command',async()=>{
  await playCommandSequence(1,sequenceHook.executor);expect(order).toEqual(['flush','lock','flush','bold','flush','save','italic','flush','save','unlock']);
  expect(save.mock.calls[0][0].document.content[0].content[0].text).toBe('مِ 中文 👩🏽‍🚀');
  expect(save.mock.calls.map(([,parent]:any[])=>parent)).toEqual(['r1','r1-saved']);expect(editor.lockSequenceInput.mock.calls).toEqual([[true],[false]]);
 });
 it('an active title composition refuses playback before locking, executing or writing',async()=>{
  nodes(tree).find(node=>node.props?.className==='scene-title').props.onCompositionStart();
  await expect(playCommandSequence(1,sequenceHook.executor)).rejects.toThrow('Finish');expect(editor.executeSequenceAction).not.toHaveBeenCalled();expect(save).not.toHaveBeenCalled();expect(editor.lockSequenceInput).not.toHaveBeenCalledWith(true);
 });
 it('a failed native acknowledgment or CAS stops further playback and unlocks the existing editor',async()=>{
  save.mockRejectedValue(new Error('Conflict: exact head changed'));await expect(playCommandSequence(1,sequenceHook.executor)).rejects.toThrow('Conflict');
  expect(editor.executeSequenceAction).toHaveBeenCalledTimes(1);expect(editor.lockSequenceInput).toHaveBeenLastCalledWith(false);expect(props.flushRef.current).toBeTypeOf('function');
 });
 it('closing is refused during playback and changed keyboard ownership stops the next step',async()=>{
  let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;});editor.executeSequenceAction.mockImplementationOnce(async()=>{await gate;});
  const run=playCommandSequence(1,sequenceHook.executor);for(let i=0;i<20;i++)await Promise.resolve();
  await expect(props.flushRef.current!()).rejects.toThrow('sequence');props={...props,keyboardActive:false};render();release();
  await expect(run).rejects.toThrow('active draft changed');expect(editor.executeSequenceAction).toHaveBeenCalledTimes(1);expect(sequenceHook.enabled).toBe(false);expect(editor.lockSequenceInput).toHaveBeenLastCalledWith(false);
 });
});
