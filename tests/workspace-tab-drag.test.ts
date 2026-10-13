import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {TAB_DRAG_TYPE,SCENE_DRAG_TYPE,type WindowTabs,type WorkspaceTab} from '../apps/writer/src/workspace-tabs';
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[],dirty:false}));
const native=vi.hoisted(()=>({invoke:vi.fn(),listeners:new Map<string,{handle:(event:any)=>void;target:string}>()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:native.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async(event:string,handle:any,options:any)=>{native.listeners.set(event,{handle,target:options.target});return ()=>native.listeners.delete(event);})}));
vi.mock('../apps/writer/src/DesktopWorkspace',()=>({DesktopWorkspace:function DesktopWorkspace(){return null;}}));
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react');
 const changed=(a:any[],b:any[])=>!a||!b||a.length!==b.length||a.some((v,i)=>!Object.is(v,b[i]));
 const memo=(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;};
 return {...actual,useCallback:(fn:any,deps:any[])=>memo(()=>fn,deps),useRef:(value:any)=>{const i=hooks.cursor++;return hooks.slots[i]??={current:value};},useState:(initial:any)=>{const i=hooks.cursor++;const state=hooks.slots[i]??={value:typeof initial==='function'?initial():initial};return [state.value,(next:any)=>{const value=typeof next==='function'?next(state.value):next;if(!Object.is(value,state.value)){state.value=value;hooks.dirty=true;}}];},useLayoutEffect:(effect:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps)){const entry={deps,cleanup:old?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=effect();});}},useEffect:(effect:()=>any,deps:any[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||changed(old.deps,deps)){const entry={deps,cleanup:old?.cleanup};hooks.slots[i]=entry;hooks.effects.push(()=>{entry.cleanup?.();entry.cleanup=effect();});}}};
});
import {bindingFromEvent,defaultShortcutSettings,reloadShortcutSettings,saveShortcutSettings,updateShortcutBindings} from '../apps/writer/src/shortcuts';
import {WorkspaceTabs} from '../apps/writer/src/WorkspaceTabs';
const tab=(id:string):WorkspaceTab=>({id,title:id,route:{projectId:'synthetic',sceneId:id,view:'write'}});
let autoMountedFlush=false;
let state:WindowTabs,tree:any,keyboard:Map<string,(event:any)=>void>,flushRef:{current:(()=>Promise<void>)|null};
function nodes(node:any):any[]{if(!node||typeof node!=='object')return [];if(Array.isArray(node))return node.flatMap(nodes);return [node,...nodes(node.props?.children)];}
function render(){hooks.cursor=0;hooks.dirty=false;tree=WorkspaceTabs({flushRef});if(autoMountedFlush)for(const node of nodes(tree))if(node.props?.tabId&&node.props?.flushRef&&!node.props.flushRef.current)node.props.flushRef.current=async()=>{};for(const effect of hooks.effects.splice(0))effect();return tree;}
function tabButton(id:string){return nodes(tree).find(node=>node.props?.['data-tab-id']===id);}
function strip(){return nodes(tree).find(node=>node.props?.role==='tablist');}
function workspace(id:string){return nodes(tree).find(node=>node.props?.tabId===id);}
async function settle(predicate:()=>boolean=()=>true){for(let i=0;i<100;i++){await new Promise(resolve=>setTimeout(resolve,0));if(hooks.dirty)render();if(predicate()&&!hooks.dirty)return;}throw new Error('Coordinator failed to settle');}
async function open(){render();await settle(()=>Boolean(tabButton('a')));for(const item of state.tabs)workspace(item.id).props.flushRef.current=vi.fn(async()=>{});strip().props.ref.current={getBoundingClientRect:()=>({left:0,right:600,top:0,bottom:44})};}
function emit(name:string,payload:any){native.listeners.get(name)!.handle({payload});}
function transferData(value=''){const values=new Map<string,string>();if(value)values.set(TAB_DRAG_TYPE,value);return {types:[TAB_DRAG_TYPE],dropEffect:'none',effectAllowed:'none',setData:(kind:string,text:string)=>values.set(kind,text),getData:(kind:string)=>values.get(kind)??''};}
function keyEvent(overrides:Record<string,unknown>){return {key:'',code:'',ctrlKey:false,altKey:false,shiftKey:false,metaKey:false,isComposing:false,repeat:false,defaultPrevented:false,getModifierState:()=>false,preventDefault:vi.fn(),...overrides};}
function dragEvent(data:any,x=100,y=20){return {dataTransfer:data,clientX:x,clientY:y,preventDefault:vi.fn(),target:{closest:()=>null}};}
beforeEach(()=>{
 autoMountedFlush=false;hooks.slots=[];hooks.cursor=0;hooks.effects=[];hooks.dirty=false;native.listeners.clear();native.invoke.mockReset();flushRef={current:null};state={windowLabel:'main',tabs:[tab('a'),tab('b')],incoming:[]};keyboard=new Map();
 const saved=new Map<string,string>();vi.stubGlobal('localStorage',{getItem:(key:string)=>saved.get(key)??null,setItem:(key:string,value:string)=>saved.set(key,value)});
 vi.stubGlobal('window',{innerWidth:1200,innerHeight:900,dispatchEvent:(event:Event)=>{keyboard.get(event.type)?.(event);return true;},addEventListener:(name:string,handle:any)=>keyboard.set(name,handle),removeEventListener:vi.fn()});
 reloadShortcutSettings();
 native.invoke.mockImplementation(async(command,args)=>{if(command==='workspace_window_state')return structuredClone(state);if(command==='workspace_command'&&args.operation==='load_project_scene')return {id:args.args.sceneId,scene:{title:'Canonical '+args.args.sceneId}};if(command==='register_tab'){const existing=state.tabs.findIndex(tab=>tab.id===args.tab.id);if(existing>=0)state.tabs[existing]=args.tab;else state.tabs.push(args.tab);return {tab:args.tab,ownerWindow:state.windowLabel,existing:false};}if(command==='unregister_tab')state.tabs=state.tabs.filter(tab=>tab.id!==args.tabId);return null;});
});
afterEach(()=>{for(const slot of hooks.slots)slot?.cleanup?.();vi.unstubAllGlobals();vi.restoreAllMocks();});
describe('browser style tab drag coordinator',()=>{
 it('moves the exact non-active source tab only after its drain and UI registration',async()=>{
  await open();const order:string[]=[];let release!:()=>void;
  workspace('a').props.flushRef.current=vi.fn(async()=>{order.push('wrong-active');});
  workspace('b').props.flushRef.current=async()=>{order.push('drain');await new Promise<void>(resolve=>{release=resolve;});};
  const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>{if(command==='register_tab')order.push('register-'+args.tab.id);if(command==='workspace_transfer_tab')order.push('transfer-'+args.tabId);return original(command,args);});native.invoke.mockClear();
  emit('workspace-tab-move-request',{tabId:'b',destinationWindow:'workspace-destination'});await settle(()=>order.includes('drain'));
  expect(order).toEqual(['drain']);expect(native.invoke).not.toHaveBeenCalledWith('workspace_transfer_tab',expect.anything());
  release();await settle(()=>order.includes('transfer-b'));expect(order).toEqual(['drain','register-b','transfer-b']);
  expect(native.invoke).toHaveBeenCalledWith('workspace_transfer_tab',{tabId:'b',destination:'window:workspace-destination'});expect(tabButton('b')).toBeDefined();
  expect(native.listeners.get('workspace-tab-move-request')?.target).toBe('main');
 });
 it('captures outgoing native scroll metadata but never restores source CSS pixels on incoming readiness',async()=>{
  await open();const outgoing=vi.fn((selector:string)=>selector==='.native-scroll'?{scrollTop:321}:null);nodes(tree).find(node=>node.props?.role==='tabpanel'&&node.props.id==='panel-b').props.ref({querySelector:outgoing});
  emit('workspace-tab-move-request',{tabId:'b',destinationWindow:'workspace-destination'});await settle(()=>native.invoke.mock.calls.some(([command])=>command==='workspace_transfer_tab'));
  expect(outgoing).toHaveBeenCalledWith('.native-scroll');expect(native.invoke).toHaveBeenCalledWith('register_tab',expect.objectContaining({tab:expect.objectContaining({id:'b',ui:{scrollTop:321}})}));
  emit('workspace-transfer-failed',{error:'Synthetic rollback'});await settle();
  const incoming={...tab('incoming'),ui:{scrollTop:999}};state.incoming=[{transferId:'incoming-transfer',tab:incoming,phase:'awaiting_ready'}];emit('workspace-tabs-changed',{});await settle(()=>Boolean(workspace('incoming')));
  const rawCssRestore=vi.fn(()=>({scrollTop:0,dispatchEvent:vi.fn()}));nodes(tree).find(node=>node.props?.role==='tabpanel'&&node.props.id==='panel-incoming').props.ref({querySelector:rawCssRestore});
  const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>{if(command==='workspace_transfer_ready'){state.tabs.push(incoming);state.incoming=[];return null;}return original(command,args);});
  workspace('incoming').props.onContentReady();await settle(()=>tabButton('incoming')?.props['aria-selected']);
  expect(native.invoke).toHaveBeenCalledWith('workspace_transfer_ready',{transferId:'incoming-transfer'});expect(rawCssRestore).not.toHaveBeenCalled();
 });
 it('refuses stale/non-owned/incoming/self-destination requests and retains source on failed drain',async()=>{
  await open();native.invoke.mockClear();
  for(const payload of [{tabId:'missing',destinationWindow:'other'},{tabId:'a',destinationWindow:'main'}])emit('workspace-tab-move-request',payload);
  await settle();expect(native.invoke).not.toHaveBeenCalled();
  workspace('b').props.flushRef.current=async()=>{throw new Error('Synthetic CAS conflict');};emit('workspace-tab-move-request',{tabId:'b',destinationWindow:'other'});await settle();
  expect(native.invoke).not.toHaveBeenCalledWith('workspace_transfer_tab',expect.anything());expect(tabButton('b')).toBeDefined();
 });
 it('Escape and cancellation coordinates prevent detachment, while a deliberate outside drop uses dragged ID',async()=>{
  await open();native.invoke.mockClear();let data=transferData();tabButton('b').props.onDragStart(dragEvent(data));keyboard.get('keydown')!(keyEvent({key:'Escape'}));tabButton('b').props.onDragEnd(dragEvent(data,100,130));await settle();expect(native.invoke).not.toHaveBeenCalled();
  data=transferData();tabButton('b').props.onDragStart(dragEvent(data));tabButton('b').props.onDragEnd(dragEvent(data,0,0));await settle();expect(native.invoke).not.toHaveBeenCalled();
  data=transferData();tabButton('b').props.onDragStart(dragEvent(data));tabButton('b').props.onDragEnd(dragEvent(data,100,130));await settle(()=>native.invoke.mock.calls.some(([command])=>command==='workspace_transfer_tab'));
  expect(native.invoke).toHaveBeenCalledWith('workspace_transfer_tab',{tabId:'b',destination:'new'});
 });
 it('reorders without draining or transfers and preserves the order after a native refresh',async()=>{
  await open();native.invoke.mockClear();const data=transferData();tabButton('a').props.onDragStart(dragEvent(data));strip().props.onDragOver(dragEvent(data,550,20));render();await strip().props.onDrop(dragEvent(data,550,20));tabButton('a').props.onDragEnd(dragEvent(data,550,20));await settle();
  expect(nodes(tree).filter(node=>node.props?.role==='tab').map(node=>node.props['data-tab-id'])).toEqual(['b','a']);expect(native.invoke).not.toHaveBeenCalled();
  emit('workspace-tabs-changed',{});await settle();expect(nodes(tree).filter(node=>node.props?.role==='tab').map(node=>node.props['data-tab-id'])).toEqual(['b','a']);
 });
 it('requests foreign ownership through native authority without draining destination or accepting payload destinations',async()=>{
  state.windowLabel='workspace-destination';await open();native.invoke.mockClear();
  const data=transferData(JSON.stringify({tabId:'remote-tab',sourceWindow:'main',destinationWindow:'forged'}));await strip().props.onDrop(dragEvent(data));await settle();
  expect(native.invoke.mock.calls).toEqual([['workspace_request_tab_move',{tabId:'remote-tab'}]]);
  expect(workspace('a').props.flushRef.current).not.toHaveBeenCalled();
 });
 it('closes only after committed transfer and refreshed empty state, retaining a nonempty source',async()=>{
  await open();native.invoke.mockClear();state.tabs=[tab('a')];emit('workspace-tab-transferred',{tabId:'b'});await settle();expect(native.invoke).not.toHaveBeenCalledWith('workspace_close_empty_window');
  state.tabs=[];emit('workspace-tab-transferred',{tabId:'a'});await settle(()=>native.invoke.mock.calls.some(([command])=>command==='workspace_close_empty_window'));
  expect(native.invoke.mock.calls.map(([command])=>command)).toEqual(['workspace_window_state','workspace_window_state','workspace_close_empty_window']);
 });
 it('retains incoming transfers and provisional workspaces rather than closing the source',async()=>{
  await open();native.invoke.mockClear();state.tabs=[];state.incoming=[{transferId:'incoming',tab:tab('pending'),phase:'preparing'}];
  emit('workspace-tab-transferred',{tabId:'a'});emit('workspace-tab-transferred',{tabId:'b'});await settle();expect(native.invoke).not.toHaveBeenCalledWith('workspace_close_empty_window');
  keyboard.get('keydown')!(keyEvent({key:'t',code:'KeyT',ctrlKey:true,preventDefault:vi.fn()}));await settle();expect(nodes(tree).filter(node=>node.props?.role==='tab')).toHaveLength(1);
  state.incoming=[];emit('workspace-tabs-changed',{});await settle();emit('workspace-tab-transferred',{tabId:'missing'});await settle();expect(native.invoke).not.toHaveBeenCalledWith('workspace_close_empty_window');
 });
 it('reserves the local empty window during native close and resumes local actions if closure fails',async()=>{
  await open();let rejectClose!:(error:Error)=>void;const original=native.invoke.getMockImplementation()!;
  native.invoke.mockImplementation(async(command,args)=>command==='workspace_close_empty_window'?new Promise((_resolve,reject)=>{rejectClose=reject;}):original(command,args));
  state.tabs=[];emit('workspace-tab-transferred',{tabId:'a'});emit('workspace-tab-transferred',{tabId:'b'});await settle(()=>Boolean(rejectClose));
  keyboard.get('keydown')!(keyEvent({key:'t',code:'KeyT',ctrlKey:true,preventDefault:vi.fn()}));await settle();expect(nodes(tree).filter(node=>node.props?.role==='tab')).toHaveLength(0);
  rejectClose(new Error('Synthetic closure refused'));await settle();keyboard.get('keydown')!(keyEvent({key:'t',code:'KeyT',ctrlKey:true,preventDefault:vi.fn()}));await settle();expect(nodes(tree).filter(node=>node.props?.role==='tab')).toHaveLength(1);
 });
 it('keeps an acknowledged new tool active when a pre-registration native refresh finishes late',async()=>{
  await open();const original=native.invoke.getMockImplementation()!,old=structuredClone(state);let stale!:()=>void;
  native.invoke.mockImplementation(async(command,args)=>{
   if(command==='register_tab'&&args.tab.route.view==='alphabets'){
    emit('workspace-tabs-changed',{});await Promise.resolve();return original(command,args);
   }
   if(command==='workspace_window_state'&&!stale)return new Promise(resolve=>{stale=()=>resolve(old);});
   return original(command,args);
  });
  await workspace('a').props.onOpenRoute({projectId:'synthetic',sceneId:'a',view:'alphabets'},'Alphabets');await settle(()=>nodes(tree).some(node=>node.props?.role==='tab'&&node.props['aria-selected']&&node.props.children==='Alphabets'));
  const opened=nodes(tree).find(node=>node.props?.role==='tab'&&node.props['aria-selected']);expect(opened.props.children).toBe('Alphabets');stale();await settle();
  expect(tabButton(opened.props['data-tab-id']).props['aria-selected']).toBe(true);expect(tabButton('a').props['aria-selected']).toBe(false);expect(workspace(opened.props['data-tab-id']).props.initialRoute.view).toBe('alphabets');
 });
 it('does not steal a newly selected tool when a hidden duplicate registration finishes late',async()=>{
  await open();const original=native.invoke.getMockImplementation()!,route={projectId:'synthetic',sceneId:'a',view:'alphabets'} as const;
  await workspace('a').props.onOpenRoute(route,'Alphabets');await settle(()=>nodes(tree).some(node=>node.props?.role==='tab'&&node.props['aria-selected']&&node.props.children==='Alphabets'));
  const selected=nodes(tree).find(node=>node.props?.role==='tab'&&node.props['aria-selected']).props['data-tab-id'];native.invoke.mockImplementation(async(command,args)=>command==='register_tab'&&args.tab.id==='b'?{tab:state.tabs[0],ownerWindow:'main',existing:true}:original(command,args));native.invoke.mockClear();
  workspace('b').props.onRouteReady(tab('a').route,'Duplicate old draft');await settle(()=>!tabButton('b'));
  expect(native.invoke.mock.calls.some(([command])=>command==='workspace_focus_tab')).toBe(false);expect(tabButton(selected).props['aria-selected']).toBe(true);
 });
 it('replaces the Window dropdown with accessible exact-tab context actions',async()=>{
  await open();native.invoke.mockClear();const b=nodes(tree).find(node=>node.props?.['data-workspace-tab']==='b');b.props.onContextMenu({preventDefault:vi.fn(),clientX:180,clientY:30});await settle();
  expect(nodes(tree).some(node=>node.type==='summary')).toBe(false);expect(nodes(tree).find(node=>node.props?.role==='menu')?.props['aria-label']).toBe('Tab actions');
  const save=nodes(tree).find(node=>node.props?.role==='menuitem'&&node.props.children==='Save tab');save.props.onClick();await settle();expect(workspace('b').props.flushRef.current).toHaveBeenCalledOnce();expect(workspace('a').props.flushRef.current).not.toHaveBeenCalled();
  tabButton('b').props.onKeyDown({key:'F10',shiftKey:true,preventDefault:vi.fn(),stopPropagation:vi.fn(),currentTarget:{getBoundingClientRect:()=>({left:100,bottom:44})}});await settle();expect(nodes(tree).find(node=>node.props?.role==='menu')).toBeDefined();keyboard.get('keydown')!(keyEvent({key:'Escape'}));await settle();expect(nodes(tree).find(node=>node.props?.role==='menu')).toBeUndefined();
 });
 it('uses CtrlW to drain/close the current tab and CtrlT to open a provisional workspace',async()=>{
  await open();const drain=workspace('a').props.flushRef.current;native.invoke.mockClear();keyboard.get('keydown')!(keyEvent({key:'w',code:'KeyW',ctrlKey:true,preventDefault:vi.fn()}));await settle();
  expect(drain).toHaveBeenCalledOnce();expect(native.invoke).toHaveBeenCalledWith('unregister_tab',{tabId:'a'});expect(tabButton('a')).toBeUndefined();expect(tabButton('b')).toBeDefined();
  keyboard.get('keydown')!(keyEvent({key:'t',code:'KeyT',ctrlKey:true,preventDefault:vi.fn()}));await settle();expect(nodes(tree).filter(node=>node.props?.role==='tab')).toHaveLength(2);
 });

 it('reacquires the acknowledged title and tool route after draining queued registration before moving',async()=>{
  await open();native.invoke.mockClear();const route={projectId:'synthetic',sceneId:'b-current',view:'library'};
  workspace('b').props.flushRef.current=async()=>{workspace('b').props.onRouteReady(route,'Acknowledged title مِ 中文');};
  emit('workspace-tab-move-request',{tabId:'b',destinationWindow:'workspace-destination'});await settle(()=>native.invoke.mock.calls.some(([command])=>command==='workspace_transfer_tab'));
  const bindings=native.invoke.mock.calls.filter(([command])=>command==='register_tab');expect(bindings).toHaveLength(2);expect(bindings[1][1].tab).toMatchObject({id:'b',title:'Acknowledged title مِ 中文',route});
  expect(native.invoke).toHaveBeenCalledWith('workspace_transfer_tab',{tabId:'b',destination:'window:workspace-destination'});
 });

 it('uses remapped close bindings exclusively and physical Arabic key codes while refusing composition',async()=>{
  await open();native.invoke.mockClear();const remapped=keyEvent({key:'F12',code:'F12',ctrlKey:true});
  saveShortcutSettings(updateShortcutBindings(defaultShortcutSettings(),'workspace.closeTab',[bindingFromEvent(remapped)!]));
  keyboard.get('keydown')!(keyEvent({key:'ص',code:'KeyW',ctrlKey:true}));await settle();expect(native.invoke).not.toHaveBeenCalled();
  keyboard.get('keydown')!(keyEvent({key:'F12',code:'F12',ctrlKey:true,isComposing:true}));await settle();expect(native.invoke).not.toHaveBeenCalled();
  keyboard.get('keydown')!(remapped);await settle();expect(native.invoke).toHaveBeenCalledWith('unregister_tab',{tabId:'a'});
  saveShortcutSettings(defaultShortcutSettings());native.invoke.mockClear();keyboard.get('keydown')!(keyEvent({key:'ص',code:'KeyW',ctrlKey:true}));await settle();expect(native.invoke).toHaveBeenCalledWith('unregister_tab',{tabId:'b'});
 });
 it('dispatches next/previous tab, fullscreen and shortcut settings without competing hardcoded handlers',async()=>{
  await open();native.invoke.mockClear();keyboard.get('keydown')!(keyEvent({key:'Tab',code:'Tab',ctrlKey:true}));await settle();expect(tabButton('b').props['aria-selected']).toBe(true);
  keyboard.get('keydown')!(keyEvent({key:'Tab',code:'Tab',ctrlKey:true,shiftKey:true}));await settle();expect(tabButton('a').props['aria-selected']).toBe(true);
  keyboard.get('keydown')!(keyEvent({key:'F11',code:'F11'}));await settle();expect(native.invoke.mock.calls).toEqual([['workspace_toggle_fullscreen']]);
  keyboard.get('keydown')!(keyEvent({key:'F1',code:'F1'}));await settle();expect(nodes(tree).some(node=>node.type?.name==='ShortcutSettings')).toBe(true);native.invoke.mockClear();
  keyboard.get('keydown')!(keyEvent({key:'w',code:'KeyW',ctrlKey:true}));await settle();expect(native.invoke).not.toHaveBeenCalled();
  const dialog=nodes(tree).find(node=>node.type?.name==='ShortcutSettings');expect(nodes(tree).find(node=>node.type==='header').props.inert).toBe(true);expect(nodes(tree).filter(node=>node.props?.role==='tabpanel').every(node=>node.props.inert)).toBe(true);dialog.props.onClose();await settle();expect(nodes(tree).some(node=>node.type?.name==='ShortcutSettings')).toBe(false);
 });

 it('shows two editable distinct native tab identities and sends global Save to the focused pane',async()=>{
  await open();const beforeA=workspace('a').props.flushRef,beforeB=workspace('b').props.flushRef;
  nodes(tree).find(node=>node.type==='button'&&node.props.children==='Split').props.onClick();await settle();
  const panels=nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden);expect(panels).toHaveLength(2);
  expect(panels.map(node=>node.props['data-draft-pane'])).toEqual(['primary','secondary']);
  expect(workspace('a').props).toMatchObject({tabId:'a',tabActive:true,keyboardActive:false,compact:false});expect(workspace('b').props).toMatchObject({tabId:'b',tabActive:true,keyboardActive:true,compact:true});
  expect(workspace('a').props.flushRef).toBe(beforeA);expect(workspace('b').props.flushRef).toBe(beforeB);beforeA.current.mockClear();beforeB.current.mockClear();
  panels[0].props.onFocusCapture();await settle();keyboard.get('keydown')!(keyEvent({key:'s',code:'KeyS',ctrlKey:true}));await settle();expect(beforeA.current).toHaveBeenCalledOnce();expect(beforeB.current).not.toHaveBeenCalled();
  panels[1].props.onPointerDownCapture();await settle();keyboard.get('keydown')!(keyEvent({key:'s',code:'KeyS',ctrlKey:true}));await settle();expect(beforeB.current).toHaveBeenCalledOnce();
  expect(workspace('a').props.tabActive).toBe(true);expect(workspace('b').props.tabActive).toBe(true);
 });
 it('retains both drafts and controller identities when leaving split cannot drain, then hides only the other pane',async()=>{
  await open();nodes(tree).find(node=>node.type==='button'&&node.props.children==='Split').props.onClick();await settle();const a=workspace('a').props.flushRef,b=workspace('b').props.flushRef;
  a.current=async()=>{throw new Error('Synthetic composition still active');};nodes(tree).find(node=>node.type==='button'&&node.props.children==='Single view').props.onClick();await settle();expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(2);
  a.current=vi.fn(async()=>{});nodes(tree).find(node=>node.type==='button'&&node.props.children==='Single view').props.onClick();await settle();
  expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden).map(node=>node.props.id)).toEqual(['panel-b']);expect(workspace('a').props.flushRef).toBe(a);expect(workspace('b').props.flushRef).toBe(b);
 });
 it('collapses after committed movement of one pane and keeps the other exact session editable',async()=>{
  await open();nodes(tree).find(node=>node.type==='button'&&node.props.children==='Split').props.onClick();await settle();native.invoke.mockClear();const a=workspace('a').props.flushRef;
  emit('workspace-tab-move-request',{tabId:'b',destinationWindow:'other'});await settle(()=>native.invoke.mock.calls.some(([command])=>command==='workspace_transfer_tab'));
  expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(2);state.tabs=[tab('a')];emit('workspace-tab-transferred',{tabId:'b'});await settle();
  expect(workspace('a').props.flushRef).toBe(a);expect(workspace('a').props).toMatchObject({tabActive:true,keyboardActive:true});expect(tabButton('b')).toBeUndefined();expect(native.invoke).not.toHaveBeenCalledWith('workspace_close_empty_window');
 });
 it('opens an existing tab beside from context and adjusts the accessible divider without remounting drafts',async()=>{
  await open();nodes(tree).find(node=>node.props?.['data-workspace-tab']==='b').props.onContextMenu({preventDefault:vi.fn(),clientX:200,clientY:30});await settle();nodes(tree).find(node=>node.props?.role==='menuitem'&&node.props.children==='Open beside').props.onClick();await settle();
  let separator=nodes(tree).find(node=>node.props?.role==='separator');expect(separator.props).toMatchObject({'aria-label':'Draft pane divider','aria-orientation':'vertical','aria-valuenow':50});const a=workspace('a').props.flushRef;
  separator.props.onKeyDown({key:'ArrowRight',preventDefault:vi.fn()});await settle();separator=nodes(tree).find(node=>node.props?.role==='separator');expect(separator.props['aria-valuenow']).toBe(52);
  separator.props.onKeyDown({key:'End',preventDefault:vi.fn()});await settle();separator=nodes(tree).find(node=>node.props?.role==='separator');expect(separator.props['aria-valuenow']).toBe(separator.props['aria-valuemax']);expect(workspace('a').props.flushRef).toBe(a);
 });
 it('resolves an exact binder scene to the existing owned tab without duplicate registration',async()=>{
  await open();const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:tab('b'),ownerWindow:'main'}:original(command,args));native.invoke.mockClear();
  await workspace('a').props.onOpenBeside(tab('b').route,'b');await settle();expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(2);expect(native.invoke).not.toHaveBeenCalledWith('register_tab',expect.anything());expect(nodes(tree).filter(node=>node.props?.role==='tab')).toHaveLength(2);
 });

 it('keeps the primary draft single when a remote beside request is refused',async()=>{
  state.tabs=[tab('a')];await open();const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>{if(command==='workspace_find_tab')return {tab:tab('b'),ownerWindow:'other'};if(command==='workspace_request_tab_move')throw new Error('Synthetic source refuses move');return original(command,args);});
  await workspace('a').props.onOpenBeside(tab('b').route,'b');await settle();expect(workspace('a').props.tabActive).toBe(true);expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(1);
  state.tabs.push(tab('b'));emit('workspace-tabs-changed',{});await settle();expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(1);
 });
 it('expires an unacknowledged source-drain request and never applies its stale split intent later',async()=>{
  state.tabs=[tab('a')];await open();const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:tab('b'),ownerWindow:'other'}:original(command,args));
  let expire!:(...args:any[])=>void;const originalTimer=globalThis.setTimeout;vi.spyOn(globalThis,'setTimeout').mockImplementation(((callback:any,ms:any,...args:any[])=>{if(ms===40000){expire=callback;return 0;}return originalTimer(callback,ms,...args);}) as any);
  await workspace('a').props.onOpenBeside(tab('b').route,'b');await settle();expect(expire).toBeDefined();expire();await settle();expect(workspace('a').props.tabActive).toBe(true);
  state.tabs.push(tab('b'));emit('workspace-tabs-changed',{});await settle();expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(1);
 });
 it('clears beside intent after an incoming handoff rolls back and closing a visible secondary keeps primary editable',async()=>{
  state.tabs=[tab('a')];await open();const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:tab('b'),ownerWindow:'other'}:original(command,args));
  await workspace('a').props.onOpenBeside(tab('b').route,'b');await settle();state.incoming=[{transferId:'fixture-move',tab:tab('b'),phase:'preparing'}];emit('workspace-tabs-changed',{});await settle();state.incoming=[];emit('workspace-tabs-changed',{});await settle();
  state.tabs.push(tab('b'));emit('workspace-tabs-changed',{});await settle();workspace('b').props.flushRef.current=vi.fn(async()=>{});expect(nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden)).toHaveLength(1);
  nodes(tree).find(node=>node.type==='button'&&node.props.children==='Split').props.onClick();await settle();const a=workspace('a').props.flushRef;
  nodes(tree).find(node=>node.type==='button'&&node.props['aria-label']==='Close b').props.onClick();await settle();expect(workspace('a').props.flushRef).toBe(a);expect(workspace('a').props).toMatchObject({tabActive:true,keyboardActive:true});expect(tabButton('b')).toBeUndefined();
 });

});

