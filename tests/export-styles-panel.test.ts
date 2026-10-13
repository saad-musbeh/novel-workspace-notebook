import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[],dirty:false}));
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react');
 const changed=(a:any[],b:any[])=>!a||!b||a.length!==b.length||a.some((v,i)=>!Object.is(v,b[i]));
 const memo=(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;};
 return {...actual,useMemo:memo,useCallback:(fn:any,deps:any[])=>memo(()=>fn,deps),useRef:(value:any)=>{const i=hooks.cursor++;return hooks.slots[i]??={current:value};},useState:(initial:any)=>{const i=hooks.cursor++,state=hooks.slots[i]??={value:typeof initial==='function'?initial():initial};return [state.value,(next:any)=>{const value=typeof next==='function'?next(state.value):next;if(!Object.is(value,state.value)){state.value=value;hooks.dirty=true;}}];},useEffect:(effect:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps)){const entry={deps,cleanup:old?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=effect();});}}};
});
import {ExportStyles} from '../apps/writer/src/ExportStyles';
import {DEFAULT_EXPORT_PROFILE,type ExportProfile} from '../apps/writer/src/export-styles';

let tree:any,props:any,registered:(()=>Promise<void>)|null,reported:any[],profileByProject:Record<string,ExportProfile|null>,headsByProject:Record<string,string[]>;
function nodes(node:any):any[]{if(!node||typeof node!=='object')return [];if(Array.isArray(node))return node.flatMap(nodes);return [node,...nodes(node.props?.children)];}
function content(node:any):string{return Array.isArray(node)?node.map(content).join(''):typeof node==='object'?content(node?.props?.children):node==null?'':String(node);}
function render(){hooks.cursor=0;hooks.dirty=false;tree=ExportStyles(props);for(const effect of hooks.effects.splice(0))effect();}
function button(label:string){return nodes(tree).find(node=>node.type==='button'&&content(node)===label);}
function fontInput(part:string){const fieldset=nodes(tree).find(node=>node.type==='fieldset'&&nodes(node).some(child=>child.type==='legend'&&content(child)===part));return nodes(fieldset).find(node=>node.type==='input'&&node.props.type!=='checkbox');}
function preset(){return nodes(tree).find(node=>node.type==='select'&&node.props['aria-label']==='Starting Word layout');}
async function settle(predicate:()=>boolean){for(let i=0;i<100;i++){await new Promise(resolve=>setTimeout(resolve,0));if(hooks.dirty)render();if(predicate())return;}throw new Error('Word export styles panel did not settle');}
async function mount(projectId='project'){
 for(const slot of hooks.slots)slot?.cleanup?.();hooks.slots=[];hooks.effects=[];hooks.cursor=0;hooks.dirty=false;registered=null;reported=[];profileByProject={[projectId]:null};headsByProject={[projectId]:[]};
  props={projectId,rpc:async<T>(operation:string,args:Record<string,unknown>)=>{if(operation==='load_project_data'){const payload=profileByProject[String(args.projectId)];return (payload?{payload:structuredClone(payload),revisionId:'r-'+args.projectId}:null) as T;}if(operation==='project_data_heads')return {revisionIds:headsByProject[String(args.projectId)]??[]} as T;if(operation==='save_project_data'){if(args.parent!==(profileByProject[String(args.projectId)]?'r-'+args.projectId:null))throw new Error('stale parent');profileByProject[String(args.projectId)]=structuredClone(args.payload as ExportProfile);return {payload:structuredClone(args.payload),revisionId:'saved-'+args.projectId} as T;}throw new Error('Unexpected RPC '+operation);},onRegisterFlush:(flush:(()=>Promise<void>)|null)=>{registered=flush;},onStatus:(revision:string|null,ready:boolean,conflicted:boolean)=>reported.push({revision,ready,conflicted})};
 render();await settle(()=>Boolean(preset()&&registered));
}
beforeEach(()=>{hooks.slots=[];hooks.effects=[];hooks.cursor=0;hooks.dirty=false;vi.stubGlobal('window',{confirm:vi.fn(()=>true)});});
afterEach(()=>{for(const slot of hooks.slots)slot?.cleanup?.();vi.unstubAllGlobals();});

