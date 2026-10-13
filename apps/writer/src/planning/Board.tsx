import React, { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { newId, removeBoardCard, type BoardCard, type BoardEdge, type StoryBoard, type PlotPoint } from './model';
import { Field, LinkedRecords, LinkPicker, type PanelProps } from './controls';
import { BOARD_PAGE_SIZE, boardBounds, connectionPageChoices, filterBoard, indexBoard, listNavigationTarget, moveCardForKey, navigationTarget, pageItems, visibleBoardCards, visibleBoardEdges, type CardPresentation } from './board-view';

export interface BoardProps extends PanelProps { initialView?:'freeform'|'list' }
export function Board(props:BoardProps) {
  const {document:doc,change,scenes}=props;
  const [boardId,setBoardId]=useState(doc.boards[0]?.id??''), [selected,setSelected]=useState('');
  const [zoom,setZoom]=useState(1), [filter,setFilter]=useState(''), [list,setList]=useState(props.initialView==='list');
  const [groupFilter,setGroupFilter]=useState(''), [listPage,setListPage]=useState(0), [edgePage,setEdgePage]=useState(0);
  const [edgeFrom,setEdgeFrom]=useState(''), [edgeTo,setEdgeTo]=useState(''), [edgeKind,setEdgeKind]=useState<BoardEdge['kind']>('reference');
  const [edgeLabel,setEdgeLabel]=useState(''), [groupName,setGroupName]=useState(''), [connectionsOpen,setConnectionsOpen]=useState(false);
  const [revealTick,setRevealTick]=useState(0), [viewport,setViewport]=useState({left:0,top:0,width:900,height:510});
  const [drag,setDrag]=useState<{id:string;x:number;y:number}|null>(null);
  const origin=useRef<{id:string;x:number;y:number;clientX:number;clientY:number;pointerId:number}|null>(null);
  const dragPosition=useRef<typeof drag>(null), pane=useRef<HTMLDivElement>(null);
  const buttons=useRef(new Map<string,HTMLButtonElement>()), reveal=useRef<{id:string;focus:boolean}|null>(null);
  const helpId=useId();
  const board=doc.boards.find(b=>b.id===boardId)??doc.boards[0];
  const index=useMemo(()=>board?indexBoard(doc,scenes,board):new Map<string,CardPresentation>(),[doc,scenes,board]);
  const cards=useMemo(()=>filterBoard(index,filter,groupFilter),[index,filter,groupFilter]);
  const page=pageItems(cards,listPage), edges=pageItems(board?.edges??[],edgePage);
  const card=index.get(selected)?.card as BoardCard|undefined;
  const point=doc.plotPoints.find(p=>card?.reference?.kind==='plot-point'&&p.id===card.reference.id);
  const cardTitle=(c:BoardCard)=>index.get(c.id)?.title||'Untitled card';
  const update=(patch:Partial<StoryBoard>)=>board&&change({...doc,boards:doc.boards.map(b=>b.id===board.id?{...b,...patch}:b)});
  const updatePoint=(patch:Partial<PlotPoint>)=>point&&change({...doc,plotPoints:doc.plotPoints.map(p=>p.id===point.id?{...p,...patch}:p)});
  const updateCard=(id:string,patch:Partial<BoardCard>)=>{if(!board)return;if(patch.x!==undefined||patch.y!==undefined)reveal.current={id,focus:false};update({cards:board.cards.map(c=>c.id===id?{...c,...patch}:c)});};
  const position=(c:BoardCard)=>drag?.id===c.id?drag:c;
  const visibleCards=list?page.items:visibleBoardCards(cards,viewport,zoom,[selected,...(drag?[drag.id]:[])],drag);
  const visibleEdges=useMemo(()=>visibleBoardEdges(board?.edges??[],cards,visibleCards),[board?.edges,cards,visibleCards]);
  const bounds=useMemo(()=>boardBounds(cards,drag),[cards,drag]);
  const selectedIndex=cards.findIndex(c=>c.id===selected);
  const measure=()=>{const el=pane.current;if(!el)return;const next={left:el.scrollLeft,top:el.scrollTop,width:el.clientWidth,height:el.clientHeight};setViewport(old=>Object.keys(next).every(k=>old[k as keyof typeof old]===next[k as keyof typeof next])?old:next);};
  const choose=(id:string,focus=false)=>{reveal.current={id,focus};setSelected(id);setListPage(Math.floor(Math.max(0,cards.findIndex(c=>c.id===id))/BOARD_PAGE_SIZE));setRevealTick(t=>t+1);};
  const navigate=(key:string)=>{const id=list?listNavigationTarget(cards,selected,listPage,key):navigationTarget(cards,selected,key);if(id)choose(id,true);return !!id;};
  const resetSelection=(resetConnections=false)=>{reveal.current=null;origin.current=null;dragPosition.current=null;setDrag(null);setSelected('');setListPage(0);if(resetConnections){setEdgePage(0);setEdgeFrom('');setEdgeTo('');}};
  useEffect(()=>{const el=pane.current;if(!el)return;measure();if(typeof ResizeObserver==='undefined')return;const observer=new ResizeObserver(measure);observer.observe(el);return()=>observer.disconnect();},[board?.id,list]);
  useLayoutEffect(()=>{
    const request=reveal.current,el=pane.current;if(!request||!el)return;
    const target=index.get(request.id)?.card as BoardCard|undefined,button=buttons.current.get(request.id);if(!target||!button)return;
    if(list){const rect=button.closest('article')!.getBoundingClientRect(),port=el.getBoundingClientRect();if(rect.top<port.top)el.scrollTop+=rect.top-port.top;else if(rect.bottom>port.bottom)el.scrollTop+=rect.bottom-port.bottom;}
    else {const x=target.x*zoom,y=target.y*zoom,w=220*zoom,h=260*zoom;if(x<el.scrollLeft||x+w>el.scrollLeft+el.clientWidth)el.scrollLeft=Math.max(0,x-(el.clientWidth-w)/2);if(y<el.scrollTop||y+h>el.scrollTop+el.clientHeight)el.scrollTop=Math.max(0,y-(el.clientHeight-h)/2);}
    if(request.focus)button.focus({preventScroll:true});reveal.current=null;measure();
  },[selected,card?.x,card?.y,list,page.page,zoom,revealTick]);
  const addCard=(kind:BoardCard['kind'])=>{if(!board)return;const id=newId(),plotPointId=kind==='plot-point'?newId():null;
    const next:BoardCard={id,title:kind==='plot-point'?'New plot point':'New note',body:'',kind,reference:plotPointId?{kind:'plot-point',id:plotPointId}:null,x:60+(board.cards.length%5)*260,y:60+Math.floor(board.cards.length/5)*170,groupId:groupFilter||null,labels:[]};
    change({...doc,plotPoints:plotPointId?[...doc.plotPoints,{id:plotPointId,title:'New plot point',notes:'',links:[],requiresPayoff:false,scopeClosed:false,payoffId:null}]:doc.plotPoints,boards:doc.boards.map(b=>b.id===board.id?{...b,cards:[...b.cards,next]}:b)});
    setFilter('');setSelected(id);setListPage(Math.floor(board.cards.filter(c=>!groupFilter||c.groupId===groupFilter).length/BOARD_PAGE_SIZE));reveal.current={id,focus:true};setRevealTick(t=>t+1);
  };
  const finishDrag=(cancel=false)=>{const pos=dragPosition.current;if(!cancel&&pos)updateCard(pos.id,{x:pos.x,y:pos.y});setDrag(null);dragPosition.current=null;origin.current=null;};
  const pageList=(next:number)=>{reveal.current=null;if(!pageItems(cards,next).items.some(c=>c.id===selected))setSelected('');setListPage(next);pane.current?.focus();if(pane.current)pane.current.scrollTop=0;};
  return <div>
    <div className="planning-toolbar">
      <select aria-label="Board" value={board?.id??''} onChange={e=>{resetSelection(true);setBoardId(e.target.value);setFilter('');setGroupFilter('');if(pane.current)pane.current.scrollTop=pane.current.scrollLeft=0;}}>{doc.boards.map(b=><option key={b.id} value={b.id}>{b.name}</option>)}</select>
      <button onClick={()=>{const id=newId();resetSelection(true);setFilter('');setGroupFilter('');change({...doc,boards:[...doc.boards,{id,name:'New board',cards:[],groups:[],edges:[]}]});setBoardId(id);}}>＋ Board</button>
      <button onClick={()=>addCard('plot-point')}>＋ Plot point</button><button onClick={()=>addCard('note')}>＋ Note / linked card</button>
      <button aria-pressed={list} onClick={()=>{if(selected)reveal.current={id:selected,focus:false};setList(!list);}}>{list?'Freeform view':'List view'}</button>
    </div>
    {board&&<>
      <div className="planning-toolbar">
        <input aria-label="Board name" value={board.name} onChange={e=>update({name:e.target.value})}/>
        <input aria-label="Filter board cards" placeholder="Filter cards or labels…" value={filter} onChange={e=>{resetSelection();setFilter(e.target.value);}}/>
        <select aria-label="Filter board group" value={groupFilter} onChange={e=>{resetSelection();setGroupFilter(e.target.value);}}><option value="">All groups</option>{board.groups.map(g=><option key={g.id} value={g.id}>{g.name}</option>)}</select>
        <Field label={`Zoom ${Math.round(zoom*100)}%`}><input type="range" min="0.4" max="1.5" step="0.1" value={zoom} onChange={e=>{if(selected)reveal.current={id:selected,focus:false};setZoom(Number(e.target.value));}}/></Field>
      </div>
      <div className="planning-toolbar planning-board-navigation" aria-label="Card navigation">
        <button disabled={!cards.length||selectedIndex===0} onClick={()=>navigate('ArrowUp')}>Previous card</button>
        <button disabled={!cards.length||selectedIndex===cards.length-1} onClick={()=>navigate('ArrowDown')}>Next card</button>
        <button disabled={selectedIndex<0} onClick={()=>choose(selected,true)}>Show selected card</button>
        <span role="status">{selectedIndex<0?`${cards.length} cards`:`Card ${selectedIndex+1} of ${cards.length}`}</span>
      </div>
      <details className="planning-help"><summary>Board controls</summary><p id={helpId}>On a card, arrows select the next or previous card; Home / End go to the first or last. Page Up / Down skip 50 cards. On a handle, arrows move the card; Shift moves farther. Drag a handle to move, Escape to cancel. Scroll to pan.</p></details>
      <div className="planning-board-layout">
        <div ref={pane} className={list?'planning-board-list':'planning-board-viewport'} tabIndex={0} role="region" aria-label="Board cards" aria-describedby={helpId} onScroll={measure} onKeyDown={e=>{if(e.defaultPrevented||e.nativeEvent.isComposing||e.altKey||e.ctrlKey||e.metaKey||e.shiftKey)return;if(e.target!==e.currentTarget&&!(e.target instanceof Element&&e.target.closest('.planning-card-content')))return;if(navigate(e.key))e.preventDefault();}}>
          <div className={list?'planning-list-page':'planning-canvas'} style={list?{}:{width:bounds.width*zoom,height:bounds.height*zoom}}>
            {!list&&<svg className="planning-connectors" aria-hidden="true" width={viewport.width} height={viewport.height} style={{left:viewport.left,top:viewport.top}}>{visibleEdges.map(e=>{const from=position(index.get(e.fromId)!.card),to=position(index.get(e.toId)!.card),x1=(from.x+110)*zoom-viewport.left,y1=(from.y+55)*zoom-viewport.top,x2=(to.x+110)*zoom-viewport.left,y2=(to.y+55)*zoom-viewport.top;return <g key={e.id}><line x1={x1} y1={y1} x2={x2} y2={y2}/><text x={(x1+x2)/2} y={(y1+y2)/2-6}>{e.label||e.kind} →</text></g>;})}</svg>}
            {visibleCards.map(c=>{const pos=position(c);return <article className={`planning-board-card ${selected===c.id?'selected':''}`} key={c.id} style={list?{}:{left:pos.x*zoom,top:pos.y*zoom,width:220*zoom,maxHeight:260*zoom,fontSize:13*zoom}}>
              <button className="planning-drag-handle" aria-label={`Move ${cardTitle(c)}`} aria-describedby={helpId} onFocus={()=>setSelected(c.id)} onClick={()=>choose(c.id)} onKeyDown={e=>{if(e.key==='Escape'&&origin.current){e.preventDefault();finishDrag(true);return;}if(e.nativeEvent.isComposing||e.altKey||e.ctrlKey||e.metaKey)return;const patch=moveCardForKey(c,e.key,e.shiftKey);if(patch){e.preventDefault();updateCard(c.id,patch);}else if(['Home','End','PageUp','PageDown'].includes(e.key)&&navigate(e.key))e.preventDefault();}}
                onPointerDown={e=>{if(list||e.button!==0||origin.current)return;e.currentTarget.setPointerCapture(e.pointerId);setSelected(c.id);origin.current={id:c.id,x:c.x,y:c.y,clientX:e.clientX,clientY:e.clientY,pointerId:e.pointerId};dragPosition.current={id:c.id,x:c.x,y:c.y};setDrag(dragPosition.current);}}
                onPointerMove={e=>{const start=origin.current;if(start?.id!==c.id||start.pointerId!==e.pointerId)return;dragPosition.current={id:c.id,x:Math.max(0,start.x+(e.clientX-start.clientX)/zoom),y:Math.max(0,start.y+(e.clientY-start.clientY)/zoom)};setDrag(dragPosition.current);}}
                onPointerUp={e=>{if(origin.current?.pointerId===e.pointerId)finishDrag();}} onPointerCancel={()=>finishDrag(true)} onLostPointerCapture={()=>finishDrag(true)}>
                ⠿ {c.kind}{c.groupId?` · ${board.groups.find(g=>g.id===c.groupId)?.name}`:''}
              </button>
              <button ref={el=>{if(el)buttons.current.set(c.id,el);else buttons.current.delete(c.id);}} className="planning-card-content" aria-label={`Select ${cardTitle(c)}`} aria-pressed={selected===c.id} aria-describedby={helpId} onFocus={()=>setSelected(c.id)} onClick={()=>choose(c.id)}>
                <strong dir="auto">{cardTitle(c)}</strong>{c.reference&&<small>{index.get(c.id)!.referenceLabel}</small>}<p dir="auto">{index.get(c.id)!.body.slice(0,160)}</p><small>{c.labels.join(' · ')}</small>
              </button>
            </article>;})}
            {!cards.length&&<p className="planning-empty">{filter||groupFilter?'No matching cards.':'No cards.'}{(filter||groupFilter)&&<button onClick={()=>{setFilter('');setGroupFilter('');}}>Clear filters</button>}</p>}
          </div>
        </div>
<aside className="planning-board-inspector" aria-label="Card details">{card ? <><h3>Card</h3><Field label="Title"><input dir="auto" value={point?.title ?? card.title} onChange={e => point ? updatePoint({title:e.target.value}) : updateCard(card.id,{title:e.target.value})}/></Field><Field label="Notes"><textarea rows={4} dir="auto" value={point?.notes ?? card.body} onChange={e => point ? updatePoint({notes:e.target.value}) : updateCard(card.id,{body:e.target.value})}/></Field><Field label="Linked record or scene"><LinkPicker document={doc} scenes={scenes} value={card.reference} onChange={reference => updateCard(card.id,{reference,kind:reference?.kind ?? 'note'})}/></Field>{card.reference?.kind==='scene' && props.onOpenScene && <button onClick={()=>props.onOpenScene!(card.reference!.id)}>Open linked scene</button>}{point&&<><h3>Links</h3><LinkedRecords {...props} links={point.links} onChange={links=>updatePoint({links})}/><label><input type="checkbox" checked={point.requiresPayoff} onChange={e=>updatePoint({requiresPayoff:e.target.checked})}/> This setup requires a payoff</label><label><input type="checkbox" checked={point.scopeClosed} onChange={e=>updatePoint({scopeClosed:e.target.checked})}/> Review scope is closed</label><Field label="Payoff"><select value={point.payoffId??''} onChange={e=>updatePoint({payoffId:e.target.value||null})}><option value="">No payoff linked</option>{doc.plotPoints.filter(p=>p.id!==point.id).map(p=><option key={p.id} value={p.id}>{p.title}</option>)}</select></Field></>}<Field label="Labels (one per line)"><textarea value={card.labels.join('\n')} onChange={e=>updateCard(card.id,{labels:e.target.value.split('\n')})}/></Field><Field label="Group"><select value={card.groupId??''} onChange={e=>updateCard(card.id,{groupId:e.target.value||null})}><option value="">No group</option>{board.groups.map(g=><option key={g.id} value={g.id}>{g.name}</option>)}</select></Field><div className="planning-fields"><Field label="X"><input type="number" min="0" value={card.x} onChange={e=>{if(e.target.value!=='')updateCard(card.id,{x:Math.max(0,Number(e.target.value))});}}/></Field><Field label="Y"><input type="number" min="0" value={card.y} onChange={e=>{if(e.target.value!=='')updateCard(card.id,{y:Math.max(0,Number(e.target.value))});}}/></Field></div><button onClick={()=>{change(removeBoardCard(doc,board.id,card.id));setSelected('');pane.current?.focus();}}>Remove card from board</button><p className="planning-muted">Linked records are kept.</p></> : <p className="planning-muted">Select a card to edit.</p>}<h3>Groups</h3>{board.groups.map(g=><div className="planning-toolbar" key={g.id}><input aria-label="Group name" value={g.name} onChange={e=>update({groups:board.groups.map(item=>item.id===g.id?{...item,name:e.target.value}:item)})}/><button aria-label={`Ungroup ${g.name}`} onClick={()=>update({groups:board.groups.filter(item=>item.id!==g.id),cards:board.cards.map(c=>c.groupId===g.id?{...c,groupId:null}:c)})}>×</button></div>)}<input aria-label="New group name" placeholder="New group" value={groupName} onChange={e=>setGroupName(e.target.value)}/><button disabled={!groupName.trim()} onClick={()=>{update({groups:[...board.groups,{id:newId(),name:groupName.trim()}]});setGroupName('');}}>Add group</button></aside>
      </div>
      {list&&<div className="planning-toolbar planning-board-navigation" aria-label="Card list pages"><button disabled={page.page===0} onClick={()=>pageList(page.page-1)}>Previous page</button><span>{cards.length?`${page.start+1}–${page.end} of ${cards.length}`:'0 cards'}</span><button disabled={page.page===page.pages-1} onClick={()=>pageList(page.page+1)}>Next page</button></div>}
      <details className="planning-box planning-connections" open={connectionsOpen} onToggle={e=>setConnectionsOpen(e.currentTarget.open)}>
        <summary>Connections ({board.edges.length})</summary>
        {connectionsOpen&&<><p className="planning-muted">Set time constraints in Chronology.</p><div className="planning-toolbar">
          <ConnectionEndpoint key={`${board.id}:from`} label="Connection from" index={index} value={edgeFrom} onChange={setEdgeFrom}/>
          <ConnectionEndpoint key={`${board.id}:to`} label="Connection to" index={index} value={edgeTo} onChange={setEdgeTo}/>
          <select aria-label="Connection type" value={edgeKind} onChange={e=>setEdgeKind(e.target.value as BoardEdge['kind'])}>{['reference','causes','happens-before','reveals','depends-on','setup/payoff','custom'].map(kind=><option key={kind}>{kind}</option>)}</select>
          <input aria-label="Connection label" placeholder="Optional label" value={edgeLabel} onChange={e=>setEdgeLabel(e.target.value)}/>
          <button disabled={!index.has(edgeFrom)||!index.has(edgeTo)||edgeFrom===edgeTo} onClick={()=>update({edges:[...board.edges,{id:newId(),fromId:edgeFrom,toId:edgeTo,kind:edgeKind,label:edgeLabel,confirmed:false}]})}>Connect</button>
        </div>
        {edges.items.map(e=><div className="planning-connection" key={e.id}><span>{index.get(e.fromId)?.title||'Untitled card'} → <b>{e.label||e.kind}</b> → {index.get(e.toId)?.title||'Untitled card'}</span>{e.kind==='depends-on'&&<label><input type="checkbox" checked={e.confirmed??false} onChange={event=>update({edges:board.edges.map(edge=>edge.id===e.id?{...edge,confirmed:event.target.checked}:edge)})}/> Confirm prerequisite (from event depends on to event)</label>}<button onClick={()=>update({edges:board.edges.filter(edge=>edge.id!==e.id)})} aria-label={`Remove connection from ${index.get(e.fromId)?.title||'Untitled card'} to ${index.get(e.toId)?.title||'Untitled card'}`}>×</button></div>)}
        <div className="planning-toolbar planning-board-navigation" aria-label="Connection pages"><button disabled={edges.page===0} onClick={()=>setEdgePage(edges.page-1)}>Previous connections</button><span>{board.edges.length?`${edges.start+1}–${edges.end} of ${board.edges.length}`:'No connections'}</span><button disabled={edges.page===edges.pages-1} onClick={()=>setEdgePage(edges.page+1)}>Next connections</button></div></>}
      </details>
    </>}
  </div>;
}

export function ConnectionEndpoint({label,index,value,onChange}:{label:string;index:Map<string,CardPresentation>;value:string;onChange:(id:string)=>void}) {
  const [query,setQuery]=useState(''),[pageNumber,setPageNumber]=useState(0);
  const page=useMemo(()=>connectionPageChoices(index,query,value,pageNumber),[index,query,value,pageNumber]);
  return <div className="planning-connection-endpoint">
    <input aria-label={`${label} filter`} placeholder="Find card…" value={query} onChange={e=>{setQuery(e.target.value);setPageNumber(0);}}/>
    <select aria-label={label} value={value} onChange={e=>onChange(e.target.value)}><option value="">Choose card…</option>{page.choices.map(c=><option key={c.id} value={c.id}>{c.label}</option>)}</select>
    <div className="planning-toolbar planning-board-navigation"><button aria-label={`Previous ${label.toLocaleLowerCase()} choices`} disabled={page.page===0} onClick={()=>setPageNumber(page.page-1)}>Previous</button><span>{page.end?`${page.start+1}–${page.end}`:'0'} of {page.total}</span><button aria-label={`Next ${label.toLocaleLowerCase()} choices`} disabled={page.page===page.pages-1} onClick={()=>setPageNumber(page.page+1)}>Next</button></div>
  </div>;
}