describe('drag to arrange draft and tool panes',()=>{
 const zone=(side:string)=>nodes(tree).find(node=>node.props?.['data-pane-drop']===side);
 const paneIds=()=>nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden).map(node=>node.props.id);
 it('drops the exact tab into the requested left pane, drains before visibility and never detaches after a successful pane drop',async()=>{
  await open();const data=transferData(),order:string[]=[];let release!:()=>void;const a=workspace('a').props.flushRef,b=workspace('b').props.flushRef;
  a.current=async()=>{order.push('a');await new Promise<void>(resolve=>{release=resolve;});};b.current=async()=>{order.push('b');};
  tabButton('b').props.onDragStart(dragEvent(data));await settle();const event={...dragEvent(data,100,200),stopPropagation:vi.fn()};zone('left').props.onDragOver(event);await settle();
  const pending=zone('left').props.onDrop(event);await settle(()=>order.includes('a'));expect(paneIds()).toEqual(['panel-a']);release();await pending;await settle();
  expect(order).toEqual(['a','b']);expect(paneIds()).toEqual(['panel-a','panel-b']);expect(nodes(tree).find(node=>node.props?.id==='panel-b').props['data-draft-pane']).toBe('primary');expect(tabButton('b').props['aria-selected']).toBe(true);
  tabButton('b').props.onDragEnd(dragEvent(data,100,200));await settle();expect(native.invoke).not.toHaveBeenCalledWith('workspace_transfer_tab',expect.anything());expect(workspace('a').props.flushRef).toBe(a);expect(workspace('b').props.flushRef).toBe(b);
 });
 it('retains every old pane and its unsaved controller when a replaced pane cannot drain',async()=>{
  state.tabs.push(tab('c'));await open();nodes(tree).find(node=>node.type==='button'&&node.props.children==='Split').props.onClick();await settle();const a=workspace('a').props.flushRef,b=workspace('b').props.flushRef;
  a.current=async()=>{throw new Error('Synthetic dirty left draft');};const data=transferData();tabButton('c').props.onDragStart(dragEvent(data));await settle();await zone('left').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle();
  expect(paneIds()).toEqual(['panel-a','panel-b']);expect(workspace('a').props.flushRef).toBe(a);expect(workspace('b').props.flushRef).toBe(b);expect(nodes(tree).some(node=>node.props?.role==='alert'&&String(node.props.children).includes('dirty left'))).toBe(true);expect(native.invoke).not.toHaveBeenCalledWith('workspace_transfer_tab',expect.anything());
 });
 it('docks an existing planning tool beside the draft with independent focus and save without remounting either controller',async()=>{
  state.tabs.push({...tab('plan'),route:{projectId:'synthetic',view:'plan'}});await open();const a=workspace('a').props.flushRef,plan=workspace('plan').props.flushRef,data=transferData();tabButton('plan').props.onDragStart(dragEvent(data));await settle();await zone('right').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle();
  expect(paneIds()).toEqual(['panel-a','panel-plan']);expect(workspace('plan').props).toMatchObject({compact:true,tabActive:true,keyboardActive:true});expect(workspace('a').props.flushRef).toBe(a);expect(workspace('plan').props.flushRef).toBe(plan);plan.current.mockClear();a.current.mockClear();keyboard.get('keydown')!(keyEvent({key:'s',code:'KeyS',ctrlKey:true}));await settle();expect(plan.current).toHaveBeenCalledOnce();expect(a.current).not.toHaveBeenCalled();
 });
 it('waits for authoritative remote handoff commit before arranging and retains the local draft on source rejection',async()=>{
  state.tabs=[tab('a')];await open();const original=native.invoke.getMockImplementation()!,data=transferData(JSON.stringify({tabId:'remote',sourceWindow:'other'}));
  tree.props.onDragEnter(dragEvent(data));await settle();await zone('right').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle();expect(native.invoke).toHaveBeenCalledWith('workspace_request_tab_move',{tabId:'remote'});expect(paneIds()).toEqual(['panel-a']);
  state.incoming=[{transferId:'handoff',tab:tab('remote'),phase:'awaiting_ready'}];emit('workspace-tabs-changed',{});await settle(()=>Boolean(workspace('remote')));workspace('remote').props.flushRef.current=vi.fn(async()=>{});expect(paneIds()).toEqual(['panel-a']);
  native.invoke.mockImplementation(async(command,args)=>{if(command==='workspace_transfer_ready'){state.tabs.push(tab('remote'));state.incoming=[];return null;}return original(command,args);});workspace('remote').props.onContentReady();await settle(()=>paneIds().length===2);expect(paneIds()).toEqual(['panel-a','panel-remote']);
 });
 it('offers keyboard/context placement alternatives and clears the drag zones on Escape',async()=>{
  await open();const data=transferData();tabButton('b').props.onDragStart(dragEvent(data));await settle();expect(zone('left')).toBeDefined();keyboard.get('keydown')!(keyEvent({key:'Escape'}));await settle();expect(zone('left')).toBeUndefined();
  nodes(tree).find(node=>node.props?.['data-workspace-tab']==='b').props.onContextMenu({preventDefault:vi.fn(),clientX:100,clientY:30});await settle();nodes(tree).find(node=>node.props?.role==='menuitem'&&node.props.children.join?.('')==='Open on left').props.onClick();await settle();expect(nodes(tree).find(node=>node.props?.id==='panel-b').props['data-draft-pane']).toBe('primary');
 });
 it('captures only valid synthetic book folds along with its logical reading anchor',async()=>{
  state.tabs[1]={...tab('b'),route:{projectId:'synthetic',view:'book'}};await open();nodes(tree).find(node=>node.props?.id==='panel-b').props.ref({querySelector:(selector:string)=>selector==='.book-scroll'?{scrollTop:33,dataset:{bookSceneId:'scene-a',bookSceneOffset:'12'}}:selector==='.book-workspace'?{dataset:{bookCharacter:'character',bookFoldedSceneIds:'["scene-a","scene-b"]'}}:null});
  emit('workspace-tab-move-request',{tabId:'b',destinationWindow:'other'});await settle(()=>native.invoke.mock.calls.some(([command])=>command==='workspace_transfer_tab'));expect(native.invoke).toHaveBeenCalledWith('register_tab',expect.objectContaining({tab:expect.objectContaining({ui:{scrollTop:33,bookSceneId:'scene-a',bookSceneOffset:12,bookCharacterId:'character',bookFoldedSceneIds:['scene-a','scene-b']}})}));
 });
});

