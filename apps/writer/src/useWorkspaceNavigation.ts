import {useEffect,useRef,type RefObject} from 'react';
import type {NavigationBookmark,NavigationController} from './navigation-trail';
import type {WorkspaceView} from './workspace-tabs';
import {BOOK_REVEAL_EVENT,BOOK_REVEAL_CANCEL_EVENT,BOOK_REVEAL_RESULT_EVENT} from './book-model';
interface Options {view:WorkspaceView;tabId?:string;rootRef:RefObject<HTMLElement|null>;onRegisterNavigation?:(controller:NavigationController|null)=>void}
function finiteOffset(value:number){if(!Number.isFinite(value)||value<0)throw new Error('Invalid reading position.');return value;}
export function workspaceReadingController(root:()=>HTMLElement|null,view:()=>WorkspaceView):NavigationController & {dispose():void} {
 const operations=new Set<AbortController>();let disposed=false;const initialView=view();
 const check=()=>{if(disposed||view()!==initialView)throw new Error('The reading workspace is no longer active.');};
 const element=()=>{const node=root();if(!node)throw new Error('The workspace is closed.');const book=node.querySelector<HTMLElement>('.book-scroll');if(book)return book;const main=node.querySelector<HTMLElement>('main');if(main&&['auto','scroll'].includes(getComputedStyle(main).overflowY))return main;return document.scrollingElement as HTMLElement??node;};
 return {
  async capture(){check();const host=element();return {reading:{scrollTop:host.scrollTop,...(view()==='book'?{bookSceneId:host.dataset.bookSceneId,bookSceneOffset:Number(host.dataset.bookSceneOffset)||0,bookCharacterId:root()?.querySelector<HTMLElement>('.book-workspace')?.dataset.bookCharacter}: {})}};},
  dispose(){disposed=true;for(const controller of operations)controller.abort(new Error('Reading restoration was cancelled.'));operations.clear();},
  async restore(bookmark:NavigationBookmark,signal?:AbortSignal){
   check();const controller=new AbortController(),abort=()=>controller.abort(signal?.reason);operations.add(controller);signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
   try{controller.signal.throwIfAborted();const scrollTop=finiteOffset(bookmark.reading.scrollTop);
   if(view()==='book'){
    const node=root();if(!node)throw new Error('The workspace is closed.');
    const select=node.querySelector<HTMLSelectElement>('[aria-label="Book selection"]'),character=bookmark.reading.bookCharacterId??'';
    if(!select||!Array.from(select.options).some(option=>option.value===character))throw new Error('The saved character cut is no longer available.');
    const sceneId=bookmark.reading.bookSceneId,offset=finiteOffset(bookmark.reading.bookSceneOffset??0);
    {
     const host=element(),requestId=crypto.randomUUID();
     await new Promise<void>((resolve,reject)=>{
      const finish=(error?:string)=>{clearTimeout(timer);controller.signal.removeEventListener('abort',cancel);host.removeEventListener(BOOK_REVEAL_RESULT_EVENT,result);if(error)reject(new Error(error));else resolve();};
      const result=(event:Event)=>{const detail=(event as CustomEvent).detail;if(detail?.requestId===requestId)finish(detail.ok?undefined:detail.error??'The saved reading location is no longer available.');};
      const cancel=()=>{host.dispatchEvent(new CustomEvent(BOOK_REVEAL_CANCEL_EVENT,{detail:{requestId}}));finish('Reading restoration was cancelled.');};
      const timer=setTimeout(cancel,10000);controller.signal.addEventListener('abort',cancel,{once:true});
      host.addEventListener(BOOK_REVEAL_RESULT_EVENT,result);
      if(controller.signal.aborted){cancel();return;}host.dispatchEvent(new CustomEvent(BOOK_REVEAL_EVENT,{detail:{requestId,...(sceneId?{sceneId}:{}),offset,characterId:character}}));
     });return;
    }
   }
   controller.signal.throwIfAborted();const host=element();host.scrollTop=scrollTop;host.dispatchEvent(new Event('scroll'));
   }finally{signal?.removeEventListener('abort',abort);operations.delete(controller);}
  }
 };
}
/** Book trails restore reading/filter only; ordinary Writer restores native selection. */
export function useWorkspaceNavigation({view,rootRef,onRegisterNavigation}:Options){
 const current=useRef(view);current.current=view;
 useEffect(()=>{if(view==='write'||!onRegisterNavigation)return;const controller=workspaceReadingController(()=>rootRef.current,()=>current.current);onRegisterNavigation(controller);return()=>{controller.dispose();onRegisterNavigation(null);};},[view,rootRef,onRegisterNavigation]);
}
