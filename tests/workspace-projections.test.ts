import React from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {describe,expect,it,vi} from 'vitest';
import {filterManuscript,indexScenes,ManuscriptTree,projectReviewSources,revealManuscriptRow} from '../apps/writer/src/workspace-projections';
import type {Chapter,SceneSummary} from '../apps/writer/src/workspace';
import type {SceneSnapshot} from '../apps/writer/src/assistance/model';

const scene=(id:string,chapterId='chapter',position=0,revisionId='r-'+id):SceneSummary=>({id,chapterId,position,revisionId,title:id});
const source=(record:SceneSummary):SceneSnapshot=>({sceneId:record.id,revisionId:record.revisionId,hash:'a'.repeat(64),text:'مِ 中文 繁體 👩🏽‍🚀 \uE000 "English"',title:record.title,language:'ar'});
const facts=(projectId='project',revisionId='planning-head'):SceneSnapshot=>({kind:'facts',sceneId:'planning:'+projectId,revisionId,hash:'b'.repeat(64),text:'Synthetic approved facts'});
describe('workspace projections',()=>{
 it('groups and stably sorts copied chapter arrays, retaining exact scene objects and original order',()=>{
  const records=[scene('third','a',3),scene('first','a',1),scene('tie','a',1),scene('other','b'),scene('orphan','missing')],before=JSON.stringify(records),index=indexScenes(records);
  expect(index.byChapter.get('a')?.map(s=>s.id)).toEqual(['first','tie','third']);expect(index.byChapter.get('b')).toEqual([records[3]]);expect(index.byChapter.get('missing')).toEqual([records[4]]);
  expect(index.byId.get('first')).toBe(records[1]);expect(index.byChapter.get('a')?.[0]).toBe(records[1]);expect(JSON.stringify(records)).toBe(before);expect(indexScenes([]).byId.size).toBe(0);
 });
 it('filters exact current revisions without modifying frozen evidence, hashes or multilingual text',()=>{
  const current=scene('one'),changed=scene('two','chapter',1,'new-head'),deleted=scene('deleted'),foreign=scene('foreign'),stale=source({...changed,revisionId:'old-head'});
  const fresh=source(current),second=source(changed),input=[source(foreign),fresh,stale,source(deleted),second],fact=facts();
  const before=JSON.stringify({input,records:[current,changed],fact});Object.freeze(fresh);Object.freeze(second);
  const result=projectReviewSources(indexScenes([current,changed]),input,'project',fact,'planning-head');
  expect(result).toEqual([fresh,second,fact]);expect(result[0]).toBe(fresh);expect(result[1]).toBe(second);expect(result[2]).toBe(fact);
  expect(JSON.stringify({input,records:[current,changed],fact})).toBe(before);
 });
 it('does not leak facts from a prior project/head while an asynchronous refresh is pending',()=>{
  const record=scene('one'),snapshot=source(record),index=indexScenes([record]);
  expect(projectReviewSources(index,[snapshot],'new-project',facts('old-project'), 'planning-head')).toEqual([snapshot]);
  expect(projectReviewSources(index,[snapshot],'project',facts('project','old-head'),'new-head')).toEqual([snapshot]);
  expect(projectReviewSources(index,[snapshot],'project',facts(),null)).toEqual([snapshot]);
  expect(projectReviewSources(index,[snapshot],'project',null,'planning-head')).toEqual([snapshot]);
  expect(projectReviewSources(indexScenes([]),[snapshot],'new-project',null,null)).toEqual([]);
 });
 it('preserves source ordering even when chapters and manuscript positions differ',()=>{
  const records=[scene('b','chapter',2),scene('a','chapter',1)],snapshots=records.map(source),index=indexScenes(records);
  expect(index.byChapter.get('chapter')?.map(s=>s.id)).toEqual(['a','b']);expect(projectReviewSources(index,snapshots,'project',null,null)).toEqual(snapshots);
 });
 it('finds exact multilingual scene titles or complete matching chapters without reading bodies or changing order',()=>{
  const chapters:Chapter[]=[{id:'a',projectId:'project',title:'First chapter',position:0},{id:'b',projectId:'project',title:'Later 中文',position:1},{id:'empty',projectId:'project',title:'Empty chapter',position:2}];
  const records=[scene('alpha','a',1),scene('مِ نور','a',0),scene('繁體','b',0)];for(const record of records)Object.defineProperty(record,'scene',{get(){throw new Error('Search read a manuscript body');}});
  const index=indexScenes(records),before=JSON.stringify({chapters,records});
  expect(filterManuscript(chapters,index,'مِ').flatMap(group=>group.scenes.map(row=>row.id))).toEqual(['مِ نور']);
  expect(filterManuscript(chapters,index,'FIRST').flatMap(group=>group.scenes.map(row=>row.id))).toEqual(['مِ نور','alpha']);
  expect(filterManuscript(chapters,index,'中文').map(group=>group.chapter.id)).toEqual(['b']);
  expect(filterManuscript(chapters,index,'empty')).toEqual([{chapter:chapters[2],scenes:[]}]);
  expect(filterManuscript(chapters,index,'absent')).toEqual([]);expect(filterManuscript(chapters,index,'')).toHaveLength(3);
  expect(JSON.stringify({chapters,records})).toBe(before);
 });
 it('bounds scene-filter input without normalizing Unicode or reading other fields',()=>{
  const chapter:Chapter={id:'chapter',projectId:'project',title:'Chapter',position:0},record=scene('x'.repeat(1000));
  expect(filterManuscript([chapter],indexScenes([record]),'x'.repeat(1000)+'unread suffix')[0].scenes).toEqual([record]);
  const unicode=scene('e\u0301');expect(filterManuscript([chapter],indexScenes([unicode]),'é')).toEqual([]);
 });
 it('reveals an active row by moving only its own list scroller, leaving visible rows and hidden lists untouched',()=>{
  const scroller={scrollTop:500,clientTop:2,clientHeight:100,getBoundingClientRect:()=>({top:20})} as unknown as HTMLElement;
  revealManuscriptRow(scroller,{getBoundingClientRect:()=>({top:130,bottom:150})} as HTMLElement);expect(scroller.scrollTop).toBe(528);
  revealManuscriptRow(scroller,{getBoundingClientRect:()=>({top:10,bottom:40})} as HTMLElement);expect(scroller.scrollTop).toBe(516);
  revealManuscriptRow(scroller,{getBoundingClientRect:()=>({top:30,bottom:80})} as HTMLElement);expect(scroller.scrollTop).toBe(516);
  Object.assign(scroller,{clientHeight:0});revealManuscriptRow(scroller,{getBoundingClientRect:()=>{throw new Error('Hidden list measured a row');}} as unknown as HTMLElement);expect(scroller.scrollTop).toBe(516);
 });
 it('uses one scene-identity read per index rather than sources × scenes membership scans',()=>{
  for(const count of [1000,10000]){
   let reads=0;
   const records=Array.from({length:count},(_,i)=>{const record=scene('scene-'+i);Object.defineProperty(record,'id',{get(){reads++;return 'scene-'+i;},enumerable:true});return record;});
   const snapshots=Array.from({length:count},(_,i)=>({...source(scene('scene-'+i)),sceneId:'scene-'+i}));
   const index=indexScenes(records);expect(reads).toBe(count);
   const current=projectReviewSources(index,snapshots,'project',null,null);expect(current).toHaveLength(count);expect(reads).toBe(count);

  }
 });
 it('renders accessible chapter/scene controls with stable order and inert author titles',()=>{
  const chapters:Chapter[]=[{id:'a',projectId:'project',title:'Chapter <script> نور',position:0},{id:'b',projectId:'project',title:'第二章',position:1}],records=[scene('last','a',5),scene('first','a',1),scene('other','b')];records[1].title='مِ 中文 <img onerror="bad">';
  const before=JSON.stringify(records),open=vi.fn(),html=renderToStaticMarkup(React.createElement(ManuscriptTree,{chapters,index:indexScenes(records),activeId:'first',onOpenScene:open}));
  expect(html).toContain('role="navigation"');expect(html).toContain('aria-label="Chapters and scenes"');expect(html.match(/draggable="true"/g)).toHaveLength(records.length);expect(html).toContain('aria-current="page"');expect(html).toContain('Manuscript');expect(html).toContain('dir="auto"');expect(html).toContain('&lt;script&gt;');expect(html).toContain('&lt;img');expect(html).not.toContain('<script>');expect(html.indexOf('مِ 中文')).toBeLessThan(html.indexOf('>last<'));expect(open).not.toHaveBeenCalled();expect(JSON.stringify(records)).toBe(before);
 });
});
