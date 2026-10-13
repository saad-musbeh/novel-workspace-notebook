import type { BoardCard, BoardEdge, PlanningDocument, StoryBoard } from './model';

export const BOARD_PAGE_SIZE = 50;
export const CARD_WIDTH = 220;
export const CARD_HEIGHT = 260;
export interface BoardViewport { left:number; top:number; width:number; height:number }
export interface CardPresentation { card:BoardCard; title:string; body:string; referenceLabel:string; search:string }

/** Resolve canonical labels once, instead of scanning story records for every edge or pan. */
export function indexBoard(doc:PlanningDocument, scenes:{id:string;title:string}[], board:StoryBoard):Map<string,CardPresentation> {
  const points=new Map(doc.plotPoints.map(p=>[p.id,p]));
  const labels=new Map<string,string>([
    ...doc.records.map(r=>[`record:${r.id}`,r.name] as [string,string]),
    ...doc.events.map(e=>[`event:${e.id}`,e.title] as [string,string]),
    ...doc.plotPoints.map(p=>[`plot-point:${p.id}`,p.title] as [string,string]),
    ...scenes.map(s=>[`scene:${s.id}`,s.title] as [string,string]),
  ]);
  return new Map(board.cards.map(card=>{
    const point=card.reference?.kind==='plot-point'?points.get(card.reference.id):undefined;
    const title=point?.title??card.title,body=point?.notes??card.body;
    const referenceLabel=card.reference?(labels.get(`${card.reference.kind}:${card.reference.id}`)??`Missing ${card.reference.kind} (${card.reference.id})`):'';
    return [card.id,{card,title,body,referenceLabel,search:`${title} ${body} ${card.labels.join(' ')} ${referenceLabel}`.toLocaleLowerCase()}];
  }));
}
export function filterBoard(index:Map<string,CardPresentation>, query:string, groupId:string):BoardCard[] {
  const text=query.toLocaleLowerCase();
  return [...index.values()].filter(p=>(!groupId||p.card.groupId===groupId)&&p.search.includes(text)).map(p=>p.card);
}
export function pageItems<T>(items:readonly T[], requested:number, size=BOARD_PAGE_SIZE) {
  const pageSize=Number.isFinite(size)?Math.max(1,Math.floor(size)):BOARD_PAGE_SIZE;
  const pages=Math.max(1,Math.ceil(items.length/pageSize));
  const page=Math.max(0,Math.min(pages-1,Number.isFinite(requested)?Math.floor(requested):0));
  const start=page*pageSize;
  return {items:items.slice(start,start+pageSize),page,pages,start,end:Math.min(items.length,start+pageSize)};
}
export function navigationTarget(cards:readonly BoardCard[], selected:string, key:string):string|null {
  if(!cards.length)return null;
  const current=cards.findIndex(c=>c.id===selected);
  let index=current;
  if(key==='Home')index=0;
  else if(key==='End')index=cards.length-1;
  else if(['ArrowDown','ArrowRight'].includes(key))index=current<0?0:current+1;
  else if(['ArrowUp','ArrowLeft'].includes(key))index=current<0?0:current-1;
  else if(key==='PageDown')index=current<0?0:current+BOARD_PAGE_SIZE;
  else if(key==='PageUp')index=current<0?0:current-BOARD_PAGE_SIZE;
  else return null;
  return cards[Math.max(0,Math.min(cards.length-1,index))].id;
}
/** After explicit pagination, arrows begin on that page rather than an old hidden selection. */
export function listNavigationTarget(cards:readonly BoardCard[],selected:string,pageNumber:number,key:string):string|null {
  if(cards.some(c=>c.id===selected))return navigationTarget(cards,selected,key);
  const first=pageItems(cards,pageNumber).items[0]?.id;
  if(['ArrowDown','ArrowRight','ArrowUp','ArrowLeft'].includes(key))return first??null;
  return navigationTarget(cards,first??'',key);
}
export function moveCardForKey(card:BoardCard,key:string,farther=false):{x:number;y:number}|null {
  const distance=farther?50:10;
  const directions:Record<string,[number,number]>={ArrowLeft:[-distance,0],ArrowRight:[distance,0],ArrowUp:[0,-distance],ArrowDown:[0,distance]};
  const delta=directions[key];
  return delta?{x:Math.max(0,card.x+delta[0]),y:Math.max(0,card.y+delta[1])}:null;
}
export function visibleBoardCards(cards:readonly BoardCard[],viewport:BoardViewport,zoom:number,retained:string[],drag:{id:string;x:number;y:number}|null):BoardCard[] {
  const keep=new Set(retained);
  return cards.filter(card=>{
    if(keep.has(card.id))return true;
    const p=drag?.id===card.id?drag:card;
    return p.x*zoom<viewport.left+viewport.width+300&&(p.x+CARD_WIDTH)*zoom>viewport.left-300&&p.y*zoom<viewport.top+viewport.height+300&&(p.y+CARD_HEIGHT)*zoom>viewport.top-300;
  });
}
export function visibleBoardEdges(edges:readonly BoardEdge[],filtered:readonly BoardCard[],visible:readonly BoardCard[]):BoardEdge[] {
  const allowed=new Set(filtered.map(c=>c.id)),painted=new Set(visible.map(c=>c.id));
  return edges.filter(e=>allowed.has(e.fromId)&&allowed.has(e.toId)&&(painted.has(e.fromId)||painted.has(e.toId)));
}
export function boardBounds(cards:readonly BoardCard[],drag:{id:string;x:number;y:number}|null) {
  return cards.reduce((bounds,card)=>{const p=drag?.id===card.id?drag:card;return {width:Math.max(bounds.width,p.x+CARD_WIDTH+60),height:Math.max(bounds.height,p.y+CARD_HEIGHT+60)};},{width:2000,height:1200});
}

/** A chosen endpoint stays available while a different page or search is displayed. */
export function connectionPageChoices(index:Map<string,CardPresentation>,query:string,selected:string,requested:number) {
  const filtered=filterBoard(index,query,'');
  const page=pageItems(filtered,requested);
  const choices=[...page.items];
  const chosen=index.get(selected)?.card;
  if(chosen&&!choices.some(c=>c.id===selected))choices.unshift(chosen);
  const ordinals=new Map([...index.keys()].map((id,i)=>[id,i+1]));
  return {...page,total:filtered.length,choices:choices.map(c=>({id:c.id,label:`${index.get(c.id)?.title||'Untitled card'} (#${ordinals.get(c.id)})`}))};
}
