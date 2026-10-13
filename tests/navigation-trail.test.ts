import {describe,expect,it} from 'vitest';
import {NavigationTrail,type TrailLocation} from '../apps/writer/src/navigation-trail';
const at=(id:string,offset=0):TrailLocation=>({tabId:id,route:{projectId:'project',sceneId:id,view:'write'},bookmark:{native:{documentHash:'a'.repeat(64),cursor:[0,2],anchor:[0,0]},reading:{scrollTop:offset}}});
describe('transactional navigation trail',()=>{
 it('restores independent selection and passage bookmarks only after a successful commit',()=>{const trail=new NavigationTrail();trail.visit(at('a'));trail.visit(at('b'),at('a',420));const back=trail.begin('back')!;expect(back.target.bookmark?.reading.scrollTop).toBe(420);expect(back.target.bookmark?.native?.anchor).toEqual([0,0]);expect(trail.current?.tabId).toBe('b');expect(trail.commit(back)).toBe(true);expect(trail.current?.tabId).toBe('a');expect(trail.canForward).toBe(true);});
 it('failed ownership or stale native restoration leaves both trail cursor and forward entries intact',()=>{const trail=new NavigationTrail();trail.visit(at('a'));trail.visit(at('b'));const ticket=trail.begin('back')!;expect(trail.current?.tabId).toBe('b');expect(trail.begin('back')).toEqual(ticket);expect(trail.canForward).toBe(false);trail.updateCurrent(at('b',100));expect(trail.commit(ticket)).toBe(false);expect(trail.current?.bookmark?.reading.scrollTop).toBe(100);});
 it('a new successful destination discards the forward branch and bounds retained bookmarks',()=>{const trail=new NavigationTrail(3);for(const id of ['a','b','c','d'])trail.visit(at(id));trail.commit(trail.begin('back')!);trail.visit(at('new'));expect(trail.canForward).toBe(false);trail.commit(trail.begin('back')!);trail.commit(trail.begin('back')!);expect(trail.current?.tabId).toBe('b');expect(trail.canBack).toBe(false);});
 it('closed or transferred tabs are pruned without revisiting foreign ownership',()=>{const trail=new NavigationTrail();trail.visit(at('a'));trail.visit(at('b'));trail.visit(at('c'));const old=trail.begin('back')!;trail.removeTab('b');expect(trail.begin('back')?.target.tabId).toBe('a');expect(trail.commit(old)).toBe(false);expect(trail.current?.tabId).toBe('c');});
 it('snapshot values cannot be mutated by consumers or callers',()=>{const trail=new NavigationTrail(),entry=at('يس مِ 中文',100);trail.visit(entry);entry.bookmark!.reading.scrollTop=999;const current=trail.current!;current.bookmark!.native!.cursor[1]=99;expect(trail.current?.bookmark?.reading.scrollTop).toBe(100);expect(trail.current?.bookmark?.native?.cursor).toEqual([0,2]);});
});

it('pruning preserves the exact repeated visit rather than rewinding to the first matching route',()=>{
 const trail=new NavigationTrail();for(const [id,offset] of [['a',100],['b',200],['a',300],['c',400],['a',500]] as const)trail.visit(at(id,offset));
 trail.removeTab('b');expect(trail.current?.bookmark?.reading.scrollTop).toBe(500);expect(trail.canForward).toBe(false);expect(trail.begin('back')?.target.tabId).toBe('c');
 trail.removeTab('a');expect(trail.current?.tabId).toBe('c');expect(trail.current?.bookmark?.reading.scrollTop).toBe(400);
});
