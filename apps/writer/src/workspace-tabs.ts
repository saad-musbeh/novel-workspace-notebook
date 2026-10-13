export type WorkspaceView = 'write' | 'plan' | 'review' | 'library' | 'alphabets' | 'book';
export interface TabRoute { projectId: string; sceneId?: string; view: WorkspaceView }
export interface WorkspaceTab { id: string; route: TabRoute; title: string; ui?: { scrollTop: number; bookSceneId?:string; bookSceneOffset?:number; bookCharacterId?:string; bookFoldedSceneIds?:string[] } }
export interface IncomingTab { transferId: string; tab: WorkspaceTab; phase: 'preparing' | 'awaiting_ready' }
export interface WindowTabs { windowLabel: string; tabs: WorkspaceTab[]; incoming: IncomingTab[] }

export function tabTitle(title: string): string {
  return [...title.replace(/[\u0000-\u001f\u007f-\u009f]/g, ' ')].slice(0, 200).join('').trim() || 'Untitled scene';
}

// Tools belong to a project; writing tabs belong to an exact scene.
export function sameRoute(a: TabRoute, b: TabRoute): boolean {
  return a.projectId === b.projectId && a.view === b.view &&
    (a.view !== 'write' || a.sceneId === b.sceneId);
}

export function mergeWindowTabs(current: WorkspaceTab[], state: WindowTabs): WorkspaceTab[] {
  const available = new Map(state.tabs.map(tab => [tab.id, tab]));
  for (const incoming of state.incoming) {
    if (incoming.phase === 'awaiting_ready') available.set(incoming.tab.id, incoming.tab);
  }
  // Preserve mounted tab order and React identity during a handoff.
  const result = current.filter(tab => available.has(tab.id)).map(tab => available.get(tab.id)!);
  for (const tab of available.values()) if (!result.some(item => item.id === tab.id)) result.push(tab);
  return result;
}

export const TAB_DRAG_TYPE = 'application/x-novel-workspace-tab';
export const TAB_DRAG_THRESHOLD = 32;
export interface TabDragPayload { tabId: string; sourceWindow: string }
export interface TabDragSession extends TabDragPayload { startX: number; startY: number; cancelled: boolean; dropped: boolean }
export interface TabStripBounds { left: number; right: number; top: number; bottom: number }

export function readTabDrag(value: string): TabDragPayload | null {
  if (!value || value.length > 1024) return null;
  try {
    const payload: unknown = JSON.parse(value);
    if (!payload || typeof payload !== 'object') return null;
    const {tabId, sourceWindow} = payload as Partial<TabDragPayload>;
    if (typeof tabId !== 'string' || !tabId || tabId.length > 200 || typeof sourceWindow !== 'string' || !sourceWindow || sourceWindow.length > 200) return null;
    return {tabId, sourceWindow};
  } catch {return null;}
}

export function reorderWindowTabs<T extends {id: string}>(tabs: T[], tabId: string, beforeId?: string): T[] {
  const moving = tabs.find(tab => tab.id === tabId);
  if (!moving || tabId === beforeId || (beforeId !== undefined && !tabs.some(tab => tab.id === beforeId))) return tabs;
  const next = tabs.filter(tab => tab.id !== tabId);
  const index = beforeId === undefined ? next.length : next.findIndex(tab => tab.id === beforeId);
  next.splice(index, 0, moving);
  return next;
}

export function shouldDetachTab(drag: TabDragSession, x: number, y: number, bounds: TabStripBounds, dropEffect: string): boolean {
  // Some native webviews report (0,0) when a drag is cancelled or leaves the surface.
  if (drag.cancelled || drag.dropped || dropEffect !== 'none' || !Number.isFinite(x) || !Number.isFinite(y) || (x === 0 && y === 0)) return false;
  if (Math.hypot(x - drag.startX, y - drag.startY) < TAB_DRAG_THRESHOLD) return false;
  const margin = 24;
  return x < bounds.left - margin || x > bounds.right + margin || y < bounds.top - margin || y > bounds.bottom + margin;
}

export interface DraftSplit { primary: string; secondary: string }
export function draftSplit(tabs: readonly {id: string; route?: TabRoute}[], primary: string, secondary: string): DraftSplit | null {
  const first = tabs.find(tab => tab.id === primary), second = tabs.find(tab => tab.id === secondary);
  if (!first?.route || !second?.route || primary === secondary || first.route.view !== 'write' || second.route.view !== 'write' || !first.route.sceneId || !second.route.sceneId || sameRoute(first.route, second.route)) return null;
  return {primary, secondary};
}
export function clampSplitRatio(value: number, width = 960): number {
  const minimum = Math.max(25, Math.ceil(240 / Math.max(640, Number.isFinite(width) ? width : 960) * 100));
  return Math.min(100 - minimum, Math.max(minimum, Number.isFinite(value) ? Math.round(value) : 50));
}

export type PaneSide = 'left' | 'right';
/** Separate canonical routes can share a window without sharing an editor session. */
export function paneSplit(tabs: readonly {id:string;route?:TabRoute}[],primary:string,secondary:string):DraftSplit|null {
 const first=tabs.find(tab=>tab.id===primary),second=tabs.find(tab=>tab.id===secondary);
 if(!first?.route||!second?.route||primary===secondary||sameRoute(first.route,second.route)||(first.route.view==='write'&&!first.route.sceneId)||(second.route.view==='write'&&!second.route.sceneId))return null;
 return {primary,secondary};
}
/** Dropping the already-visible opposite tab swaps panes; a new tab replaces only the requested side. */
export function placeTabInPane(tabs:readonly {id:string;route?:TabRoute}[],current:DraftSplit|null,active:string,tabId:string,side:PaneSide):DraftSplit|null {
 const currentOther=current?(side==='left'?current.secondary:current.primary):active;
 const counterpart=currentOther!==tabId&&tabs.some(tab=>tab.id===currentOther&&tab.route)?currentOther:
  current?(side==='left'?current.primary:current.secondary):tabs.find(tab=>tab.id!==tabId&&tab.route)?.id;
 if(!counterpart||counterpart===tabId)return null;
 return side==='left'?paneSplit(tabs,tabId,counterpart):paneSplit(tabs,counterpart,tabId);
}
export function readBookFoldedScenes(value:string|undefined):string[]|undefined {
 if(!value||value.length>2_640_000)return undefined;
 try{const parsed:unknown=JSON.parse(value);if(!Array.isArray(parsed)||parsed.length>20000||parsed.some(id=>typeof id!=='string'||!id||id.length>128||!/^[A-Za-z0-9_:-]+$/.test(id))||new Set(parsed).size!==parsed.length)return undefined;return parsed;}catch{return undefined;}
}

export const SCENE_DRAG_TYPE='application/x-novel-workspace-scene';
export interface SceneDragPayload {projectId:string;sceneId:string;title:string}
/** A drag conveys a route hint only. Native membership and title remain authoritative. */
export function readSceneDrag(value:string):SceneDragPayload|null {
 if(!value||value.length>4096)return null;
 try{const payload:unknown=JSON.parse(value);if(!payload||typeof payload!=='object'||Array.isArray(payload))return null;
 const {projectId,sceneId,title}=payload as Partial<SceneDragPayload>;
 if(typeof projectId!=='string'||typeof sceneId!=='string'||![projectId,sceneId].every(id=>id.length>0&&id.length<=128&&/^[A-Za-z0-9_:-]+$/.test(id))||typeof title!=='string'||title.length>1000)return null;
 return {projectId,sceneId,title:tabTitle(title)};
 }catch{return null;}
}
