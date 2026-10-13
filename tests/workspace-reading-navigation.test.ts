import {afterEach,describe,expect,it,vi} from 'vitest';
import {workspaceReadingController} from '../apps/writer/src/useWorkspaceNavigation';
afterEach(()=>vi.unstubAllGlobals());
describe('tool reading navigation',()=>{
 it('restores the actual scroll host without reading a scene or modifying its content',async()=>{
  const host={scrollTop:700,dispatchEvent:vi.fn(),dataset:{}},main={};
  vi.stubGlobal('document',{scrollingElement:host});vi.stubGlobal('getComputedStyle',()=>({overflowY:'visible'}));
  const root={querySelector:(selector:string)=>selector==='main'?main:null};
  const controller=workspaceReadingController(()=>root as any,()=> 'review');
  const saved=await controller.capture();host.scrollTop=40;await controller.restore(saved);
  expect(host.scrollTop).toBe(700);expect(host.dispatchEvent).toHaveBeenCalledOnce();
  await expect(controller.restore({reading:{scrollTop:Infinity}})).rejects.toThrow('Invalid');expect(host.scrollTop).toBe(700);
 });
 it('a vanished character filter refuses restoration before changing the book passage',async()=>{
  const host={scrollTop:80,dataset:{}},select={value:'',options:[{value:''}]};
  const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='[aria-label="Book selection"]'?select:null};
  const controller=workspaceReadingController(()=>root as any,()=> 'book');
  await expect(controller.restore({reading:{scrollTop:500,bookSceneId:'scene',bookSceneOffset:500,bookCharacterId:'deleted'}})).rejects.toThrow('no longer available');expect(host.scrollTop).toBe(80);
 });
});

it('book navigation uses the scoped reveal acknowledgement instead of estimating virtual scene geometry',async()=>{
 const host=Object.assign(new EventTarget(),{scrollTop:80,dataset:{bookSceneId:'current',bookSceneOffset:'80'}}),select={value:'hero',options:[{value:'hero'}]},book={dataset:{bookCharacter:'hero'},inert:false};
 const requests:any[]=[];host.addEventListener('novel-book-reveal',event=>{const request=(event as CustomEvent).detail;requests.push(request);host.dispatchEvent(new CustomEvent('novel-book-reveal-result',{detail:{requestId:'wrong',ok:false,error:'Ignore unrelated reply'}}));host.scrollTop=2300;host.dispatchEvent(new CustomEvent('novel-book-reveal-result',{detail:{requestId:request.requestId,ok:true}}));});
 const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='.book-workspace'?book:selector==='[aria-label="Book selection"]'?select:null};
 const controller=workspaceReadingController(()=>root as any,()=> 'book');
 await controller.restore({reading:{scrollTop:99999,bookSceneId:'folded-scene',bookSceneOffset:300,bookCharacterId:'hero'}});
 expect(requests).toEqual([{requestId:expect.any(String),sceneId:'folded-scene',offset:300,characterId:'hero'}]);expect(host.scrollTop).toBe(2300);
});
it('a rejected book reveal preserves its current passage and propagates the composition or save refusal',async()=>{
 const host=Object.assign(new EventTarget(),{scrollTop:80,dataset:{}}),select={value:'',options:[{value:''}]},book={dataset:{bookCharacter:''},inert:false};
 host.addEventListener('novel-book-reveal',event=>{host.dispatchEvent(new CustomEvent('novel-book-reveal-result',{detail:{requestId:(event as CustomEvent).detail.requestId,ok:false,error:'Finish entering the current word'}}));});
 const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='.book-workspace'?book:selector==='[aria-label="Book selection"]'?select:null};
 const controller=workspaceReadingController(()=>root as any,()=> 'book');
 await expect(controller.restore({reading:{scrollTop:600,bookSceneId:'scene',bookSceneOffset:600}})).rejects.toThrow('Finish entering');expect(host.scrollTop).toBe(80);
});

