import {describe,expect,it} from 'vitest';
import {BOOK_FOLDED_HEIGHT,bookAnchor,bookAnchorTop,bookRows,bookViewport,characterSceneIds,initialBookFolds,manuscriptOrder,visibleBookRows} from '../apps/writer/src/book-model';
import {createPlanningDocument} from '../apps/writer/src/planning/model';
import type {ProjectIndex,SceneSummary} from '../apps/writer/src/workspace';
function fixture():ProjectIndex{return {chapterNumberStart:1,chapterNumberingRevisionId:null,chapterNumberingConflicted:false,project:{id:'project',title:'Book',createdAt:'fixture'},chapters:[{id:'chapter-a',projectId:'project',position:0,title:'First'},{id:'chapter-b',projectId:'project',position:1,title:'Second'}],scenes:[{id:'b',chapterId:'chapter-b',position:0,title:'Last',revisionId:'head-b'},{id:'a2',chapterId:'chapter-a',position:1,title:'نور / 明 appears only in title',revisionId:'head-a2'},{id:'a1',chapterId:'chapter-a',position:0,title:'First',revisionId:'head-a1'}]};}
describe('whole book metadata projections',()=>{
 it('restores only bounded known fold identities without modifying their expanded heights',()=>{
  const scenes=manuscriptOrder(fixture()),heights=new Map([['a1',8297]]);
  const folded=initialBookFolds(['a1','a1','deleted',null,3],scenes);
  expect(folded).toEqual(new Set(['a1']));expect(initialBookFolds('a1',scenes)).toEqual(new Set());
  expect(bookRows(scenes,heights,folded)[0].height).toBe(BOOK_FOLDED_HEIGHT);
  expect(bookRows(scenes,heights)[0].height).toBe(8297);expect(heights.get('a1')).toBe(8297);
  expect(initialBookFolds([...Array(20000).fill('missing'),'a1'],scenes)).toEqual(new Set());
 });
 it('keeps logical downstream anchors across fold/unfold and clamps a folded current scene to its header',()=>{
  const scenes=manuscriptOrder(fixture()),heights=new Map([['a1',3000]]),expanded=bookRows(scenes,heights),folded=bookRows(scenes,heights,new Set(['a1']));
  const anchor=bookAnchor(expanded,3200)!;expect(anchor).toEqual({sceneId:'a2',offset:200});
  expect(bookAnchorTop(folded,anchor)).toBe(BOOK_FOLDED_HEIGHT+200);
  expect(bookAnchorTop(expanded,anchor)).toBe(3200);
  expect(bookAnchorTop(folded,{sceneId:'a1',offset:1500})).toBe(0);
  expect(bookAnchorTop(folded,{sceneId:'deleted',offset:0})).toBeNull();expect(bookAnchor([],-4)).toBeNull();
 });
 it('orders scenes by authored chapter/scene order without mutating metadata',()=>{const index=fixture(),frozen=JSON.stringify(index);expect(manuscriptOrder(index).map(scene=>scene.id)).toEqual(['a1','a2','b']);expect(JSON.stringify(index)).toBe(frozen);});
 it('chapter position defines order even when incoming chapter summaries are shuffled',()=>{const index=fixture();index.chapters.reverse();expect(manuscriptOrder(index).map(scene=>scene.id)).toEqual(['a1','a2','b']);});
 it('bounds visible rows to eight for a thousand-scene scroll surface and preserves exact scene identities',()=>{const base=fixture().scenes[0],scenes:SceneSummary[]=Array.from({length:1000},(_,i)=>({...base,id:'scene-'+i,position:i}));const rows=bookRows(scenes,new Map());expect(rows).toHaveLength(1000);expect(rows[999].top).toBe(999*560);const visible=visibleBookRows(rows,500*560,800);expect(visible.map(row=>row.scene.id)).toEqual(['scene-499','scene-500','scene-501']);expect(visibleBookRows(rows,0,1e8)).toHaveLength(8);expect(visibleBookRows(rows,1e9,800)).toEqual([]);expect(visibleBookRows([],0,800)).toEqual([]);});
 it('uses measured heights and caps ordinary oversize values while retaining a bounded viewport',()=>{const scenes=manuscriptOrder(fixture());const rows=bookRows(scenes,new Map([['a1',10],['a2',2000],['b',1e10]]));expect(rows.map(row=>row.height)).toEqual([200,2000,100000000]);expect(rows.map(row=>row.top)).toEqual([0,200,2200]);expect(bookViewport({top:-300,height:2000},{top:0,height:800})).toEqual({top:300,height:800});});
 it('character cuts use canonical authored scene/event/field/reveal/knowledge links, never prose/title name matching',()=>{const doc=createPlanningDocument();doc.fields=[{id:'links',label:'References',kind:'links',recordKind:'character',options:[],hidden:false}];doc.records=[{id:'hero',kind:'character',name:'نور / 明',aliases:[],notes:'',links:[{kind:'scene',id:'a1'},{kind:'scene',id:'deleted'}],values:{links:[{kind:'event',id:'appearance'}]}},{id:'place',kind:'location',name:'Library',aliases:[],notes:'',links:[{kind:'scene',id:'a2'}],values:{}}];doc.events=[{id:'appearance',title:'Appearance',notes:'',laneIds:[doc.lanes[0].id],sceneIds:['b'],time:{kind:'unknown'}}];const scenes=manuscriptOrder(fixture()),before=JSON.stringify(doc);expect(characterSceneIds(doc,scenes,'hero')).toEqual(new Set(['a1','b']));expect(characterSceneIds(doc,scenes,'place')).toEqual(new Set());expect(characterSceneIds(doc,scenes,'missing')).toEqual(new Set());expect(JSON.stringify(doc)).toBe(before);});
 it('explicit knowledge and fact reveals can add scene membership without assigning narrative time or approving facts',()=>{const doc=createPlanningDocument();doc.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[]}];doc.facts=[{id:'belief',subjectId:'hero',property:'Door',value:'مِ 中文',status:'belief',confirmed:false,observerId:null,source:'Author',eventId:null,untilEventId:null}];doc.readerReveals=[{id:'reveal',eventId:null,factId:'belief',scene:{kind:'scene',id:'b'},position:0,notes:''}];doc.knowledge=[{id:'knowledge',characterId:'hero',factId:'belief',stance:'believes',factSnapshot:{subjectId:'hero',property:'Door',value:'مِ 中文'},interpretation:'Door is open',source:'Authored',scope:{kind:'scenes',sceneIds:['a2']}}];const cut=characterSceneIds(doc,manuscriptOrder(fixture()),'hero');expect(manuscriptOrder(fixture()).filter(scene=>cut.has(scene.id)).map(scene=>scene.id)).toEqual(['a2','b']);expect(doc.facts[0].confirmed).toBe(false);});
 it('scans a thousand reveals once and visits a shared event once across subject facts',()=>{
  const doc=createPlanningDocument();
  doc.records=[{id:'hero',kind:'character',name:'Hero',aliases:[],notes:'',values:{},links:[]}];
  doc.facts=Array.from({length:1000},(_,i)=>({id:'fact-'+i,subjectId:'hero',property:'Claim '+i,value:'مِ 中文',status:'fact' as const,confirmed:true,observerId:null,source:'Authored',eventId:'shared-event',untilEventId:null}));
  let revealReads=0,eventSceneReads=0;
  doc.readerReveals=doc.facts.map((fact,i)=>({id:'reveal-'+i,get factId(){revealReads++;return fact.id;},eventId:null,scene:{kind:'scene' as const,id:i%2?'b':'a1'},position:i,notes:''}));
  doc.events=[{id:'shared-event',title:'Authored event',notes:'',laneIds:[doc.lanes[0].id],get sceneIds(){eventSceneReads++;return ['a2'];},time:{kind:'unknown'}}];
  expect(characterSceneIds(doc,manuscriptOrder(fixture()),'hero')).toEqual(new Set(['a1','a2','b']));
  expect(revealReads).toBeLessThanOrEqual(2000);
  // One index pass and one cut traversal; repeated subject facts must not rescan this event.
  expect(eventSceneReads).toBeLessThanOrEqual(2);
 });
 it('metadata-only rows never read a scene body',()=>{const index=fixture();for(const scene of index.scenes)Object.defineProperty(scene,'document',{get(){throw new Error('Scene body read');}});expect(visibleBookRows(bookRows(manuscriptOrder(index),new Map()),0,800)).toHaveLength(2);});
});
