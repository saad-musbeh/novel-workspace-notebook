import {describe, expect, it} from 'vitest';
import {mergeWindowTabs, sameRoute, readSceneDrag, type WorkspaceTab} from '../apps/writer/src/workspace-tabs';

const scene=(id:string):WorkspaceTab=>({id,route:{projectId:'project',sceneId:id,view:'write'},title:id});
describe('desktop tab identity and transfer visibility',()=>{
 it('writing routes distinguish scenes, and tool routes belong to their project',()=>{
  expect(sameRoute(scene('a').route,scene('b').route)).toBe(false);
  expect(sameRoute({...scene('a').route,view:'plan'},{...scene('b').route,view:'plan'})).toBe(true);
  expect(sameRoute({...scene('a').route,view:'plan'},{projectId:'other',view:'plan'})).toBe(false);
 });
 it('does not mount a destination before its session handoff is ready',()=>{
  const incoming={transferId:'move',tab:scene('a'),phase:'preparing' as const};
  expect(mergeWindowTabs([],{windowLabel:'target',tabs:[],incoming:[incoming]})).toEqual([]);
  expect(mergeWindowTabs([],{windowLabel:'target',tabs:[],incoming:[{...incoming,phase:'awaiting_ready'}]})).toEqual([scene('a')]);
 });
 it('preserves mounted order, removes committed source tabs, and never duplicates incoming tabs',()=>{
  expect(mergeWindowTabs([scene('a'),scene('b')],{windowLabel:'source',tabs:[scene('b'),scene('a')],incoming:[]})).toEqual([scene('a'),scene('b')]);
  expect(mergeWindowTabs([scene('a'),scene('b')],{windowLabel:'source',tabs:[scene('b')],incoming:[]})).toEqual([scene('b')]);
  expect(mergeWindowTabs([scene('a')],{windowLabel:'target',tabs:[scene('a')],incoming:[{transferId:'move',tab:scene('a'),phase:'awaiting_ready'}]})).toEqual([scene('a')]);
 });
});

import {readTabDrag,reorderWindowTabs,shouldDetachTab,TAB_DRAG_THRESHOLD,type TabDragSession} from '../apps/writer/src/workspace-tabs';
describe('tab drag intent',()=>{
 const bounds={left:0,right:600,top:0,bottom:44};
 const dragging:TabDragSession={tabId:'a',sourceWindow:'main',startX:100,startY:20,cancelled:false,dropped:false};
 it('reorders the exact dragged tab and retains that order after registry refresh',()=>{
  const ordered=reorderWindowTabs([scene('a'),scene('b'),scene('c')],'a','c');
  expect(ordered.map(tab=>tab.id)).toEqual(['b','a','c']);
  expect(mergeWindowTabs(ordered,{windowLabel:'main',tabs:[scene('c'),scene('a'),scene('b')],incoming:[]}).map(tab=>tab.id)).toEqual(['b','a','c']);
  expect(reorderWindowTabs(ordered,'b').map(tab=>tab.id)).toEqual(['a','c','b']);
  expect(reorderWindowTabs(ordered,'missing','c')).toBe(ordered);
  expect(reorderWindowTabs(ordered,'b','missing')).toBe(ordered);
 });
 it('requires an explicit distance and outside-strip margin before detaching',()=>{
  expect(shouldDetachTab(dragging,100,100,bounds,'none')).toBe(true);
  expect(shouldDetachTab(dragging,100,60,bounds,'none')).toBe(false);
  expect(shouldDetachTab({...dragging,startY:90},100,90+TAB_DRAG_THRESHOLD-1,bounds,'none')).toBe(false);
  expect(shouldDetachTab(dragging,200,25,bounds,'none')).toBe(false);
 });
 it('never detaches a cancelled/successfully dropped drag or invalid native coordinates',()=>{
  expect(shouldDetachTab({...dragging,cancelled:true},100,100,bounds,'none')).toBe(false);
  expect(shouldDetachTab({...dragging,dropped:true},100,100,bounds,'none')).toBe(false);
  expect(shouldDetachTab(dragging,100,100,bounds,'move')).toBe(false);
  expect(shouldDetachTab(dragging,0,0,bounds,'none')).toBe(false);
  expect(shouldDetachTab(dragging,NaN,100,bounds,'none')).toBe(false);
 });
 it('accepts only bounded tab/window identity payloads without trusting their authority',()=>{
  expect(readTabDrag(JSON.stringify({tabId:'a',sourceWindow:'main',projectId:'ignored'}))).toEqual({tabId:'a',sourceWindow:'main'});
  for(const value of ['', 'bad json','null','[]','{}','{"tabId":"a","sourceWindow":4}',JSON.stringify({tabId:'a'.repeat(201),sourceWindow:'main'})])expect(readTabDrag(value)).toBeNull();
 });
});