describe('owned-tab navigation trail coordinator',()=>{
 const bookmark=(hash:string,scrollTop:number)=>({native:{documentHash:hash,cursor:[0,2] as [number,number],anchor:[0,2] as [number,number]},reading:{scrollTop}});
 function ownership(){const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:state.tabs.find(tab=>tab.route.view===args.route.view&&tab.route.sceneId===args.route.sceneId),ownerWindow:state.windowLabel}:original(command,args));}
 it('captures only after drain and restores exact caret/reading data in a measurable active target while user input stays inert',async()=>{
  await open();ownership();const order:string[]=[];let finish!:()=>void;
  workspace('a').props.flushRef.current=async()=>{order.push('drain-a');};workspace('b').props.flushRef.current=async()=>{order.push('drain-b');};
  const a={capture:vi.fn(async()=>{order.push('capture-a');return bookmark('a'.repeat(64),42);}),restore:vi.fn(async value=>{order.push('restore-a');expect(value).toEqual(bookmark('a'.repeat(64),42));await new Promise<void>(resolve=>{finish=resolve;});})};
  const b={capture:vi.fn(async()=>{order.push('capture-b');return bookmark('b'.repeat(64),99);}),restore:vi.fn(async()=>{})};
  workspace('a').props.onRegisterNavigation(a);workspace('b').props.onRegisterNavigation(b);tabButton('b').props.onClick();await settle(()=>tabButton('b').props['aria-selected']);expect(order).toEqual(['drain-a','capture-a']);expect(workspace('b').props.canNavigateBack).toBe(true);
  order.length=0;keyboard.get('keydown')!(keyEvent({key:'ArrowLeft',code:'ArrowLeft',altKey:true}));await settle(()=>Boolean(finish));expect(order).toEqual(['drain-b','capture-b','restore-a']);expect(tree.props.inert).toBe(true);expect(workspace('a').props.tabActive).toBe(true);expect(workspace('a').props.keyboardActive).toBe(false);finish();await settle(()=>workspace('a').props.canNavigateForward);
  expect(tabButton('a').props['aria-selected']).toBe(true);expect(workspace('a').props.canNavigateBack).toBe(false);expect(tree.props.inert).toBe(false);
  keyboard.get('keydown')!(keyEvent({key:'ArrowRight',code:'ArrowRight',altKey:true}));await settle(()=>tabButton('b').props['aria-selected']&&workspace('b').props.canNavigateBack);expect(b.restore).toHaveBeenCalledExactlyOnceWith(bookmark('b'.repeat(64),99));
 });
 it('keeps the source active and trail cursor unchanged when an earlier native bookmark is stale, then permits a successful retry',async()=>{
  await open();ownership();const stale=vi.fn(async():Promise<void>=>{throw new Error('Synthetic document hash changed');});workspace('a').props.onRegisterNavigation({capture:async()=>bookmark('a'.repeat(64),42),restore:stale});workspace('b').props.onRegisterNavigation({capture:async()=>bookmark('b'.repeat(64),99),restore:async()=>{}});tabButton('b').props.onClick();await settle(()=>tabButton('b').props['aria-selected']);const a=workspace('a').props.flushRef;
  workspace('b').props.onNavigateBack();await settle(()=>nodes(tree).some(node=>node.props?.role==='alert'&&String(node.props.children).includes('document hash changed')));expect(tabButton('b').props['aria-selected']).toBe(true);expect(workspace('b').props.canNavigateBack).toBe(true);expect(workspace('b').props.canNavigateForward).toBe(false);expect(workspace('a').props.flushRef).toBe(a);
  stale.mockResolvedValue(undefined);workspace('b').props.onNavigateBack();await settle(()=>tabButton('a').props['aria-selected']&&workspace('a').props.canNavigateForward);expect(stale).toHaveBeenCalledTimes(2);
 });
 it('refuses foreign ownership before restoration and prunes committed closed tabs from history',async()=>{
  await open();ownership();const restore=vi.fn(async()=>{});workspace('a').props.onRegisterNavigation({capture:async()=>bookmark('a'.repeat(64),42),restore});tabButton('b').props.onClick();await settle(()=>tabButton('b').props['aria-selected']);const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:tab('a'),ownerWindow:'other-window'}:original(command,args));
  workspace('b').props.onNavigateBack();await settle(()=>nodes(tree).some(node=>node.props?.role==='alert'&&String(node.props.children).includes('another window')));expect(restore).not.toHaveBeenCalled();expect(tabButton('b').props['aria-selected']).toBe(true);
  nodes(tree).find(node=>node.props?.['aria-label']==='Close a').props.onClick();await settle(()=>!tabButton('a'));expect(workspace('b').props.canNavigateBack).toBe(false);
 });
 it('does not add a departure or focus a target when saving the current draft fails',async()=>{
  await open();const capture=vi.fn(async()=>bookmark('a'.repeat(64),42));workspace('a').props.onRegisterNavigation({capture,restore:async()=>{}});workspace('a').props.flushRef.current=async()=>{throw new Error('Synthetic composition pending');};tabButton('b').props.onClick();await settle(()=>nodes(tree).some(node=>node.props?.role==='alert'));
  expect(capture).not.toHaveBeenCalled();expect(tabButton('a').props['aria-selected']).toBe(true);expect(workspace('a').props.canNavigateBack).toBe(false);
 });
});

