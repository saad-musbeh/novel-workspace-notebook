import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Board, ConnectionEndpoint } from '../apps/writer/src/planning/Board';
import { boardBounds, connectionPageChoices, filterBoard, indexBoard, listNavigationTarget, moveCardForKey, navigationTarget, pageItems, visibleBoardCards, visibleBoardEdges } from '../apps/writer/src/planning/board-view';
import { type BoardCard, type PlanningDocument, removeBoardCard, validatePlanningDocument } from '../apps/writer/src/planning/model';

function fixture(count=1000):PlanningDocument {
  const cards:BoardCard[]=Array.from({length:count},(_,i)=>({id:`card-${i}`,title:`Note ${i} نور 明`,body:'Synthetic English العربية 简体 繁體',kind:'note',reference:null,x:60+(i%10)*260,y:60+Math.floor(i/10)*300,groupId:i%2?'group':null,labels:[`label-${i}`]}));
  return {schemaVersion:1,fields:[],records:[],plotPoints:[],facts:[],boards:[{id:'board',name:'Synthetic board',cards,groups:[{id:'group',name:'Thread'}],edges:cards.slice(1).map((c,i)=>({id:`edge-${i}`,fromId:cards[i].id,toId:c.id,kind:'depends-on',label:`Prerequisite ${i}`,confirmed:i%2===0}))}],lanes:[{id:'main',name:'Main',kind:'main'}],events:[],constraints:[],issues:[]};
}
const viewport={left:0,top:0,width:900,height:510};
describe('board navigation, projection and source preservation',()=>{
  it('searches current canonical plot titles and notes, retaining fallback text for missing links',()=>{
    const doc=fixture(4),board=doc.boards[0];
    doc.plotPoints=[{id:'point',title:'نور returns',notes:'New clue: 雨',links:[],requiresPayoff:false,scopeClosed:false,payoffId:null}];
    board.cards[0]={...board.cards[0],kind:'plot-point',reference:{kind:'plot-point',id:'point'},title:'Old cached title',body:'Obsolete clue'};
    board.cards[1].reference={kind:'scene',id:'scene'};
    board.cards[2].reference={kind:'record',id:'missing'};
    const before=JSON.stringify(doc),index=indexBoard(doc,[{id:'scene',title:'夜晚 arrival'}],board);
    expect(filterBoard(index,'雨','').map(c=>c.id)).toEqual(['card-0']);
    expect(filterBoard(index,'نور returns','').map(c=>c.id)).toEqual(['card-0']);
    expect(filterBoard(index,'Obsolete clue','')).toEqual([]);
    expect(filterBoard(index,'夜晚','').map(c=>c.id)).toEqual(['card-1']);
    expect(index.get('card-2')?.referenceLabel).toBe('Missing record (missing)');
    expect(filterBoard(index,'label-3','group').map(c=>c.id)).toEqual(['card-3']);
    expect(JSON.stringify(doc)).toBe(before);
    doc.plotPoints[0].notes='مَرْحَبًا updated';
    expect(filterBoard(indexBoard(doc,[],board),'مَرْحَبًا','').map(c=>c.id)).toEqual(['card-0']);
  });
  it('provides first/last/adjacent/page navigation across all 1,000 cards and filtered groups',()=>{
    const doc=fixture(),cards=doc.boards[0].cards,before=JSON.stringify(doc);
    expect(navigationTarget(cards,'','ArrowDown')).toBe('card-0');
    expect(navigationTarget(cards,'card-0','ArrowUp')).toBe('card-0');
    expect(navigationTarget(cards,'card-999','ArrowDown')).toBe('card-999');
    expect(navigationTarget(cards,'card-500','Home')).toBe('card-0');
    expect(navigationTarget(cards,'card-0','End')).toBe('card-999');
    expect(navigationTarget(cards,'card-10','PageDown')).toBe('card-60');
    expect(navigationTarget(cards,'card-10','PageUp')).toBe('card-0');
    const filtered=filterBoard(indexBoard(doc,[],doc.boards[0]),'','group');
    expect(navigationTarget(filtered,'card-1','ArrowRight')).toBe('card-3');
    expect(navigationTarget(filtered,'card-2','ArrowLeft')).toBe('card-1');
    expect(navigationTarget(filtered,'card-1','End')).toBe('card-999');
    expect(navigationTarget([],'card-0','End')).toBeNull();
    expect(navigationTarget(cards,'card-0','Enter')).toBeNull();
    expect(JSON.stringify(doc)).toBe(before);
  });
  it('starts keyboard selection on the displayed list page after paging away, without changing source data',()=>{
    const doc=fixture(),cards=doc.boards[0].cards,before=JSON.stringify(doc);
    // Explicit Previous page from the final selected card clears the hidden selection.
    expect(pageItems(cards,18).items.some(c=>c.id==='card-999')).toBe(false);
    for(const key of ['ArrowDown','ArrowRight','ArrowUp','ArrowLeft'])expect(listNavigationTarget(cards,'',18,key)).toBe('card-900');
    expect(listNavigationTarget(cards,'card-900',18,'ArrowDown')).toBe('card-901');
    expect(listNavigationTarget(cards,'',18,'Home')).toBe('card-0');
    expect(listNavigationTarget(cards,'',18,'End')).toBe('card-999');
    expect(listNavigationTarget(cards,'',18,'PageDown')).toBe('card-950');
    expect(listNavigationTarget(cards,'',18,'PageUp')).toBe('card-850');
    expect(listNavigationTarget(cards,'',999,'ArrowDown')).toBe('card-950');
    expect(listNavigationTarget(cards.slice(0,2),'missing',18,'ArrowDown')).toBe('card-0');
    expect(listNavigationTarget([],'',18,'ArrowDown')).toBeNull();
    expect(listNavigationTarget(cards,'',18,'Enter')).toBeNull();
    expect(JSON.stringify(doc)).toBe(before);
  });
  it('moves coordinates only, with precise 10/50 steps and a nonnegative boundary',()=>{
    const doc=fixture(1),card={...doc.boards[0].cards[0],x:5,y:5,body:'مَرْحَبًا  “quote”'},before=JSON.stringify(card);
    expect(moveCardForKey(card,'ArrowLeft')).toEqual({x:0,y:5});
    expect(moveCardForKey(card,'ArrowUp',true)).toEqual({x:5,y:0});
    expect(moveCardForKey(card,'ArrowRight')).toEqual({x:15,y:5});
    expect(moveCardForKey(card,'ArrowDown',true)).toEqual({x:5,y:55});
    expect(moveCardForKey(card,'Escape')).toBeNull();
    expect(JSON.stringify(card)).toBe(before);
  });
  it('keeps focused and dragged cards mounted while bounding normal viewport cards',()=>{
    const doc=fixture(),cards=doc.boards[0].cards,before=JSON.stringify(doc);
    const visible=visibleBoardCards(cards,viewport,1,[],null);
    expect(visible.length).toBeLessThan(25);
    expect(visible.map(c=>c.id)).not.toContain('card-999');
    const retained=visibleBoardCards(cards,viewport,1,['card-999','card-998'],{id:'card-998',x:80,y:80});
    expect(retained.map(c=>c.id)).toContain('card-999');
    expect(retained.map(c=>c.id)).toContain('card-998');
    expect(retained.length).toBeLessThan(27);
    const resized=visibleBoardCards(cards,{...viewport,width:1600},1,[],null);
    expect(resized.length).toBeGreaterThan(visible.length);
    const panned=visibleBoardCards(cards,{...viewport,left:1800,top:29700},1,['card-0'],null);
    expect(panned.map(c=>c.id)).toContain('card-0');
    expect(panned.map(c=>c.id)).toContain('card-999');
    expect(JSON.stringify(doc)).toBe(before);
  });
  it('projects directed connections using filtered endpoints without rewriting relationships',()=>{
    const doc=fixture(),board=doc.boards[0],before=JSON.stringify(doc);
    const visible=visibleBoardCards(board.cards,viewport,1,[],null),projected=visibleBoardEdges(board.edges,board.cards,visible);
    expect(projected).toContain(board.edges[0]);
    expect(projected).not.toContain(board.edges[998]);
    expect(visibleBoardEdges(board.edges,board.cards.slice(0,1),visible)).toEqual([]);
    expect(projected[0]).toMatchObject({fromId:'card-0',toId:'card-1',kind:'depends-on',confirmed:true});
    expect(JSON.stringify(doc)).toBe(before);
  });
  it('makes every list card and connection reachable in stable 50-item pages',()=>{
    const doc=fixture(),board=doc.boards[0],before=JSON.stringify(doc);
    const all=Array.from({length:20},(_,i)=>pageItems(board.cards,i).items).flat();
    expect(all.map(c=>c.id)).toEqual(board.cards.map(c=>c.id));
    expect(pageItems(board.cards,100)).toMatchObject({page:19,start:950,end:1000});
    expect(pageItems(board.edges,19).items).toHaveLength(49);
    expect(pageItems(board.cards.slice(0,2),19)).toMatchObject({page:0,pages:1,start:0,end:2});
    expect(pageItems([],10)).toMatchObject({items:[],page:0,pages:1,start:0,end:0});
    expect(pageItems(board.cards,Number.NaN,Number.NaN).items).toHaveLength(50);
    expect(JSON.stringify(doc)).toBe(before);
  });
  it('bounds endpoint pickers, preserves a chosen off-page endpoint, and disambiguates identical titles',()=>{
    const doc=fixture(),board=doc.boards[0];board.cards[0].title=board.cards[1].title='Same title';const before=JSON.stringify(doc);
    const index=indexBoard(doc,[],board),page=connectionPageChoices(index,'','card-999',0);
    expect(page.choices).toHaveLength(51);expect(page.choices[0]).toMatchObject({id:'card-999'});
    expect(page.choices.find(c=>c.id==='card-0')?.label).toBe('Same title (#1)');
    expect(page.choices.find(c=>c.id==='card-1')?.label).toBe('Same title (#2)');
    expect(connectionPageChoices(index,'label-998','card-999',0).choices.map(c=>c.id)).toEqual(['card-999','card-998']);
    const all=Array.from({length:20},(_,i)=>connectionPageChoices(index,'','',i).choices).flat();
    expect(all.map(c=>c.id)).toEqual(board.cards.map(c=>c.id));
    expect(connectionPageChoices(index,'absent','',99)).toMatchObject({choices:[],total:0,page:0,pages:1});
    expect(JSON.stringify(doc)).toBe(before);
    const html=renderToStaticMarkup(React.createElement(ConnectionEndpoint,{label:'Connection from',index,value:'card-999',onChange:()=>{}}));
    expect(html.match(/<option /g)).toHaveLength(52);expect(html).toContain('Connection from filter');expect(html).toContain('1–50 of 1000');
  });
  it('computes scroll bounds without spreading arrays or changing stored positions',()=>{
    const doc=fixture(),before=JSON.stringify(doc);
    expect(boardBounds(doc.boards[0].cards,null)).toEqual({width:2680,height:30080});
    expect(boardBounds([],null)).toEqual({width:2000,height:1200});
    expect(boardBounds(doc.boards[0].cards,{id:'card-999',x:9000,y:90000})).toEqual({width:9280,height:90320});
    expect(JSON.stringify(doc)).toBe(before);
  });
  it('removing a view card leaves canonical plot data and other board instances intact',()=>{
    const doc=fixture(2);doc.plotPoints=[{id:'point',title:'明',notes:'مَرْحَبًا',links:[{kind:'scene',id:'scene'}],requiresPayoff:true,scopeClosed:false,payoffId:null}];
    doc.boards[0].cards[0].reference={kind:'plot-point',id:'point'};
    doc.boards.push({id:'other-board',name:'Other',cards:[{...doc.boards[0].cards[0],id:'other-card'}],groups:[],edges:[]});
    validatePlanningDocument(doc);
    const result=removeBoardCard(doc,'board','card-0');
    expect(result.plotPoints).toEqual(doc.plotPoints);expect(result.boards[1]).toEqual(doc.boards[1]);expect(result.boards[0].edges).toEqual([]);validatePlanningDocument(result);
  });
});
describe('large synthetic board markup',()=>{
  it('bounds freeform DOM and defers connection forms rather than mounting 1,000 endpoints',()=>{
    const doc=fixture(),before=JSON.stringify(doc);validatePlanningDocument(doc);
    const html=renderToStaticMarkup(React.createElement(Board,{document:doc,change:()=>{},scenes:[]}));
    expect(html.match(/<article /g)?.length).toBeLessThan(25);
    expect(html).toContain('Connections (999)');expect(html).not.toContain('Connection from');
    expect(html).toContain('aria-label="Board cards"');expect(html).toContain('role="region"');expect(html).toContain('tabindex="0"');
    expect(html).toContain('Show selected card');expect(html).toContain('Home / End');
    expect(html).toContain('aria-label="Move Note 0 نور 明"');expect(html).toContain('aria-label="Select Note 0 نور 明"');
    expect(html.length).toBeLessThan(40_000);expect(JSON.stringify(doc)).toBe(before);
  });
  it('bounds list DOM to 50 cards with visible page controls and multilingual source text',()=>{
    const doc=fixture(),before=JSON.stringify(doc);
    const html=renderToStaticMarkup(React.createElement(Board,{document:doc,change:()=>{},scenes:[],initialView:'list'}));
    expect(html.match(/<article /g)).toHaveLength(50);
    expect(html).toContain('1–50 of 1000');expect(html).toContain('Next page');expect(html).toContain('Previous page');
    expect(html).toContain('العربية 简体 繁體');expect(html).toContain('Freeform view');expect(html).not.toContain('<svg');
    expect(html).not.toContain('Connection from');expect(html.length).toBeLessThan(75_000);expect(JSON.stringify(doc)).toBe(before);
  });
  it('keeps initial DOM size stable as synthetic boards grow from 1,000 to 10,000 cards',()=>{
    let visibleCount:number|undefined;
    for(const count of [1000,10000]){
      const doc=fixture(count),before=JSON.stringify(doc);
      const html=renderToStaticMarkup(React.createElement(Board,{document:doc,change:()=>{},scenes:[]}));
      const articles=html.match(/<article /g)?.length??0;
      expect(articles).toBeGreaterThan(0);expect(articles).toBeLessThan(25);
      visibleCount??=articles;expect(articles).toBe(visibleCount);
      expect(html.length).toBeLessThan(40_000);expect(JSON.stringify(doc)).toBe(before);
    }
  });
  it('escapes titles and notes without modifying their exact stored codepoints',()=>{
    const doc=fixture(1);doc.boards[0].cards[0].title='<script>bad</script> مَرْحَبًا';doc.boards[0].cards[0].body='"quoted" & ';
    const before=JSON.stringify(doc),html=renderToStaticMarkup(React.createElement(Board,{document:doc,change:()=>{},scenes:[]}));
    expect(html).not.toContain('<script>');expect(html).toContain('&lt;script&gt;');expect(html).toContain('');expect(JSON.stringify(doc)).toBe(before);
  });
});
