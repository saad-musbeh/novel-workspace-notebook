import React, {useCallback, useEffect, useLayoutEffect, useRef, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {DesktopWorkspace} from './DesktopWorkspace';
import {clampSplitRatio, paneSplit, placeTabInPane, readBookFoldedScenes, sameRoute, mergeWindowTabs, readTabDrag, reorderWindowTabs, shouldDetachTab, TAB_DRAG_TYPE, SCENE_DRAG_TYPE, readSceneDrag, tabTitle, type DraftSplit, type PaneSide, type TabDragSession, type IncomingTab, type TabRoute, type WindowTabs, type WorkspaceTab} from './workspace-tabs';
import {NavigationTrail,type NavigationController,type TrailLocation} from './navigation-trail';
import {EDITOR_RESIZE_EVENT} from './editor-resize';
import {dispatchShortcut} from './shortcuts';
import {ShortcutSettings} from './ShortcutSettings';
import {acknowledgeWorkspaceExportSave,type WorkspaceExportSavePayload} from './docx-export';
import './workspace-tabs.css';

type LocalTab = Omit<WorkspaceTab, 'route'> & {route?: TabRoute};
type RegisteredTab = {tab: WorkspaceTab; ownerWindow: string; existing: boolean};
type Props = {flushRef: React.RefObject<(() => Promise<void>) | null>};

export function WorkspaceTabs({flushRef}: Props) {
  const [tabs, setTabs] = useState<LocalTab[]>([]);
  const [active, setActive] = useState('');
  const activeTab=useRef(active);activeTab.current=active;
  const selectTab=useCallback((id:string)=>{activeTab.current=id;setActive(id);},[]);
  const navigation=useRef(new NavigationTrail());
  const navigationControllers=useRef(new Map<string,NavigationController>());
  const navigationCallbacks=useRef(new Map<string,(value:NavigationController|null)=>void>());
  const [,setNavigationVersion]=useState(0);
  const [restoringTab,setRestoringTab]=useState('');
  const restoreMounted=useRef<(()=>void)|null>(null);
  const [split, setSplit] = useState<DraftSplit | null>(null);
  const [splitRatio, setSplitRatio] = useState(50);
  const [paneWidth, setPaneWidth] = useState(960);
  const splitRef = useRef(split); splitRef.current = split;
  const pendingBeside = useRef<{primary: string; tabId: string; seenIncoming: boolean;side?:PaneSide;departure?:TrailLocation} | null>(null);
  const besideTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const host = useRef<HTMLDivElement>(null);
  const resizeStart = useRef<number | null>(null);
  const [windowLabel, setWindowLabel] = useState('');
  const [busy, setBusy] = useState(false);
  const busyRef=useRef(busy);busyRef.current=busy;
  const [error, setError] = useState('');
  const [versions, setVersions] = useState<Record<string, number>>({});
  const controllers = useRef(new Map<string, React.RefObject<(() => Promise<void>) | null>>());
  const nodes = useRef(new Map<string, HTMLDivElement>());
  const incoming = useRef(new Map<string, IncomingTab>());
  const readyTransfers = useRef(new Set<string>());
  const mountedReady = useRef(new Set<string>());
  const registered = useRef(new Set<string>());
  const work = useRef(false);
  const current = useRef(tabs); current.current = tabs;
  const label = useRef(windowLabel); label.current = windowLabel;
  const initialState = useRef(true);
  const pendingBootstrap = useRef<string | null>(null);
  const alive = useRef(true);
  const registerQueues = useRef(new Map<string, Promise<void>>());
  const activateRef = useRef<(id: string) => Promise<void>>(async () => {});
  const pendingFocus=useRef<string|null>(null);
  const refreshVersion=useRef(0);
  const strip = useRef<HTMLDivElement>(null);
  const drag = useRef<TabDragSession | null>(null);
  const [dragging, setDragging] = useState('');
  const [dropBefore, setDropBefore] = useState<string | null>(null);
  const [paneDragging,setPaneDragging]=useState(false);
  const [dropPane,setDropPane]=useState<PaneSide|null>(null);
  const [closeWhenEmpty, setCloseWhenEmpty] = useState(false);
  const [contextMenu, setContextMenu] = useState<{tabId: string; left: number; top: number} | null>(null);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const settingsOpen = useRef(false); settingsOpen.current = shortcutsOpen;
  const menu = useRef<HTMLDivElement>(null);
  const shortcutRef = useRef<(event: KeyboardEvent) => void>(() => {});
  const moveRef = useRef<(id: string, destination: string) => Promise<void>>(async () => {});

  useEffect(() => {
    const measure = () => {const width = host.current?.clientWidth || Math.max(640, window.innerWidth || 960);setPaneWidth(width);setSplitRatio(value => clampSplitRatio(value, width));};
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    if (host.current) observer?.observe(host.current);measure();window.addEventListener('resize', measure);
    return () => {observer?.disconnect();window.removeEventListener('resize', measure);};
  }, []);

  useLayoutEffect(()=>{window.dispatchEvent(new Event(EDITOR_RESIZE_EVENT));},[split?.primary,split?.secondary,splitRatio,paneWidth,active]);

  const navigationCallback=(id:string)=>{let callback=navigationCallbacks.current.get(id);if(!callback){callback=value=>{if(value)navigationControllers.current.set(id,value);else navigationControllers.current.delete(id);};navigationCallbacks.current.set(id,callback);}return callback;};
  const location=(id:string):TrailLocation|undefined=>{const tab=current.current.find(tab=>tab.id===id);if(!tab?.route||!registered.current.has(id)||incoming.current.has(id))return undefined;return {tabId:id,route:{...tab.route}};};
  async function captureLocation(id:string){const value=location(id);if(!value)return undefined;const adapter=navigationControllers.current.get(id);if(adapter)value.bookmark=await adapter.capture();return value;}
  function visitLocation(id:string,departure?:TrailLocation){const next=location(id);if(!next)return;const previous=navigation.current.current;if(departure&&previous&&(previous.tabId!==departure.tabId||!sameRoute(previous.route,departure.route)))navigation.current.visit(departure);navigation.current.visit(next,departure);setNavigationVersion(value=>value+1);}
  function forgetLocation(id:string){navigation.current.removeTab(id);navigationControllers.current.delete(id);navigationCallbacks.current.delete(id);setNavigationVersion(value=>value+1);}
  useEffect(()=>{if(!navigation.current.current&&location(active))visitLocation(active);},[active,tabs]);
  useLayoutEffect(()=>{if(restoringTab&&active===restoringTab){const resolve=restoreMounted.current;restoreMounted.current=null;resolve?.();}},[restoringTab,active]);
  async function navigateTrail(direction:'back'|'forward'){
    if(work.current||!navigation.current.begin(direction))return;
    const previousActive=active,previousSplit=splitRef.current;let mountTimer:ReturnType<typeof setTimeout>|null=null;work.current=true;setBusy(true);setError('');
    try{
      if(previousActive)await drain(previousActive);
      const departure=previousActive?await captureLocation(previousActive):undefined;
      if(departure)navigation.current.updateCurrent(departure);
      const ticket=navigation.current.begin(direction);if(!ticket)return;
      const target=location(ticket.target.tabId);
      if(!target||!sameRoute(target.route,ticket.target.route)){navigation.current.prune(item=>Boolean(location(item.tabId)&&sameRoute(location(item.tabId)!.route,item.route)));setNavigationVersion(value=>value+1);throw new Error('This earlier place is no longer owned by this window.');}
      const owned=await invoke<{tab:WorkspaceTab;ownerWindow:string}|null>('workspace_find_tab',{route:target.route});
      if(!owned||owned.ownerWindow!==label.current||owned.tab.id!==target.tabId)throw new Error('This earlier tab moved to another window. Its position was kept there.');
      const mounted=new Promise<void>((resolve,reject)=>{restoreMounted.current=()=>{if(mountTimer!==null)clearTimeout(mountTimer);mountTimer=null;resolve();};mountTimer=setTimeout(()=>{restoreMounted.current=null;reject(new Error('This earlier tab did not become ready to restore.'));},5000);});
      displayTab(target.tabId,target.route);setRestoringTab(target.tabId);await mounted;
      if(ticket.target.bookmark){const adapter=navigationControllers.current.get(target.tabId);if(!adapter)throw new Error('This earlier place is still opening. Try again.');await adapter.restore(ticket.target.bookmark);}
      const stillOwned=await invoke<{tab:WorkspaceTab;ownerWindow:string}|null>('workspace_find_tab',{route:target.route});
      if(!stillOwned||stillOwned.ownerWindow!==label.current||stillOwned.tab.id!==target.tabId||!location(target.tabId)||!navigation.current.commit(ticket))throw new Error('This tab changed ownership while restoring its place.');
      setNavigationVersion(value=>value+1);
    }catch(reason){if(location(previousActive)){setSplit(previousSplit&&paneSplit(current.current,previousSplit.primary,previousSplit.secondary)?previousSplit:null);selectTab(previousActive);}setError('Could not return to this place. '+String(reason));}
    finally{if(mountTimer!==null)clearTimeout(mountTimer);restoreMounted.current=null;setRestoringTab('');work.current=false;setBusy(false);}
  }
  function clearBesideIntent() {
    pendingBeside.current = null;
    if (besideTimeout.current !== null) clearTimeout(besideTimeout.current);
    besideTimeout.current = null;
  }
  useEffect(() => () => {if (besideTimeout.current !== null) clearTimeout(besideTimeout.current);}, []);

  const controller = (id: string) => {
    let ref = controllers.current.get(id);
    if (!ref) {ref = {current: null}; controllers.current.set(id, ref);}
    return ref;
  };
  const drain = useCallback(async (id?: string) => {
    const targets = id ? current.current.filter(tab => tab.id === id) : current.current;
    for (const tab of targets) {
      const save = controllers.current.get(tab.id)?.current;
      if (!save) throw new Error('This tab is still opening. Try again in a moment.');
      await save();
      await registerQueues.current.get(tab.id);
    }
  }, []);
  useEffect(() => {flushRef.current = drain; return () => {flushRef.current = null;};}, [drain, flushRef]);

  const refresh = useCallback(async () => {
    const version=++refreshVersion.current;
    const state = await invoke<WindowTabs>('workspace_window_state');
    if (!alive.current||version!==refreshVersion.current) return;
    setWindowLabel(state.windowLabel);
    incoming.current = new Map(state.incoming.map(item => [item.tab.id, item]));
    for (const tab of state.tabs) registered.current.add(tab.id);
    setTabs(previous => {
      if(!alive.current||version!==refreshVersion.current)return previous;
      const known = mergeWindowTabs(previous.filter(tab => tab.route) as WorkspaceTab[], state);
      const provisional = previous.filter(tab => tab.id === pendingBootstrap.current && !registered.current.has(tab.id));
      if (initialState.current && state.windowLabel === 'main' && !known.length && !state.incoming.length) {
        const id = crypto.randomUUID(); pendingBootstrap.current = id;
        provisional.push({id, title: 'Write'});
      }
      initialState.current = false;
      const next = [...known, ...provisional];
      setActive(selected => version!==refreshVersion.current?selected:next.some(tab => tab.id === selected) ? selected : next[0]?.id ?? '');
      return next;
    });
  }, []);

  useEffect(() => {
    if (split && !paneSplit(tabs, split.primary, split.secondary)) {
      const remaining = tabs.find(tab => tab.id === split.primary || tab.id === split.secondary);
      setSplit(null); if (remaining) selectTab(remaining.id);
    }
    const pending = pendingBeside.current;
    if (pending && incoming.current.has(pending.tabId)) pending.seenIncoming = true;
    if (pending?.seenIncoming && !incoming.current.has(pending.tabId) && !tabs.some(tab => tab.id === pending.tabId)) {clearBesideIntent();setError('The draft move was cancelled. Your current draft remains open.');return;}
    if (pending && !work.current && tabs.some(tab => tab.id === pending.tabId) && !incoming.current.has(pending.tabId)) {
      clearBesideIntent();
      const pair = pending.side==='left'?paneSplit(tabs,pending.tabId,pending.primary):paneSplit(tabs, pending.primary, pending.tabId);
      if (pair) {void openPair(pair.primary,pair.secondary,pending.tabId,pending.departure);}
      else setError('Choose two different draft tabs to open beside each other.');
    }
  }, [tabs, split,busy]);

  function displayTab(id: string, route?: TabRoute) {
    const pair = splitRef.current;
    if (pair && id !== pair.primary && id !== pair.secondary) {
      setSplit(paneSplit(current.current, active === pair.secondary ? pair.primary : id, active === pair.secondary ? id : pair.secondary));
    }
    selectTab(id);
  }

  const contentReady = useCallback(async (id: string) => {
    mountedReady.current.add(id);
    const transfer = incoming.current.get(id);
    if (!transfer || transfer.phase !== 'awaiting_ready' || readyTransfers.current.has(transfer.transferId)) return;
    readyTransfers.current.add(transfer.transferId);
    try {
      // Native session snapshots retain the logical passage and reflow it at
      // the destination width. Raw source CSS pixels cannot restore that anchor.
      await invoke('workspace_transfer_ready', {transferId: transfer.transferId});
      selectTab(id); await refresh();
    } catch (reason) {
      setError(String(reason));
      await invoke('workspace_transfer_abort', {transferId: transfer.transferId}).catch(() => {});
      await refresh();
    }
  }, [refresh]);

  const contentFailed=useCallback(async(id:string,message:string)=>{
    const transfer=incoming.current.get(id);if(!transfer)return;
    setError(message);if (pendingBeside.current?.tabId === id) clearBesideIntent();
    await invoke('workspace_transfer_abort',{transferId:transfer.transferId}).catch(reason=>setError(String(reason)));
    await refresh();
  },[refresh]);

  useEffect(() => {
    alive.current = true;
    let disposed = false;
    let eventTarget='';
    const unsubscribe: (() => void)[] = [];
    const add = async <T,>(event: string, handle: (payload: T) => void) => {
      const off = await listen<T>(event, event => handle(event.payload), {target:eventTarget});
      if (disposed) off(); else unsubscribe.push(off);
    };
    void (async () => {
      eventTarget=(await invoke<WindowTabs>('workspace_window_state')).windowLabel;
      await add('workspace-tabs-changed', () => {void refresh().catch(reason => setError(String(reason)));});
      await add<{tabId: string}>('workspace-focus-tab', ({tabId}) => {if(work.current)pendingFocus.current=tabId;else void activateRef.current(tabId);});
      await add<{tabId: string; destinationWindow: string}>('workspace-tab-move-request', ({tabId, destinationWindow}) => {
        if (!registered.current.has(tabId) || incoming.current.has(tabId) || !current.current.some(tab => tab.id === tabId && tab.route) || destinationWindow === label.current) return;
        void moveRef.current(tabId, `window:${destinationWindow}`);
      });
      await add<{tabId: string}>('workspace-tab-transferred', ({tabId}) => {
        registered.current.delete(tabId); controllers.current.delete(tabId); mountedReady.current.delete(tabId);forgetLocation(tabId);
        setTabs(previous => previous.filter(tab => tab.id !== tabId));
        work.current = false; setBusy(false);
        void refresh().then(() => setCloseWhenEmpty(true)).catch(reason => setError(String(reason)));
      });
      await add<{error: string}>('workspace-transfer-failed', ({error}) => {
        work.current = false; setBusy(false); setError(error);clearBesideIntent();
        void refresh().catch(reason => setError(String(reason)));
      });
      await add<WorkspaceExportSavePayload>('workspace-export-save', payload => {
        const projectTabs=current.current.filter(tab=>tab.route?.projectId===payload.projectId);
        void acknowledgeWorkspaceExportSave(payload,projectTabs,{busy:busyRef.current||work.current,isIncoming:tabId=>incoming.current.has(tabId),drain:async tabId=>{await drain(tabId);},acknowledge:result=>invoke('workspace_export_save_ready',{requestId:result.requestId,error:result.error})}).catch(reason=>setError(String(reason)));
      });
      await add<{projectId: string; tabId: string}>('workspace-content-changed', change => {
        window.dispatchEvent(new CustomEvent('workspace-universe-changed',{detail:{projectId:change.projectId}}));
        setVersions(previous => {
          const next = {...previous};
          for (const tab of current.current) if (tab.id !== change.tabId && tab.route?.projectId === change.projectId) next[tab.id] = (next[tab.id] ?? 0) + 1;
          return next;
        });
      });
      await refresh();
    })().catch(reason => setError(String(reason)));
    const savedData=(event:Event)=>{
      const {projectId,kind}=(event as CustomEvent<{projectId:string;kind:string}>).detail;
      const view=kind==='sceneMetadata'?'library':['assistanceJobs','languagePacks','settings'].includes(kind)?'review':null;
      if(!view)return;
      const tab=current.current.find(tab=>tab.route?.projectId===projectId&&tab.route.view===view&&registered.current.has(tab.id));
      if(tab)void invoke('workspace_notify_change',{projectId,tabId:tab.id}).catch(reason=>setError(String(reason)));
    };
    const cancelDrag = (event: KeyboardEvent) => {if (settingsOpen.current) return;if (event.key === 'Escape') {if (drag.current) {drag.current.cancelled = true; setDragging(''); setDropBefore(null);}setPaneDragging(false);setDropPane(null);setContextMenu(null);} else shortcutRef.current(event);};
    const dismissMenu = (event: PointerEvent) => {if (!menu.current?.contains(event.target as Node)) setContextMenu(null);};
    window.addEventListener('pointerdown',dismissMenu);
    window.addEventListener('keydown',cancelDrag,true);
    window.addEventListener('workspace-data-saved',savedData);
    return () => {disposed = true; alive.current = false; unsubscribe.forEach(off => off());window.removeEventListener('workspace-data-saved',savedData);window.removeEventListener('keydown',cancelDrag,true);window.removeEventListener('pointerdown',dismissMenu);drag.current=null;};
  }, [refresh]);

  useEffect(() => {
    if (!closeWhenEmpty) return;
    setCloseWhenEmpty(false);
    if (tabs.length || current.current.length || pendingBootstrap.current || incoming.current.size || busy || work.current || !windowLabel) return;
    work.current = true; setBusy(true);
    void invoke('workspace_close_empty_window').catch(reason => {work.current = false; setBusy(false); setError(String(reason));});
  }, [closeWhenEmpty, tabs, busy, windowLabel]);

  async function openRoute(route: TabRoute, title: string) {
    if (work.current) return;
    work.current = true; setBusy(true); setError('');
    try {
      for(const id of new Set([active,splitRef.current?.primary,splitRef.current?.secondary].filter((id):id is string=>Boolean(id))))await drain(id);
      const departure=active?await captureLocation(active):undefined;
      const existing = await invoke<{tab: WorkspaceTab;ownerWindow:string} | null>('workspace_find_tab', {route});
      if (existing) {await invoke('workspace_focus_tab', {tabId: existing.tab.id});if(existing.ownerWindow===label.current){displayTab(existing.tab.id, existing.tab.route);visitLocation(existing.tab.id,departure);} return;}
      const tab: WorkspaceTab = {id: crypto.randomUUID(), route, title: tabTitle(title)};
      const result = await invoke<RegisteredTab>('register_tab', {tab});
      if (result.ownerWindow !== label.current || result.existing) {
        await invoke('workspace_focus_tab', {tabId: result.tab.id}); return;
      }
      ++refreshVersion.current;
      registered.current.add(tab.id);
      current.current=current.current.some(item=>item.id===result.tab.id)?current.current:[...current.current,result.tab];
      setTabs(previous => previous.some(item=>item.id===result.tab.id)?previous:[...previous, result.tab]); setSplit(null); selectTab(tab.id);visitLocation(tab.id,departure);
      void refresh().catch(reason=>setError(String(reason)));
    } catch (reason) {setError(String(reason));}
    finally {work.current = false; setBusy(false);}
  }

  const routeReady = useCallback((id: string, route: TabRoute, title: string) => {
    if (incoming.current.has(id)) return;
    const previous = registerQueues.current.get(id) ?? Promise.resolve();
    const next = previous.catch(() => {}).then(async () => {
      const ui=current.current.find(tab=>tab.id===id)?.ui;
      const result = await invoke<RegisteredTab>('register_tab', {tab: {id, route, title: tabTitle(title),...(ui?{ui}:{})}});
      if (!alive.current) return;
      ++refreshVersion.current;
      if (result.tab.id !== id) {
        pendingBootstrap.current = null;
        if(registered.current.has(id)){await invoke('unregister_tab',{tabId:id});registered.current.delete(id);}
        else await invoke('editor_release', {sessionId: id});
        forgetLocation(id);setTabs(tabs => tabs.filter(tab => tab.id !== id));
        if(activeTab.current===id)await invoke('workspace_focus_tab', {tabId: result.tab.id});
        void refresh().catch(reason=>setError(String(reason)));
        return;
      }
      registered.current.add(id); pendingBootstrap.current = null;
      // Drain waits for this queue; expose its acknowledged binding immediately.
      current.current = current.current.map(tab => tab.id === id ? result.tab : tab);
      setTabs(tabs => tabs.map(tab => tab.id === id ? result.tab : tab));
      void refresh().catch(reason=>setError(String(reason)));
    });
    registerQueues.current.set(id, next);
    void next.catch(reason => setError(String(reason)));
  }, [refresh]);

  async function closeTab(id: string) {
    if (work.current) return;
    work.current = true; setBusy(true); setError('');
    try {
      await drain(id);
      if (registered.current.has(id)) await invoke('unregister_tab', {tabId: id});
      registered.current.delete(id); controllers.current.delete(id); mountedReady.current.delete(id);forgetLocation(id);
      if (pendingBootstrap.current === id) pendingBootstrap.current = null;
      setTabs(previous => previous.filter(tab => tab.id !== id)); await refresh();
    } catch (reason) {setError('Could not close this tab. ' + String(reason));}
    finally {work.current = false; setBusy(false);}
  }

  async function activateTab(id: string) {
    if (id === active || work.current || !current.current.some(tab => tab.id === id)) return;
    const pair = splitRef.current;
    work.current = true; setBusy(true); setError('');
    try {if (active) await drain(active);const departure=active?await captureLocation(active):undefined;if(!current.current.some(tab=>tab.id===id)||!registered.current.has(id)||incoming.current.has(id))throw new Error('This tab changed ownership while saving.');displayTab(id, current.current.find(tab => tab.id === id)?.route);visitLocation(id,departure);}
    catch (reason) {setError('Could not leave this tab. ' + String(reason));}
    finally {work.current = false; setBusy(false);}
  }
  activateRef.current = activateTab;
  useEffect(()=>{
    if(busy||!pendingFocus.current)return;
    const id=pendingFocus.current;pendingFocus.current=null;void activateRef.current(id);
  },[busy,active,tabs]);

  async function saveTab(id: string) {
    if (!id || work.current) return;
    work.current=true;setBusy(true);setError('');
    try {await drain(id);}
    catch(reason){setError('Could not save this tab. '+String(reason));}
    finally{work.current=false;setBusy(false);}
  }

  async function moveTab(id: string, destination: string) {
    const tab = current.current.find(tab => tab.id === id);
    if (!tab?.route || !registered.current.has(tab.id) || incoming.current.has(tab.id) || destination === label.current || work.current) return;
    work.current = true; setBusy(true); setError('');
    try {
      await drain(tab.id);
      const acknowledged = current.current.find(item => item.id === id);
      if (!acknowledged?.route || !registered.current.has(id) || incoming.current.has(id)) throw new Error('This tab changed ownership while saving. Try again.');
      const book=nodes.current.get(id)?.querySelector<HTMLElement>('.book-scroll');
      const scrollTop=book?.scrollTop??nodes.current.get(id)?.querySelector<HTMLElement>('.native-scroll')?.scrollTop??0;
      const bookWorkspace=nodes.current.get(id)?.querySelector<HTMLElement>('.book-workspace');
      const bookCharacterId=bookWorkspace?.dataset.bookCharacter;
      const bookFoldedSceneIds=readBookFoldedScenes(bookWorkspace?.dataset.bookFoldedSceneIds);
      const bookUi=book?{...(bookFoldedSceneIds?{bookFoldedSceneIds}:{}),...(book.dataset.bookSceneId?{bookSceneId:book.dataset.bookSceneId,bookSceneOffset:Number(book.dataset.bookSceneOffset)||0}:{}),...(bookCharacterId?{bookCharacterId}:{})}:{};
      const binding = await invoke<RegisteredTab>('register_tab', {tab: {...acknowledged, ui: {scrollTop,...bookUi}}});
      if (binding.tab.id !== id || binding.ownerWindow !== label.current) throw new Error('This tab changed ownership while saving. Try again.');
      await invoke('workspace_transfer_tab', {tabId: id, destination});
      // Source stays mounted and paused until destination acknowledges readiness.
    } catch (reason) {setError('Could not move this tab. ' + String(reason)); work.current = false; setBusy(false);}
  }

  moveRef.current = moveTab;

  function startDrag(event: React.DragEvent, id: string) {
    if (work.current || !registered.current.has(id) || incoming.current.has(id) || !current.current.find(tab => tab.id === id)?.route) {event.preventDefault(); return;}
    drag.current = {tabId: id, sourceWindow: label.current, startX: event.clientX, startY: event.clientY, cancelled: false, dropped: false};
    event.dataTransfer.setData(TAB_DRAG_TYPE, JSON.stringify({tabId: id, sourceWindow: label.current}));
    event.dataTransfer.effectAllowed = 'move'; setDragging(id);setPaneDragging(true);setContextMenu(null);
  }
  function finishDrag(event: React.DragEvent) {
    const session = drag.current; drag.current = null; setDragging(''); setDropBefore(null);setPaneDragging(false);setDropPane(null);
    if (session && strip.current && shouldDetachTab(session, event.clientX, event.clientY, strip.current.getBoundingClientRect(), event.dataTransfer.dropEffect)) void moveTab(session.tabId, 'new');
  }
  function hoverDrag(event: React.DragEvent) {
    if (work.current || drag.current?.cancelled || !Array.from(event.dataTransfer.types).includes(TAB_DRAG_TYPE)) return;
    event.preventDefault(); event.dataTransfer.dropEffect = 'move';
    const item = (event.target as HTMLElement).closest<HTMLElement>('[data-workspace-tab]');
    if (!item) {setDropBefore(''); return;}
    const rect = item.getBoundingClientRect();
    const index = current.current.findIndex(tab => tab.id === item.dataset.workspaceTab);
    setDropBefore(event.clientX < rect.left + rect.width / 2 ? item.dataset.workspaceTab! : current.current[index + 1]?.id ?? '');
  }
  async function dropTab(event: React.DragEvent) {
    if (work.current || drag.current?.cancelled) return;
    const payload = readTabDrag(event.dataTransfer.getData(TAB_DRAG_TYPE));
    if (!payload) return;
    event.preventDefault(); event.dataTransfer.dropEffect = 'move';
    if (drag.current) drag.current.dropped = true;
    const before = dropBefore || undefined; setDropBefore(null);setPaneDragging(false);setDropPane(null);
    if (payload.sourceWindow === label.current) {
      if (!registered.current.has(payload.tabId) || incoming.current.has(payload.tabId)) return;
      setTabs(previous => reorderWindowTabs(previous, payload.tabId, before));
    } else {
      // Native authority locates the actual owner; the payload grants no ownership.
      try {await invoke('workspace_request_tab_move', {tabId: payload.tabId});}
      catch (reason) {setError('Could not move this tab. ' + String(reason));}
    }
  }

  function hoverPane(event:React.DragEvent,side:PaneSide){
    if(work.current||drag.current?.cancelled||!Array.from(event.dataTransfer.types).some(type=>type===TAB_DRAG_TYPE||type===SCENE_DRAG_TYPE))return;
    event.preventDefault();event.stopPropagation();event.dataTransfer.dropEffect=Array.from(event.dataTransfer.types).includes(SCENE_DRAG_TYPE)?'copy':'move';setDropBefore(null);setPaneDragging(true);setDropPane(side);
  }
  async function dropInPane(event:React.DragEvent,side:PaneSide){
    if(work.current||drag.current?.cancelled)return;
    const scene=readSceneDrag(event.dataTransfer.getData(SCENE_DRAG_TYPE));
    if(scene){event.preventDefault();event.stopPropagation();event.dataTransfer.dropEffect='copy';setDropBefore(null);setPaneDragging(false);setDropPane(null);await openInPaneRoute({projectId:scene.projectId,sceneId:scene.sceneId,view:'write'},scene.title,side);return;}
    const payload=readTabDrag(event.dataTransfer.getData(TAB_DRAG_TYPE));if(!payload)return;
    event.preventDefault();event.stopPropagation();event.dataTransfer.dropEffect='move';
    if(drag.current)drag.current.dropped=true;
    setDropBefore(null);setPaneDragging(false);setDropPane(null);
    if(payload.sourceWindow===label.current){
      if(!registered.current.has(payload.tabId)||incoming.current.has(payload.tabId))return;
      clearBesideIntent();const pair=placeTabInPane(current.current,splitRef.current,active,payload.tabId,side);
      if(!pair){setError('Open a second distinct tab before arranging panes.');return;}
      await openPair(pair.primary,pair.secondary,payload.tabId);return;
    }
    const counterpart=splitRef.current?(side==='left'?splitRef.current.secondary:splitRef.current.primary):active;
    if(!counterpart||!registered.current.has(counterpart)||incoming.current.has(counterpart)){setError('Open a local tab before bringing another tab beside it.');return;}
    work.current=true;setBusy(true);setError('');clearBesideIntent();
    try{
      for(const id of new Set([active,splitRef.current?.primary,splitRef.current?.secondary].filter((id):id is string=>Boolean(id))))await drain(id);
      const departure=active?await captureLocation(active):undefined;
      const intent={primary:counterpart,tabId:payload.tabId,seenIncoming:false,side,departure};pendingBeside.current=intent;
      besideTimeout.current=setTimeout(()=>{if(pendingBeside.current!==intent)return;clearBesideIntent();setError('The other window did not complete the tab move. Your current panes remain open.');},40000);
      await invoke('workspace_request_tab_move',{tabId:payload.tabId});
    }catch(reason){clearBesideIntent();setError('Could not arrange this tab. '+String(reason));}
    finally{work.current=false;setBusy(false);}
  }
  function freshWorkspace() {
    if (work.current || pendingBootstrap.current) return;
    const id = crypto.randomUUID(); pendingBootstrap.current = id;
    setTabs(previous => [...previous, {id, title: 'Write'}]); setSplit(null); selectTab(id);
  }
  async function openPair(primary: string, secondary: string,focus=secondary,capturedDeparture?:TrailLocation) {
    const pair = paneSplit(current.current, primary, secondary);
    if (!pair || work.current) return;
    work.current = true; setBusy(true); setError('');
    try {
      for (const id of new Set([active,splitRef.current?.primary,splitRef.current?.secondary, primary, secondary].filter((id):id is string=>Boolean(id)))) await drain(id);
      const acknowledged = paneSplit(current.current, primary, secondary);
      if (!acknowledged) throw new Error('A selected draft changed while saving.');
      if([primary,secondary].some(id=>!registered.current.has(id)||incoming.current.has(id)))throw new Error('A selected tab changed ownership while saving.');
      const departure=capturedDeparture??(active?await captureLocation(active):undefined);
      setSplit(acknowledged); selectTab(focus);visitLocation(focus,departure);
    } catch (reason) {setError('Could not arrange these tabs. ' + String(reason));}
    finally {work.current = false; setBusy(false);}
  }
  async function singleView() {
    const pair = splitRef.current; if (!pair || work.current) return;
    work.current = true; setBusy(true); setError('');
    try {await drain(active === pair.secondary ? pair.primary : pair.secondary); setSplit(null);}
    catch (reason) {setError('Could not leave split view. ' + String(reason));}
    finally {work.current = false; setBusy(false);}
  }
  function toggleSplit() {
    if(work.current)return;
    if(splitRef.current){void singleView();return;}
    const currentTab=current.current.find(tab=>tab.id===active);
    const other=current.current.find(tab=>tab.id!==active&&tab.route?.view==='write');
    if(currentTab?.route?.view!=='write'||!other){setError('Open another scene tab before splitting the writing area.');return;}
    void openPair(active,other.id);
  }
  async function openBesideRoute(route:TabRoute,title:string){await openInPaneRoute(route,title,'right');}
  async function openInPaneRoute(route:TabRoute,title:string,side:PaneSide){
    if(route.view!=='write'||!route.sceneId||work.current)return;
    const candidate=splitRef.current?(side==='left'?splitRef.current.secondary:splitRef.current.primary):active;
    const primary=current.current.find(tab=>tab.id===candidate&&tab.route&&!sameRoute(tab.route,route))??current.current.find(tab=>tab.route&&!sameRoute(tab.route,route)&&registered.current.has(tab.id)&&!incoming.current.has(tab.id));
    if(!primary?.route){setError('Choose a different scene or tool to open beside this draft.');return;}
    work.current=true;setBusy(true);setError('');
    try{
      for(const id of new Set([active,splitRef.current?.primary,splitRef.current?.secondary,primary.id].filter((id):id is string=>Boolean(id))))await drain(id);
      const departure=active?await captureLocation(active):undefined;
      const scene=await invoke<{id:string;scene:{title:string}}>('workspace_command',{operation:'load_project_scene',args:{projectId:route.projectId,sceneId:route.sceneId}});
      if(scene.id!==route.sceneId||typeof scene.scene?.title!=='string')throw new Error('This scene is no longer in the selected project.');
      let found=await invoke<RegisteredTab|null>('workspace_find_tab',{route});
      if(!found){const tab={id:crypto.randomUUID(),route,title:tabTitle(scene.scene.title)};found=await invoke<RegisteredTab>('register_tab',{tab});
        if(found.ownerWindow===label.current){registered.current.add(found.tab.id);setTabs(previous=>previous.some(item=>item.id===found!.tab.id)?previous:[...previous,found!.tab]);}
      }
      if(!sameRoute(found.tab.route,route))throw new Error('This tab no longer identifies the selected scene.');
      if(!registered.current.has(primary.id)||incoming.current.has(primary.id))throw new Error('The other pane changed ownership while saving.');
      clearBesideIntent();
      const intent={primary:primary.id,tabId:found.tab.id,seenIncoming:false,side,departure};pendingBeside.current=intent;
      besideTimeout.current=setTimeout(()=>{if(pendingBeside.current!==intent)return;clearBesideIntent();setError('The other window did not complete the draft move. Your current panes remain open.');},40000);
      if(found.ownerWindow!==label.current)await invoke('workspace_request_tab_move',{tabId:found.tab.id});else await refresh();
    }catch(reason){clearBesideIntent();setError('Could not open this scene in a pane. '+String(reason));}
    finally{work.current=false;setBusy(false);}
  }
  function openBesideTab(id: string) {
    const primary = current.current.find(tab => tab.id === active && tab.id !== id && tab.route?.view === 'write') ?? current.current.find(tab => tab.id !== id && tab.route?.view === 'write');
    if (primary) void openPair(primary.id, id);
  }
  function resizeSplit(clientX: number, clientY: number) {
    const rect = host.current?.getBoundingClientRect(); if (!rect) return;
    if (paneWidth < 640) {const height = rect.height - 44;setSplitRatio(clampSplitRatio((clientY - rect.top - 44) / height * 100, height));}
    else setSplitRatio(clampSplitRatio((clientX - rect.left) / rect.width * 100, rect.width));
  }
  shortcutRef.current = event => {
    if (work.current || settingsOpen.current) return;
    const cycle = (direction: number) => {
      const available = current.current;
      if (!available.length) return;
      const index = available.findIndex(tab => tab.id === active);
      const next = available[(index + direction + available.length) % available.length];
      if (next) void activateTab(next.id);
    };
    dispatchShortcut(event, 'workspace', {
      'workspace.navigateBack':()=>{void navigateTrail('back');},
      'workspace.navigateForward':()=>{void navigateTrail('forward');},
      'workspace.split': toggleSplit,
      'workspace.save': () => {void saveTab(active);},
      'workspace.newTab': freshWorkspace,
      'workspace.closeTab': () => {void closeTab(active);},
      'workspace.nextTab': () => cycle(1),
      'workspace.previousTab': () => cycle(-1),
      'workspace.detachTab': () => {void moveTab(active, 'new');},
      'workspace.returnTab': () => {if (label.current !== 'main') void moveTab(active, 'main');},
      'workspace.quit': () => {void invoke('workspace_request_quit').catch(reason => setError(String(reason)));},
      'workspace.shortcuts': () => {setContextMenu(null); setShortcutsOpen(true);},
      'workspace.fullscreen': () => {void invoke('workspace_toggle_fullscreen').catch(reason => setError(String(reason)));},
    });
  };
  function showContext(id: string, x: number, y: number) {
    if (work.current) return;
    setContextMenu({tabId: id, left: Math.max(4, Math.min(x, window.innerWidth - 230)), top: Math.max(4, Math.min(y, window.innerHeight - 260))});
  }
  useEffect(() => {if (contextMenu) menu.current?.querySelector<HTMLButtonElement>('[role=menuitem]:not(:disabled)')?.focus();}, [contextMenu]);
  const selected = tabs.find(tab => tab.id === contextMenu?.tabId);
  const writingTabs = tabs.filter(tab => tab.route?.view === 'write');
  const paneTabs=tabs.filter(tab=>tab.route&&registered.current.has(tab.id)&&!incoming.current.has(tab.id));
  const splitCandidate = writingTabs.find(tab => tab.id !== active);
  const hostWidth = paneWidth;
  const minimumRatio = clampSplitRatio(0, hostWidth), maximumRatio = clampSplitRatio(100, hostWidth);
  return <div ref={host} onDragEnd={()=>{setPaneDragging(false);setDropPane(null);}} onDragEnter={event=>{if(!work.current&&Array.from(event.dataTransfer.types).some(type=>type===TAB_DRAG_TYPE||type===SCENE_DRAG_TYPE))setPaneDragging(true);}} onDragLeave={event=>{const rect=event.currentTarget.getBoundingClientRect();if(event.clientX<=rect.left||event.clientX>=rect.right||event.clientY<=rect.top||event.clientY>=rect.bottom){setPaneDragging(false);setDropPane(null);}}} className={"window-workspace" + (tabs.length===1 ? " single-tab-workspace" : "") + (split ? " workspace-split" : "")} inert={busy} style={{'--split-ratio': splitRatio + '%', '--split-fraction': splitRatio / 100} as React.CSSProperties}>
    <header className="window-tabs" inert={shortcutsOpen}>
      <div role="tablist" aria-label="Workspace tabs" className="tab-list" ref={strip} onDragOver={hoverDrag} onDrop={event => void dropTab(event)} onDragLeave={event => {if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDropBefore(null);}} onKeyDown={event => {
        if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
        event.preventDefault();
        const index = tabs.findIndex(tab => tab.id === active);
        const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
        if (tabs[next]) {const id=tabs[next].id;void activateTab(id).then(()=>document.querySelector<HTMLButtonElement>(`[data-tab-id="${id}"]`)?.focus());}
      }}>
        {tabs.map(tab => <div className={"tab-item" + (dragging === tab.id ? " tab-dragging" : "") + (dropBefore === tab.id ? " tab-drop-before" : "")} data-workspace-tab={tab.id} key={tab.id} onContextMenu={event => {event.preventDefault(); showContext(tab.id, event.clientX, event.clientY);}}>
          <button role="tab" id={'tab-' + tab.id} data-tab-id={tab.id} aria-controls={'panel-' + tab.id} aria-selected={active === tab.id} draggable={Boolean(tab.route) && registered.current.has(tab.id) && !incoming.current.has(tab.id)} onDragStart={event => startDrag(event, tab.id)} onDragEnd={finishDrag} onKeyDown={event => {if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {event.preventDefault(); event.stopPropagation(); const rect = event.currentTarget.getBoundingClientRect(); showContext(tab.id, rect.left, rect.bottom);}}} tabIndex={active === tab.id ? 0 : -1} onClick={() => void activateTab(tab.id)} dir="auto">{tab.title || 'Untitled scene'}</button>
          <button aria-label={'Close ' + (tab.title || 'Untitled scene')} onClick={() => void closeTab(tab.id)}>×</button>
        </div>)}
        {dropBefore === '' && <span className="tab-drop-end" aria-hidden="true"/>}
      </div>
      <button className="split-toggle" disabled={!split && (!splitCandidate || !tabs.find(tab => tab.id === active)?.route || tabs.find(tab => tab.id === active)?.route?.view !== 'write')} onClick={toggleSplit}>{split ? 'Single view' : 'Split'}</button>
      {split && <select className="beside-selector" aria-label="Tab beside" value={split.secondary} onChange={event => void openPair(split.primary, event.target.value)}>{paneTabs.filter(tab => tab.id !== split.primary).map(tab => <option key={tab.id} value={tab.id}>{tab.title}</option>)}</select>}
    </header>
    {paneDragging&&!busy&&!shortcutsOpen&&<div className="pane-drop-targets" aria-label="Arrange dragged tab"><div className={"pane-drop-target"+(dropPane==='left'?" pane-drop-hover":"")} data-pane-drop="left" onDragOver={event=>hoverPane(event,'left')} onDrop={event=>void dropInPane(event,'left')}><span>{paneWidth<640?'Open above':'Open on left'}</span><small>Keep the other tab beside it</small></div><div className={"pane-drop-target"+(dropPane==='right'?" pane-drop-hover":"")} data-pane-drop="right" onDragOver={event=>hoverPane(event,'right')} onDrop={event=>void dropInPane(event,'right')}><span>{paneWidth<640?'Open below':'Open on right'}</span><small>Drafts and project tools</small></div></div>}
    {contextMenu && <div ref={menu} role="menu" aria-label="Tab actions" className="tab-context-menu" style={{left: contextMenu.left, top: contextMenu.top}} onKeyDown={event => {
      if (event.key === 'Escape') {event.preventDefault(); setContextMenu(null); document.querySelector<HTMLButtonElement>(`[data-tab-id="${contextMenu.tabId}"]`)?.focus(); return;}
      if (!['ArrowDown','ArrowUp','Home','End'].includes(event.key)) return;
      event.preventDefault(); const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role=menuitem]:not(:disabled)'));
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    }}>
      <button role="menuitem" disabled={!selected} onClick={() => {setContextMenu(null); void saveTab(contextMenu.tabId);}}>Save tab</button>
      <button role="menuitem" disabled={selected?.route?.view !== 'write' || !writingTabs.some(tab => tab.id !== contextMenu.tabId)} onClick={() => {setContextMenu(null); openBesideTab(contextMenu.tabId);}}>Open beside</button>
      {(['left','right'] as const).map(side=><button key={side} role="menuitem" disabled={!selected?.route||!placeTabInPane(tabs,split,active,selected.id,side)} onClick={()=>{setContextMenu(null);const pair=placeTabInPane(current.current,splitRef.current,active,contextMenu.tabId,side);if(pair)void openPair(pair.primary,pair.secondary,contextMenu.tabId);}}>Open on {side}</button>)}
      <button role="menuitem" disabled={!selected?.route} onClick={() => {setContextMenu(null); void moveTab(contextMenu.tabId, 'new');}}>Open in new window</button>
      {windowLabel !== 'main' && <button role="menuitem" disabled={!selected?.route} onClick={() => {setContextMenu(null); void moveTab(contextMenu.tabId, 'main');}}>Move back to main window</button>}
      <button role="menuitem" onClick={() => {setContextMenu(null); void closeTab(contextMenu.tabId);}}>Close tab</button>
      <button role="menuitem" onClick={() => {setContextMenu(null); freshWorkspace();}}>New workspace tab</button>
      <button role="menuitem" onClick={() => {setContextMenu(null); setShortcutsOpen(true);}}>Keyboard shortcuts</button>
      <button role="menuitem" onClick={() => {setContextMenu(null); void invoke('workspace_request_quit').catch(reason => setError(String(reason)));}}>Quit Novel Workspace</button>
    </div>}
    {shortcutsOpen && <div className="shortcut-overlay"><ShortcutSettings onClose={() => setShortcutsOpen(false)}/></div>}
    {error && <div className="save-error" role="alert">{error}</div>}
    {!tabs.length && <div className="empty-tabs" inert={shortcutsOpen}><button onClick={freshWorkspace}>Open workspace</button></div>}
    {split && <div role="separator" aria-label="Draft pane divider" aria-orientation={paneWidth < 640 ? 'horizontal' : 'vertical'} aria-valuemin={minimumRatio} aria-valuemax={maximumRatio} aria-valuenow={splitRatio} tabIndex={0} className="draft-divider" onPointerDown={event => {if (event.button !== 0) return;event.preventDefault();resizeStart.current = splitRatio;event.currentTarget.setPointerCapture(event.pointerId);}} onPointerMove={event => {if (resizeStart.current !== null) resizeSplit(event.clientX, event.clientY);}} onPointerUp={event => {if (resizeStart.current !== null) {resizeStart.current = null;event.currentTarget.releasePointerCapture(event.pointerId);}}} onPointerCancel={() => {if (resizeStart.current !== null) setSplitRatio(resizeStart.current);resizeStart.current = null;}} onKeyDown={event => {if (!(paneWidth < 640 ? ['ArrowUp','ArrowDown','Home','End'] : ['ArrowLeft','ArrowRight','Home','End']).includes(event.key)) return;event.preventDefault();const key=event.key;setSplitRatio(value=>clampSplitRatio(key === 'Home' ? minimumRatio : key === 'End' ? maximumRatio : value + (['ArrowRight','ArrowDown'].includes(key) ? 2 : -2), hostWidth));}}/>}
    {tabs.map(tab => <div id={'panel-' + tab.id} role="tabpanel" aria-labelledby={'tab-' + tab.id} hidden={split ? tab.id !== split.primary && tab.id !== split.secondary : active !== tab.id} data-draft-pane={split ? tab.id === split.primary ? 'primary' : tab.id === split.secondary ? 'secondary' : undefined : undefined} data-pane-active={active === tab.id} data-compact-binder={Boolean(split && split.primary === tab.id && paneWidth * splitRatio / 100 < 560)} onPointerDownCapture={() => {if (split && !work.current) selectTab(tab.id);}} onFocusCapture={() => {if (split && !work.current) selectTab(tab.id);}} inert={shortcutsOpen} key={tab.id} ref={node => {if (node) nodes.current.set(tab.id, node); else nodes.current.delete(tab.id);}}>
      <DesktopWorkspace onToggleSplit={(split ? tab.id===split.primary||tab.id===split.secondary : active===tab.id) ? toggleSplit : undefined} splitActive={Boolean(split)} onRegisterNavigation={navigationCallback(tab.id)} onNavigateBack={()=>void navigateTrail('back')} onNavigateForward={()=>void navigateTrail('forward')} canNavigateBack={navigation.current.canBack} canNavigateForward={navigation.current.canForward} flushRef={controller(tab.id)} tabId={tab.id} initialRoute={tab.route} initialUi={tab.ui} compact={split?.secondary === tab.id} keyboardActive={active === tab.id && !busy} tabActive={(split ? tab.id === split.primary || tab.id === split.secondary : active === tab.id) && (!busy||restoringTab===tab.id)}
        beforeProjectRecover={async projectId=>{for(const peer of current.current.filter(peer=>peer.route?.projectId===projectId))await drain(peer.id);}}
        sharedChangeVersion={versions[tab.id] ?? 0} onOpenRoute={openRoute} onOpenBeside={openBesideRoute} onOpenInPane={openInPaneRoute}
        onRouteReady={(route, title) => routeReady(tab.id, route, title)} onContentReady={() => void contentReady(tab.id)}
        onContentError={message=>void contentFailed(tab.id,message)}
        onSharedChange={projectId => {if (registered.current.has(tab.id)&&current.current.find(current=>current.id===tab.id)?.route?.projectId===projectId) void invoke('workspace_notify_change', {projectId, tabId: tab.id}).catch(reason => setError(String(reason)));}}/>
    </div>)}
  </div>;
}
