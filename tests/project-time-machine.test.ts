import React from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {describe,it,expect,vi} from 'vitest';
import {BiblePreview,recoverTimePiece,timeMachineScenes,canRecoverTimePiece} from '../apps/writer/src/ProjectTimeMachine';
import type {ProjectTimePreview,TimeMachineObject,Revision} from '../apps/writer/src/generated/workspace-contract';
import type {PlanningDocument} from '../apps/writer/src/planning/model';
const scene={schemaVersion:1,title:'يس مِ 中文',document:{type:'doc',content:[{type:'paragraph',attrs:{dir:'rtl',lang:'ar'},content:[{type:'text',text:'👩🏽‍🚀 مِ',marks:[{type:'bold'},{type:'underline'}]}]}]}};
function revision(entityId='scene',entityType='scene',payload:unknown=scene):Revision{return {id:'saved-version',projectId:'project',entityId,entityType,parents:[],payload,operation:entityType==='projectData'?'planning':'save',schemaVersion:1,checksum:'native-verified',createdAt:'saved timestamp',sequence:10,originEpoch:'author-epoch',originDevice:'author-device'};}
function object(entityId='scene',entityType='scene'):TimeMachineObject{return {entityId,entityType,heads:[{revisionId:'saved-version',sequence:10,entityType,title:'يس مِ 中文',createdAt:'saved timestamp',order:null,chapterId:null,position:null}],currentHeadIds:['current-head'],chapterId:'chapter',position:0};}
function preview(objects:TimeMachineObject[]=[object()]):ProjectTimePreview{return {epoch:'local-epoch',pointRevisionId:'exact-point',projectId:'project',sequence:10,objects,warnings:[]};}
function bible():PlanningDocument{return {schemaVersion:1,fields:[],records:[],plotPoints:[],facts:[],boards:[],lanes:[{id:'main-lane',name:'Main',kind:'main'}],events:[],constraints:[],issues:[]};}
describe('project time machine recovery and bounded preview',()=>{
 it('drains before mutation and sends exact point identity and reviewed current heads without copying payload over IPC',async()=>{
  const order:string[]=[];let release!:()=>void;const flush=vi.fn(async()=>{order.push('drain');await new Promise<void>(resolve=>{release=resolve;});});
  const rpc=vi.fn(async()=>{order.push('recover');return {...revision(),id:'new-revision'};});const pending=recoverTimePiece(preview(),object(),revision(),flush,rpc as any);
  expect(order).toEqual(['drain']);expect(rpc).not.toHaveBeenCalled();release();const result=await pending;expect(order).toEqual(['drain','recover']);expect(result.id).toBe('new-revision');
  expect(rpc).toHaveBeenCalledExactlyOnceWith('recover_time_piece',{projectId:'project',epoch:'local-epoch',sequence:10,pointRevisionId:'exact-point',entityId:'scene',entityType:'scene',revisionId:'saved-version',expectedHeads:['current-head']});
 });
 it('preserves drafts on failed drain and does not silently retry after concurrent native CAS rejection',async()=>{
  const rpc=vi.fn();await expect(recoverTimePiece(preview(),object(),revision(),async()=>{throw new Error('Composition or draft save failed');},rpc)).rejects.toThrow('draft save failed');expect(rpc).not.toHaveBeenCalled();
  rpc.mockRejectedValue(new Error('Conflict: another window saved'));await expect(recoverTimePiece(preview(),object(),revision(),async()=>{},rpc)).rejects.toThrow('Conflict');expect(rpc).toHaveBeenCalledOnce();expect(rpc.mock.calls[0][1].expectedHeads).toEqual(['current-head']);
 });
 it('rejects current conflicts, foreign revisions and past deletions before any write or drain',async()=>{
  const flush=vi.fn(),rpc=vi.fn(),competing={...object(),currentHeadIds:['left','right']};expect(canRecoverTimePiece(competing,revision())).toBe(false);
  for(const [group,rev] of [[competing,revision()],[object(),{...revision(),projectId:'foreign'}],[object(),{...revision(),entityType:'tombstone',payload:{deleted:true}}]] as const)await expect(recoverTimePiece(preview(),group,rev,flush,rpc)).rejects.toThrow();
  expect(flush).not.toHaveBeenCalled();expect(rpc).not.toHaveBeenCalled();
 });
 it('recovers the explicitly selected whole story bible with the same source/head safeguards',async()=>{
  const group=object('project:planning','projectData'),rev=revision('project:planning','projectData',bible()),rpc=vi.fn(async(_operation:string,_args:Record<string,unknown>)=>({...rev,id:'new-bible'}));
  await recoverTimePiece(preview([group]),group,rev,async()=>{},rpc as any);expect(rpc.mock.calls[0][1]).toMatchObject({entityId:'project:planning',entityType:'projectData',revisionId:'saved-version',expectedHeads:['current-head']});expect(rev.payload).toEqual(bible());
 });
 it('orders recorded hierarchy without inventing positions for missing legacy placements',()=>{
  const first={...object('first'),chapterId:'second-chapter',position:0},second={...object('second'),position:3},third={...object('third'),position:0},legacy={...object('legacy'),chapterId:null,position:null};
  const chapters=[{...object('chapter','chapter'),position:0},{...object('second-chapter','chapter'),position:1}];expect(timeMachineScenes(preview([first,second,legacy,third,...chapters])).map(value=>value.entityId)).toEqual(['third','second','first','legacy']);expect(legacy.position).toBeNull();
 });
 it('bounds large story bible DOM to30 records/facts/events each and does not serialize a closed full-data disclosure',()=>{
  const doc=bible();doc.records=Array.from({length:1000},(_,i)=>({id:'record-'+i,kind:'character' as const,name:'Character '+i,aliases:[],notes:'مِ 中文',values:{},links:[]}));doc.facts=Array.from({length:1000},(_,i)=>({id:'fact-'+i,subjectId:'record-'+i,property:'home',value:'Value '+i,status:'fact' as const,confirmed:true,observerId:null,source:'author',eventId:null,untilEventId:null}));doc.events=Array.from({length:1000},(_,i)=>({id:'event-'+i,title:'Event '+i,notes:'يس',laneIds:[],sceneIds:[],time:{kind:'unknown' as const}}));
  const stringify=vi.spyOn(JSON,'stringify');try{const html=renderToStaticMarkup(React.createElement(BiblePreview,{planning:doc}));expect(html).toContain('Character 29');expect(html).not.toContain('Character 30');expect(html).toContain('Value 29');expect(html).not.toContain('Value 30');expect(html).toContain('Event 29');expect(html).not.toContain('Event 30');expect(stringify.mock.calls.some(call=>call[0]===doc)).toBe(false);}finally{stringify.mockRestore();}
 });
});
