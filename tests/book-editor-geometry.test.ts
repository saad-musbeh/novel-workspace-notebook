import {describe,expect,it} from 'vitest';
import {bookViewport} from '../apps/writer/src/book-model';
import {nativeViewport,editorRequestViewport} from '../apps/writer/src/editor-geometry';

describe('shared book viewport uses each scene surface coordinates',()=>{
 it('maps a manually scrolled scene into native pixels without incorporating preceding scenes',()=>{
  const visible=bookViewport({top:-950,height:4200},{top:50,height:680});
  const viewport=nativeViewport(425,850,visible.top,visible.height,0);
  expect(viewport).toEqual({width:425,scale:1,top:2000,viewportHeight:680});
  // Moving every book row and the host together must not change this scene passage.
  const moved=bookViewport({top:-450,height:4200},{top:550,height:680});
  expect(nativeViewport(425,850,moved.top,moved.height,0)).toEqual(viewport);
 });
 it('retains the old passage when narrowing, with a bounded paint after widening',()=>{
  const visible=bookViewport({top:-800,height:5000},{top:0,height:900});
  const narrowed=nativeViewport(240,850,visible.top,visible.height,0);
  expect(narrowed.top).toBe(Math.round(800/(240/850)));
  expect(narrowed.viewportHeight).toBe(900);
  const widened=nativeViewport(1500,850,visible.top,visible.height,0);
  expect(widened.width).toBe(1500);expect(widened.viewportHeight).toBe(900);
  expect(widened.top).toBe(Math.round(800/(1500/850)));
 });
 it('queued selection retains the captured scene viewport despite later host scroll and resize',()=>{
  const first=bookViewport({top:-500,height:6000},{top:0,height:680});
  const captured=nativeViewport(425,850,first.top,first.height,0,850);
  const mouse={type:'mouse',width:captured.width,top:captured.top,viewportHeight:captured.viewportHeight,x:3000,y:18000};
  const later=bookViewport({top:-1400,height:6000},{top:0,height:680});
  const live=nativeViewport(800,850,later.top,later.height,0);
  expect(editorRequestViewport(mouse,live)).toEqual({width:850,top:1000,viewportHeight:1360});
  expect(editorRequestViewport({type:'refresh'},live).top).toBe(1488);
  expect(mouse.y).toBe(18000);
 });
 it('hidden scene checkpoints retain native anchor instead of resetting the shared host',()=>{
  expect(nativeViewport(0,425,0,0,2317)).toEqual({width:425,scale:1,top:2317,viewportHeight:620});
  const below=bookViewport({top:900,height:2000},{top:0,height:680});
  expect(below.top).toBe(0);
 });
});