import {clampSplitRatio,draftSplit} from '../apps/writer/src/workspace-tabs';
describe('distinct draft split bindings',()=>{
 it('accepts two distinct writing sessions and refuses tool/duplicate scene bindings',()=>{
  expect(draftSplit([scene('a'),scene('b')],'a','b')).toEqual({primary:'a',secondary:'b'});
  expect(draftSplit([scene('a')],'a','a')).toBeNull();
  expect(draftSplit([scene('a'),{...scene('b'),route:scene('a').route}],'a','b')).toBeNull();
  expect(draftSplit([scene('a'),{...scene('b'),route:{...scene('b').route,view:'library'}}],'a','b')).toBeNull();
  expect(draftSplit([scene('a'),scene('b')],'a','missing')).toBeNull();
 });
 it('keeps the divider bounded and at least 240 px per pane at the minimum layout width',()=>{
  expect(clampSplitRatio(-10,1200)).toBe(25);expect(clampSplitRatio(100,1200)).toBe(75);
  expect(clampSplitRatio(0,640)).toBe(38);expect(clampSplitRatio(100,640)).toBe(62);
  expect(clampSplitRatio(NaN,640)).toBe(50);
 });
});

import {paneSplit,placeTabInPane,readBookFoldedScenes} from '../apps/writer/src/workspace-tabs';
describe('explicit tab pane placement and safe book handoff metadata',()=>{
 const tool={...scene('planning'),route:{projectId:'project',view:'plan' as const}};
 it('allows a draft with an independently owned project tool while rejecting duplicate canonical tools',()=>{
  expect(paneSplit([scene('a'),tool],'a','planning')).toEqual({primary:'a',secondary:'planning'});
  expect(paneSplit([tool,{...tool,id:'copy'}],'planning','copy')).toBeNull();
  expect(paneSplit([scene('a'),{...scene('b'),route:scene('a').route}],'a','b')).toBeNull();
  expect(paneSplit([{id:'opening'},scene('a')],'opening','a')).toBeNull();
 });
 it('places a hidden tab only on its chosen side and swaps visible panes without duplicating a session',()=>{
  const tabs=[scene('a'),scene('b'),scene('c'),tool];
  expect(placeTabInPane(tabs,null,'a','b','left')).toEqual({primary:'b',secondary:'a'});
  expect(placeTabInPane(tabs,null,'a','b','right')).toEqual({primary:'a',secondary:'b'});
  expect(placeTabInPane(tabs,{primary:'a',secondary:'b'},'b','c','left')).toEqual({primary:'c',secondary:'b'});
  expect(placeTabInPane(tabs,{primary:'a',secondary:'b'},'a','planning','right')).toEqual({primary:'a',secondary:'planning'});
  expect(placeTabInPane(tabs,{primary:'a',secondary:'b'},'a','b','left')).toEqual({primary:'b',secondary:'a'});
  expect(placeTabInPane([scene('a')],null,'a','a','right')).toBeNull();
 });
 it('keeps book fold IDs bounded and unique and rejects malformed or injected payloads',()=>{
  expect(readBookFoldedScenes('["scene-a","scene-b"]')).toEqual(['scene-a','scene-b']);
  expect(readBookFoldedScenes('[]')).toEqual([]);
  for(const value of ['bad','null','{}','["a","a"]','[4]','["../private"]',JSON.stringify(['a'.repeat(129)]),JSON.stringify(Array(20001).fill('scene'))])expect(readBookFoldedScenes(value)).toBeUndefined();
 });
});

describe('bounded binder route hints',()=>{
 it('retains only bounded canonical ID hints and inert Unicode titles',()=>{
  expect(readSceneDrag(JSON.stringify({projectId:'project-id',sceneId:'scene:1',title:'نور 中文',destination:'forged'}))).toEqual({projectId:'project-id',sceneId:'scene:1',title:'نور 中文'});
  for(const payload of ['x','[]',JSON.stringify({projectId:'../../private',sceneId:'scene',title:'x'}),JSON.stringify({projectId:'project',sceneId:'',title:'x'}),JSON.stringify({projectId:'project',sceneId:'scene',title:'x'.repeat(1001)}),'x'.repeat(4097)])expect(readSceneDrag(payload)).toBeNull();
 });
});
