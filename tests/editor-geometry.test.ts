import {expect,it} from 'vitest';
import {nativeViewport,editorRequestViewport} from '../apps/writer/src/editor-geometry';
it('hidden checkpoints retain the exact prior width, finite coordinates and saved scroll anchor',()=>{
  expect(nativeViewport(0,640,0,0,400)).toEqual({width:640,scale:1,top:400,viewportHeight:620});
  expect(nativeViewport(0,350,0,0,790)).toEqual({width:350,scale:1,top:790,viewportHeight:620});
  expect(Number.isFinite(nativeViewport(0,640,0,0,NaN).top)).toBe(true);
});
it('reflows a narrowed split at its measured width instead of shrinking the former wide bitmap',()=>{
  expect(nativeViewport(350,1200,150,650,790)).toEqual({width:350,scale:1,top:514,viewportHeight:650});
  expect(nativeViewport(960,350,600,900,400)).toEqual({width:960,scale:1,top:219,viewportHeight:900});
});
it('preserves old-layout passage coordinates while sizing the new bounded viewport',()=>{
  expect(nativeViewport(200,640,100,400,0)).toEqual({width:240,scale:200/240,top:320,viewportHeight:480});
  expect(nativeViewport(1500,640,500,1000,0)).toEqual({width:1500,scale:1,top:213,viewportHeight:1000});
  const fractional=nativeViewport(350.4,1200,100,600,0);expect(fractional.width).toBe(350);expect(fractional.scale).toBe(350.4/350);expect(fractional.top).toBe(Math.round(100/(350.4/1200)));
});
it('bounds hidden/invalid dimensions without sending NaN or infinite viewport values',()=>{
  for(const invalid of [NaN,Infinity,-10]){const result=nativeViewport(invalid,NaN,Infinity,NaN,NaN);expect(result).toEqual({width:640,scale:1,top:0,viewportHeight:620});}
  expect(nativeViewport(350,1200,-50,1,0).top).toBe(0);expect(nativeViewport(350,1200,0,3000,0).viewportHeight).toBe(2160);expect(nativeViewport(350,1200,0,1,0).viewportHeight).toBe(120);
});

it('keeps queued mouse hit tests in captured display geometry through a later resize',()=>{
  const captured=nativeViewport(600,1200,150,650,790,1200);
  expect(captured).toEqual({width:1200,scale:0.5,top:300,viewportHeight:1300});
  const mouse={type:'mouse',width:captured.width,top:captured.top,viewportHeight:captured.viewportHeight,x:4000,y:8000};
  const later=nativeViewport(350,1200,150,650,790);
  expect(editorRequestViewport(mouse,later)).toEqual({width:1200,top:300,viewportHeight:1300});
  expect(editorRequestViewport({type:'key',key:'ArrowDown'},later)).toEqual({width:350,top:514,viewportHeight:650});
  expect(editorRequestViewport({type:'refresh'},later)).toEqual({width:350,top:514,viewportHeight:650});
  expect(mouse).toMatchObject({x:4000,y:8000,width:1200});
});

it('uses real full-width pixels and bounds large display viewports consistently',()=>{
 expect(nativeViewport(3840,3840,100,2160,0)).toEqual({width:3840,scale:1,top:100,viewportHeight:2160});
 expect(nativeViewport(4096,4096,200,3000,0)).toEqual({width:4096,scale:1,top:200,viewportHeight:2160});
 expect(nativeViewport(5000,4096,0,3000,0).width).toBe(4096);
});