it('a timed-out book reveal cancels the exact request, drops its listener, and permits a later retry',async()=>{
 vi.useFakeTimers();
 try{
  const host=Object.assign(new EventTarget(),{scrollTop:80,dataset:{}}),select={value:'',options:[{value:''}]},book={dataset:{bookCharacter:''},inert:false},requests:any[]=[],cancelled:any[]=[];
  host.addEventListener('novel-book-reveal',event=>requests.push((event as CustomEvent).detail));host.addEventListener('novel-book-cancel-reveal',event=>cancelled.push((event as CustomEvent).detail));
  const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='.book-workspace'?book:selector==='[aria-label="Book selection"]'?select:null};
  const controller=workspaceReadingController(()=>root as any,()=> 'book'),bookmark={reading:{scrollTop:600,bookSceneId:'scene',bookSceneOffset:600}};
  const pending=controller.restore(bookmark),refusal=expect(pending).rejects.toThrow('cancelled');await vi.advanceTimersByTimeAsync(10000);await refusal;
  expect(cancelled).toEqual([{requestId:requests[0].requestId}]);expect(host.scrollTop).toBe(80);
  host.dispatchEvent(new CustomEvent('novel-book-reveal-result',{detail:{requestId:requests[0].requestId,ok:true}}));
  const retry=controller.restore(bookmark);await Promise.resolve();host.dispatchEvent(new CustomEvent('novel-book-reveal-result',{detail:{requestId:requests[1].requestId,ok:true}}));await expect(retry).resolves.toBeUndefined();
 }finally{vi.useRealTimers();}
});
it('disposing a reading controller aborts a pending reveal and refuses use after unmount',async()=>{
 const host=Object.assign(new EventTarget(),{scrollTop:80,dataset:{}}),select={value:'',options:[{value:''}]},book={dataset:{bookCharacter:''},inert:false},requests:any[]=[],cancelled:any[]=[];
 host.addEventListener('novel-book-reveal',event=>requests.push((event as CustomEvent).detail));host.addEventListener('novel-book-cancel-reveal',event=>cancelled.push((event as CustomEvent).detail));
 const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='.book-workspace'?book:selector==='[aria-label="Book selection"]'?select:null};
 const controller=workspaceReadingController(()=>root as any,()=> 'book'),pending=controller.restore({reading:{scrollTop:600,bookSceneId:'scene',bookSceneOffset:600}});
 const refusal=expect(pending).rejects.toThrow('cancelled');await Promise.resolve();controller.dispose();await refusal;
 expect(cancelled).toEqual([{requestId:requests[0].requestId}]);await expect(controller.capture()).rejects.toThrow('no longer active');
});

it('character restoration never fires an uncancellable ordinary selector change',async()=>{
 const host=Object.assign(new EventTarget(),{scrollTop:80,dataset:{}}),select={value:'',options:[{value:''},{value:'hero'}],dispatchEvent:vi.fn()},book={dataset:{bookCharacter:''},inert:false},requests:any[]=[],cancelled:any[]=[];
 host.addEventListener('novel-book-reveal',event=>requests.push((event as CustomEvent).detail));host.addEventListener('novel-book-cancel-reveal',event=>cancelled.push((event as CustomEvent).detail));
 const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='.book-workspace'?book:selector==='[aria-label="Book selection"]'?select:null};
 const controller=workspaceReadingController(()=>root as any,()=> 'book'),signal=new AbortController(),pending=controller.restore({reading:{scrollTop:600,bookSceneId:'hero-scene',bookSceneOffset:600,bookCharacterId:'hero'}},signal.signal);
 const refusal=expect(pending).rejects.toThrow('cancelled');await Promise.resolve();signal.abort();await refusal;
 expect(requests).toEqual([{requestId:expect.any(String),sceneId:'hero-scene',offset:600,characterId:'hero'}]);expect(cancelled).toEqual([{requestId:requests[0].requestId}]);expect(select.value).toBe('');expect(select.dispatchEvent).not.toHaveBeenCalled();expect(host.scrollTop).toBe(80);
});

it('an empty-cut bookmark still waits for a scoped character restoration acknowledgement',async()=>{
 const host=Object.assign(new EventTarget(),{scrollTop:80,dataset:{}}),select={value:'',options:[{value:''},{value:'empty'}]},requests:any[]=[];
 host.addEventListener('novel-book-reveal',event=>requests.push((event as CustomEvent).detail));
 const root={querySelector:(selector:string)=>selector==='.book-scroll'?host:selector==='[aria-label="Book selection"]'?select:null};
 const controller=workspaceReadingController(()=>root as any,()=> 'book');let complete=false;
 const pending=controller.restore({reading:{scrollTop:0,bookCharacterId:'empty'}}).then(()=>{complete=true;});await Promise.resolve();
 expect(complete).toBe(false);expect(requests).toEqual([{requestId:expect.any(String),offset:0,characterId:'empty'}]);expect(host.scrollTop).toBe(80);
 host.dispatchEvent(new CustomEvent('novel-book-reveal-result',{detail:{requestId:requests[0].requestId,ok:true}}));await pending;expect(complete).toBe(true);
});