describe('Word export styles panel lifecycle',()=>{
 it('retains the edited font when its project-data CAS save fails',async()=>{
  await mount();const input=fontInput('Body text');expect(input).toBeTruthy();input.props.onChange({target:{value:'Noto Serif'}});render();
  const save=props.rpc;props.rpc=async<T>(operation:string,args:Record<string,unknown>)=>{if(operation==='save_project_data')throw new Error('Concurrent style version');return save(operation,args);};render();
  await expect(registered!()).rejects.toThrow('Concurrent style version');await settle(()=>nodes(tree).some(node=>node.props?.role==='alert'));
  expect(fontInput('Body text').props.value).toBe('Noto Serif');expect(profileByProject.project).toBeNull();
 });

 it('ignores a delayed old-project load after the panel has switched projects',async()=>{
  let releaseOld!:(value:any)=>void;
  props={projectId:'old',rpc:async<T>(operation:string,args:Record<string,unknown>)=>{if(operation==='load_project_data')return new Promise<T>(resolve=>{releaseOld=resolve;});if(operation==='project_data_heads')return {revisionIds:[]} as T;throw new Error(operation);},onRegisterFlush:(flush:(()=>Promise<void>)|null)=>{registered=flush;},onStatus:(revision:string|null,ready:boolean,conflicted:boolean)=>reported.push({revision,ready,conflicted})};
  render();
  props={...props,projectId:'new',rpc:async<T>(operation:string)=> (operation==='load_project_data'?{revisionId:'r-new',payload:{...structuredClone(DEFAULT_EXPORT_PROFILE),body:{...DEFAULT_EXPORT_PROFILE.body,font:'New project font'}}}:{revisionIds:['r-new']}) as T};render();
  await settle(()=>fontInput('Body text')?.props.value==='New project font');
  releaseOld({revisionId:'r-old',payload:{...structuredClone(DEFAULT_EXPORT_PROFILE),body:{...DEFAULT_EXPORT_PROFILE.body,font:'Stale old font'}}});await new Promise(resolve=>setTimeout(resolve,0));if(hooks.dirty)render();
  expect(fontInput('Body text').props.value).toBe('New project font');expect(reported.at(-1)).toMatchObject({revision:'r-new',ready:true,conflicted:false});
 });

 it('ignores a delayed old-project save after switching projects',async()=>{
  await mount('old');let releaseSave!:(value:any)=>void;const original=props.rpc;
  props.rpc=async<T>(operation:string,args:Record<string,unknown>)=>operation==='save_project_data'?new Promise<T>(resolve=>{releaseSave=resolve;}):original(operation,args);
  fontInput('Body text').props.onChange({target:{value:'Pending old font'}});render();
  const flushing=registered!();await settle(()=>Boolean(releaseSave));
  props={...props,projectId:'new',rpc:async<T>(operation:string)=>operation==='load_project_data'?{revisionId:'r-new',payload:{...structuredClone(DEFAULT_EXPORT_PROFILE),body:{...DEFAULT_EXPORT_PROFILE.body,font:'Fresh project font'}}} as T:{revisionIds:['r-new']} as T};render();await settle(()=>fontInput('Body text')?.props.value==='Fresh project font');
  releaseSave({revisionId:'saved-old',payload:{...structuredClone(DEFAULT_EXPORT_PROFILE),body:{...DEFAULT_EXPORT_PROFILE.body,font:'Pending old font'}}});await flushing;await new Promise(resolve=>setTimeout(resolve,0));if(hooks.dirty)render();
  expect(fontInput('Body text').props.value).toBe('Fresh project font');expect(reported.at(-1)).toMatchObject({revision:'r-new',ready:true,conflicted:false});
 });

 it('lets clean conflict-resolution recovery drain run without saving or blocking it',async()=>{
  await mount();headsByProject.project=['head-a','head-b'];
  // Reload reveals the conflict and leaves the profile clean; Recovery actions call this same drain.
  button('Reload saved styles').props.onClick();await settle(()=>reported.at(-1)?.conflicted===true);
  await expect(registered!()).resolves.toBeUndefined();expect(profileByProject.project).toBeNull();
 });

 it('keeps a dirty profile when the user cancels reload confirmation',async()=>{
  await mount();fontInput('Body text').props.onChange({target:{value:'Keep this draft'}});render();
  (window.confirm as any).mockReturnValue(false);button('Reload saved styles').props.onClick();await new Promise(resolve=>setTimeout(resolve,0));
  expect(window.confirm).toHaveBeenCalledOnce();expect(fontInput('Body text').props.value).toBe('Keep this draft');expect(profileByProject.project).toBeNull();
 });
});
