import {describe,it,expect,vi} from 'vitest';
import {ReviewSources} from '../apps/writer/src/review-sources';
import type {SceneRecord} from '../apps/writer/src/workspace';
const scene=(id:string,text='مِ中文👩🏽‍🚀',revisionId='r1'):SceneRecord=>({id,chapterId:'chapter',position:0,revisionId,scene:{schemaVersion:1,title:id,document:{type:'doc',content:[{type:'paragraph',attrs:{lang:'ar',dir:'rtl'},content:[{type:'text',text}]}]}}});
describe('incremental saved review sources',()=>{
 it('hashes only the changed scene in a thousand-scene project',async()=>{
  const digest=vi.fn(async(text:string)=>'hash:'+text),cache=new ReviewSources(digest);
  const records=Array.from({length:1000},(_,i)=>scene(String(i)));
  await cache.refresh(records);expect(digest).toHaveBeenCalledTimes(1000);
  records[0]=scene('0','new مِ中文','r2');
  const updated=await cache.refresh(records);expect(digest).toHaveBeenCalledTimes(1001);
  expect(updated[0]).toMatchObject({revisionId:'r2',text:'new مِ中文',hash:'hash:new مِ中文'});
 });
 it('reuses reloaded exact revisions while updating metadata',async()=>{
  const digest=vi.fn(async()=> 'hash'),cache=new ReviewSources(digest);
  await cache.refresh([scene('one')]);const reloaded=scene('one');reloaded.scene.title='Renamed';reloaded.scene.document.content![0].attrs!.lang='zh-Hant';
  expect((await cache.refresh([reloaded]))[0]).toMatchObject({title:'Renamed',language:'zh-Hant'});expect(digest).toHaveBeenCalledTimes(1);
 });
 it('rechecks changed text even with a reused revision id',async()=>{
  const digest=vi.fn(async(text:string)=>text),cache=new ReviewSources(digest);
  await cache.refresh([scene('one','مِ')]);expect((await cache.refresh([scene('one','م')]))[0].hash).toBe('م');expect(digest).toHaveBeenCalledTimes(2);
 });
 it('reads a new revision even when its document object is reused',async()=>{
  const cache=new ReviewSources(async(text:string)=>text),record=scene('one','old');
  await cache.refresh([record]);record.revisionId='r2';record.scene.document.content![0].content![0].text='new مِ';
  expect((await cache.refresh([record]))[0]).toMatchObject({revisionId:'r2',text:'new مِ',hash:'new مِ'});
 });
 it('does not reuse a digest for an unacknowledged revision or removed source',async()=>{
  const digest=vi.fn(async()=> 'hash'),cache=new ReviewSources(digest);
  await cache.refresh([scene('one')]);await cache.refresh([scene('one',undefined,'r2')]);await cache.refresh([]);await cache.refresh([scene('one',undefined,'r2')]);expect(digest).toHaveBeenCalledTimes(3);
 });
 it('keeps in-flight results bound to their own exact revision',async()=>{
  let release!:(value:string)=>void;
  const digest=vi.fn((text:string)=>text==='old'?new Promise<string>(resolve=>{release=resolve;}):Promise.resolve(text));
  const cache=new ReviewSources(digest),old=cache.refresh([scene('one','old')]);
  const current=await cache.refresh([scene('one','new','r2')]);release('old-hash');
  expect((await old)[0]).toMatchObject({text:'old',revisionId:'r1',hash:'old-hash'});expect(current[0]).toMatchObject({text:'new',revisionId:'r2',hash:'new'});
  await cache.refresh([scene('one','new','r2')]);expect(digest).toHaveBeenCalledTimes(2);
 });
 it('retries a failed hash without publishing unverified sources',async()=>{
  const digest=vi.fn().mockRejectedValueOnce(new Error('Digest failed')).mockResolvedValue('verified'),cache=new ReviewSources(digest);
  await expect(cache.refresh([scene('one')])).rejects.toThrow('Digest failed');expect((await cache.refresh([scene('one')]))[0].hash).toBe('verified');
 });
 it('captures exact revision, title, language and text before an in-flight digest resolves',async()=>{
  let release!:(value:string)=>void;
  const digest=vi.fn(()=>new Promise<string>(resolve=>{release=resolve;})),cache=new ReviewSources(digest),record=scene('one','Original مِ 中文','original-head');
  record.scene.title='Original title 中文';const original=structuredClone(record),request=cache.refresh([record]);
  expect(digest).toHaveBeenCalledExactlyOnceWith('Original مِ 中文');
  record.id='mutated-id';record.revisionId='new-head';record.scene.title='New title';record.scene.document.content![0].attrs!.lang='zh-Hant';record.scene.document.content![0].content![0].text='New text';
  release('verified-original');expect(await request).toEqual([{sceneId:original.id,revisionId:'original-head',title:'Original title 中文',language:'ar',text:'Original مِ 中文',hash:'verified-original'}]);
 });
 it('aborts a pending hash without publishing a result, then retries the exact revision successfully',async()=>{
  let release!:(value:string)=>void;
  const digest=vi.fn().mockImplementationOnce(()=>new Promise<string>(resolve=>{release=resolve;})).mockResolvedValue('verified-retry');
  const cache=new ReviewSources(digest),controller=new AbortController(),publish=vi.fn(),record=scene('one');
  const pending=cache.refresh([record],controller.signal).then(publish);
  const rejection=expect(pending).rejects.toThrow('Review closed');controller.abort(new Error('Review closed'));release('obsolete');await rejection;
  expect(publish).not.toHaveBeenCalled();expect(await cache.refresh([record])).toEqual([expect.objectContaining({revisionId:'r1',text:'مِ中文👩🏽‍🚀',hash:'verified-retry'})]);expect(digest).toHaveBeenCalledTimes(2);
 });
 it('an aborted older request cannot clear a newer acknowledged digest cache',async()=>{
  let release!:(value:string)=>void;
  const digest=vi.fn((text:string)=>text==='old'?new Promise<string>(resolve=>{release=resolve;}):Promise.resolve('verified:'+text));
  const cache=new ReviewSources(digest),controller=new AbortController(),pending=cache.refresh([scene('one','old')],controller.signal);
  const rejection=expect(pending).rejects.toThrow('Old request cancelled');controller.abort(new Error('Old request cancelled'));
  const current=await cache.refresh([scene('one','new مِ','r2')]);release('old-hash');await rejection;
  expect(current[0]).toMatchObject({revisionId:'r2',text:'new مِ',hash:'verified:new مِ'});
  expect(await cache.refresh([scene('one','new مِ','r2')])).toEqual(current);expect(digest).toHaveBeenCalledTimes(2);
 });

});