describe('binder scene drag to panes',()=>{
 const zone=(side:string)=>nodes(tree).find(node=>node.props?.['data-pane-drop']===side);
 const paneIds=()=>nodes(tree).filter(node=>node.props?.role==='tabpanel'&&!node.props.hidden).map(node=>node.props.id);
 function sceneData(sceneId='c',projectId='synthetic'){return {types:[SCENE_DRAG_TYPE],dropEffect:'none',getData:(type:string)=>type===SCENE_DRAG_TYPE?JSON.stringify({projectId,sceneId,title:'FORGED TITLE'}):''};}
 async function show(data:any){nodes(tree).find(node=>node.props?.className?.startsWith('window-workspace')).props.onDragEnter(dragEvent(data));await settle();}
 it('opens a newly dragged scene on the exact side only after draining and native membership, ignoring the payload title',async()=>{
  await open();autoMountedFlush=true;const data=sceneData(),order:string[]=[];workspace('a').props.flushRef.current=async()=>{order.push('drain');};
  const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>{if(command==='workspace_command')order.push('membership');if(command==='register_tab')order.push('register');return original(command,args);});
  await show(data);await zone('left').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle(()=>paneIds().length===2);
  const created=state.tabs.find(tab=>tab.route.sceneId==='c')!;expect(created.title).toBe('Canonical c');expect(paneIds()).toEqual(['panel-a','panel-'+created.id]);expect(nodes(tree).find(node=>node.props?.id==='panel-'+created.id).props['data-draft-pane']).toBe('primary');expect(order).toEqual(['drain','membership','register','drain']);expect(native.invoke).toHaveBeenCalledWith('workspace_command',{operation:'load_project_scene',args:{projectId:'synthetic',sceneId:'c'}});expect(native.invoke).not.toHaveBeenCalledWith('workspace_request_tab_move',expect.anything());
 });
 it('reuses a canonical local scene tab without registering a duplicate or changing manuscript order',async()=>{
  await open();const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:tab('b'),ownerWindow:'main'}:original(command,args));const data=sceneData('b');await show(data);await zone('right').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle(()=>paneIds().length===2);
  expect(paneIds()).toEqual(['panel-a','panel-b']);expect(nodes(tree).find(node=>node.props?.id==='panel-b').props['data-draft-pane']).toBe('secondary');expect(state.tabs.map(tab=>tab.id)).toEqual(['a','b']);expect(native.invoke).not.toHaveBeenCalledWith('register_tab',expect.anything());expect(native.invoke.mock.calls.some(([command])=>command==='reorder_scene')).toBe(false);
 });
 it('keeps dirty panes on failed drain and refuses wrong-project/deleted native scene replies before registration',async()=>{
  await open();const data=sceneData();workspace('a').props.flushRef.current=async()=>{throw new Error('Pending composition');};await show(data);await zone('left').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle();expect(paneIds()).toEqual(['panel-a']);expect(native.invoke).not.toHaveBeenCalledWith('workspace_command',expect.anything());
  workspace('a').props.flushRef.current=async()=>{};const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_command'?{id:'foreign',scene:{title:'Wrong'}}:original(command,args));await show(data);await zone('left').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle();expect(paneIds()).toEqual(['panel-a']);expect(native.invoke).not.toHaveBeenCalledWith('register_tab',expect.anything());
 });
 it('requests an existing foreign scene owner and waits for transfer instead of duplicating its session',async()=>{
  await open();const original=native.invoke.getMockImplementation()!;native.invoke.mockImplementation(async(command,args)=>command==='workspace_find_tab'?{tab:tab('remote'),ownerWindow:'other'}:original(command,args));const data=sceneData('remote');await show(data);await zone('right').props.onDrop({...dragEvent(data),stopPropagation:vi.fn()});await settle();expect(paneIds()).toEqual(['panel-a']);expect(native.invoke).toHaveBeenCalledWith('workspace_request_tab_move',{tabId:'remote'});expect(native.invoke).not.toHaveBeenCalledWith('register_tab',expect.anything());
 });
});
