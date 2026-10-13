import React, { memo,useEffect,useLayoutEffect,useMemo,useRef,useState } from 'react';
import type { Chapter, SceneSummary } from './workspace';
import {SCENE_DRAG_TYPE,tabTitle,type PaneSide} from './workspace-tabs';
import type { SceneSnapshot } from './assistance/model';

export interface SceneIndex {
  byId:ReadonlyMap<string,SceneSummary>;
  byChapter:ReadonlyMap<string,readonly SceneSummary[]>;
}
/** Index identities once and sort copied chapter groups without touching manuscript order. */
export function indexScenes(scenes:readonly SceneSummary[]):SceneIndex {
  const byId=new Map<string,SceneSummary>(),byChapter=new Map<string,SceneSummary[]>();
  for(const scene of scenes){byId.set(scene.id,scene);const group=byChapter.get(scene.chapterId);if(group)group.push(scene);else byChapter.set(scene.chapterId,[scene]);}
  for(const group of byChapter.values())group.sort((a,b)=>a.position-b.position);
  return {byId,byChapter};
}
/** Async source refreshes may finish after a project/head changed. Only current heads participate. */
export function projectReviewSources(index:SceneIndex,sources:readonly SceneSnapshot[],projectId:string,factSource:SceneSnapshot|null,planningRevision:string|null):SceneSnapshot[] {
  const current=sources.filter(source=>index.byId.get(source.sceneId)?.revisionId===source.revisionId);
  if(factSource&&planningRevision&&factSource.sceneId==='planning:'+projectId&&factSource.revisionId===planningRevision)current.push(factSource);
  return current;
}
export function filterManuscript(chapters:readonly Chapter[],index:SceneIndex,query:string):{chapter:Chapter;scenes:readonly SceneSummary[]}[] {
  const term=query.slice(0,1000).trim().toLocaleLowerCase();
  const result:{chapter:Chapter;scenes:readonly SceneSummary[]}[]=[];
  for(const chapter of chapters){
    const scenes=index.byChapter.get(chapter.id)??[];
    const matches=!term||chapter.title.toLocaleLowerCase().includes(term)?scenes:scenes.filter(scene=>(scene.title||'Untitled scene').toLocaleLowerCase().includes(term));
    if(!term||matches.length||chapter.title.toLocaleLowerCase().includes(term))result.push({chapter,scenes:matches});
  }
  return result;
}
/** Reveal only inside the binder; never move the writing viewport or outer page. */
export function revealManuscriptRow(scroller:HTMLElement,row:HTMLElement):void {
  if(scroller.clientHeight<=0)return;
  const bounds=scroller.getBoundingClientRect(),item=row.getBoundingClientRect();
  const top=bounds.top+scroller.clientTop,bottom=top+scroller.clientHeight;
  if(item.top<top)scroller.scrollTop-=top-item.top;
  else if(item.bottom>bottom)scroller.scrollTop+=item.bottom-bottom;
}
export const ManuscriptTree=memo(function ManuscriptTree({chapterNumberStart=1,chapters,index,activeId,onOpenScene,onOpenBeside,onOpenInPane,selectedChapterId,onSelectChapter,onEditChapter,searchable=true}:{chapterNumberStart?:number;chapters:readonly Chapter[];index:SceneIndex;activeId:string|null;onOpenScene:(id:string)=>void;onOpenBeside?:(id:string)=>void;onOpenInPane?:(id:string,side:PaneSide)=>void;selectedChapterId?:string|null;onSelectChapter?:(id:string)=>void;onEditChapter?:(chapter:Chapter)=>void;searchable?:boolean}) {
  const [query,setQuery]=useState('');
  const groups=useMemo(()=>filterManuscript(chapters,index,query),[chapters,index,query]);
  const scroller=useRef<HTMLDivElement>(null),search=useRef<HTMLInputElement>(null),rows=useRef(new Map<string,HTMLButtonElement>()),chapterRows=useRef(new Map<string,HTMLElement>()),lastActive=useRef(activeId),lastChapter=useRef(selectedChapterId),revealChapter=useRef<string|null>(null);
  const [menu,setMenu]=useState<{id:string;x:number;y:number}|null>(null),menuElement=useRef<HTMLDivElement>(null),origin=useRef<HTMLButtonElement|null>(null);
  useEffect(()=>{if(!menu)return;menuElement.current?.querySelector<HTMLButtonElement>('button')?.focus();const close=(event:PointerEvent)=>{if(!menuElement.current?.contains(event.target as Node))setMenu(null);};window.addEventListener('pointerdown',close);return()=>window.removeEventListener('pointerdown',close);},[menu]);
  useLayoutEffect(()=>{
    const changed=lastActive.current!==activeId,chapterChanged=lastChapter.current!==selectedChapterId;
    lastActive.current=activeId;lastChapter.current=selectedChapterId;
    if(chapterChanged)revealChapter.current=selectedChapterId??null;
    if((changed&&activeId&&!rows.current.has(activeId))||(chapterChanged&&selectedChapterId&&!chapterRows.current.has(selectedChapterId))){if(query)setQuery('');return;}
    const row=revealChapter.current?chapterRows.current.get(revealChapter.current):activeId?rows.current.get(activeId):undefined;
    if(!row||!scroller.current)return;
    const chapter=row.closest('details');if(chapter&&row.tagName==='BUTTON')chapter.open=true;
    revealManuscriptRow(scroller.current,row);revealChapter.current=null;
  },[activeId,selectedChapterId,groups]);
  function show(event:React.MouseEvent<HTMLButtonElement>|React.KeyboardEvent<HTMLButtonElement>,id:string){event.preventDefault();origin.current=event.currentTarget;const bounds=event.currentTarget.getBoundingClientRect();setMenu({id,x:Math.max(8,Math.min('clientX'in event?event.clientX:bounds.left,window.innerWidth-220)),y:Math.max(8,Math.min('clientY'in event?event.clientY:bounds.bottom,window.innerHeight-110))});}
  function close(){setMenu(null);origin.current?.focus({preventScroll:true});}
  return <div className="manuscript-tree" role="navigation" aria-label="Chapters and scenes">
    <h2 className="binder-heading">Manuscript</h2>
    {searchable&&<div className="manuscript-search"><label>Find scene<input ref={search} type="search" aria-label="Find scene" maxLength={1000} value={query} placeholder="Scene or chapter" onChange={event=>setQuery(event.target.value.slice(0,1000))} onKeyDown={event=>{if(event.key==='Escape'&&query){event.preventDefault();setQuery('');}}}/></label>{query&&<button aria-label="Clear scene filter" onClick={()=>{setQuery('');search.current?.focus({preventScroll:true});}}>Clear</button>}</div>}
    <div className="manuscript-tree-scroll" ref={scroller}>
      {!groups.length&&<p role="status">{query?'No matching scenes or chapters.':'No chapters yet. Create a chapter or scene to begin.'}</p>}
      {groups.map(({chapter,scenes})=><details open className="binder-chapter" key={chapter.id}><summary ref={node=>{if(node)chapterRows.current.set(chapter.id,node);else chapterRows.current.delete(chapter.id);}} aria-current={selectedChapterId===chapter.id?'true':undefined} onClick={()=>onSelectChapter?.(chapter.id)}><span className="chapter-number" aria-label={'Chapter number '+(chapter.position+chapterNumberStart)}>{chapter.position+chapterNumberStart}.</span><span className="chapter-title" dir="auto">{chapter.title}</span><small title={scenes.length+' scenes'}>{scenes.length}</small>{onEditChapter&&<button type="button" className="chapter-edit" aria-label={'Edit chapter '+chapter.title} title="Edit chapter name and number" onClick={event=>{event.preventDefault();event.stopPropagation();onEditChapter(chapter);}}>Edit</button>}</summary>{scenes.map(scene=><button ref={node=>{if(node)rows.current.set(scene.id,node);else rows.current.delete(scene.id);}} className={activeId===scene.id?'scene-active':''} aria-current={activeId===scene.id?'page':undefined} key={scene.id} dir="auto" draggable onDragStart={event=>{setMenu(null);event.dataTransfer.effectAllowed='copy';event.dataTransfer.setData(SCENE_DRAG_TYPE,JSON.stringify({projectId:chapter.projectId,sceneId:scene.id,title:tabTitle(scene.title)}));}} onClick={()=>{onSelectChapter?.(chapter.id);onOpenScene(scene.id);}} onContextMenu={event=>show(event,scene.id)} onKeyDown={event=>{if(event.key==='ContextMenu'||event.shiftKey&&event.code==='F10')show(event,scene.id);}}>{scene.title||'Untitled scene'}</button>)}{!scenes.length&&<p className="binder-empty">No scenes in this chapter.</p>}</details>)}
    </div>
    {menu&&<div ref={menuElement} className="binder-context-menu" role="menu" aria-label="Scene actions" style={{left:menu.x,top:menu.y}} onKeyDown={event=>{if(event.key==='Escape'){event.preventDefault();close();}}}><button role="menuitem" onClick={()=>{close();onOpenScene(menu.id);}}>Open draft</button>{onOpenBeside&&<button role="menuitem" onClick={()=>{close();onOpenBeside(menu.id);}}>Open beside</button>}{onOpenInPane&&<><button role="menuitem" onClick={()=>{close();onOpenInPane(menu.id,'left');}}>Open on left</button><button role="menuitem" onClick={()=>{close();onOpenInPane(menu.id,'right');}}>Open on right</button></>}</div>}
  </div>;
});
