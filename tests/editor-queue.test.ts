import { describe, expect, it } from 'vitest';
import { EditorQueue } from '../apps/writer/src/editor-queue';

describe('waiting editor input', () => {
  it('keeps Arabic marks, Chinese and emoji exact while merging waiting typing', () => {
    const queue = new EditorQueue(); const text = 'مِ中文👩🏽‍🚀';
    [...text].forEach((char, i) => queue.push({ type: 'text', text: char }, i + 1, true, i + 1));
    const batch = queue.shift()!;
    expect(batch.operation.text).toBe(text);
    expect(batch.operation.inputs).toEqual([...text]);
    expect(batch.tickets).toEqual([...text].map((_, i) => i + 1));
    expect(batch.epoch).toBe([...text].length);
    expect(queue.length).toBe(0);
  });
  it('preserves typing around deletion, formatting and checkpoint boundaries', () => {
    const queue = new EditorQueue();
    const operations = [{ type: 'text', text: 'م' }, { type: 'key', key: 'Backspace' }, { type: 'command', command: 'bold' }, { type: 'text', text: 'مِ' }, { type: 'snapshot' }, { type: 'text', text: '中文' }];
    operations.forEach((op, i) => queue.push(op, i + 1, op.type !== 'snapshot', i));
    const actual = []; while (queue.length) actual.push(queue.shift()!.operation);
    expect(actual).toEqual(operations);
  });
  it('retains a resize-invalidated caret retry when a plain refresh coalesces behind it', () => {
    const queue = new EditorQueue();
    queue.push({type:'refresh',followCursorGeneration:4},1,false,0);
    queue.push({type:'refresh'},2,false,0);
    expect(queue.shift()).toEqual({operation:{type:'refresh',followCursorGeneration:4},tickets:[1,2],edited:false,epoch:0});
  });
  it('uses the newest explicit caret generation and does not carry it across a command barrier', () => {
    const queue = new EditorQueue();
    queue.push({type:'refresh',followCursorGeneration:4},1,false,0);
    queue.push({type:'refresh',followCursorGeneration:5},2,false,0);
    queue.push({type:'key',key:'End',followCursorGeneration:5},3,false,0);
    queue.push({type:'refresh'},4,false,0);
    expect(queue.shift()!.operation).toEqual({type:'refresh',followCursorGeneration:5});
    expect(queue.shift()!.operation.type).toBe('key');
    expect(queue.shift()!.operation).toEqual({type:'refresh'});
  });
  it('coalesces only contiguous pointer moves, keeping down/up and modifier changes', () => {
    const queue = new EditorQueue();
    const mouse = (phase: string, x: number, shift = false) => ({ type: 'mouse', phase, x, y: 0, buttons: true, shift });
    [mouse('down', 1), mouse('move', 2), mouse('move', 3), mouse('move', 4, true), mouse('up', 5)].forEach((op, i) => queue.push(op, i + 1, false, 0));
    const actual = []; while (queue.length) actual.push(queue.shift()!);
    expect(actual.map(item => item.operation)).toEqual([mouse('down', 1), mouse('move', 3), mouse('move', 4, true), mouse('up', 5)]);
    expect(actual.flatMap(item => item.tickets)).toEqual([1, 2, 3, 4, 5]);
  });
  it('bounds batches and preserves every original input and ticket', () => {
    const queue=new EditorQueue();
    for(let i=0;i<260;i++)expect(queue.push({type:'text',text:'مِ'},i+1,true,i+1)).toBe(true);
    const entries=[];while(queue.length)entries.push(queue.shift()!);
    expect(entries.map(entry=>(entry.operation.inputs as string[]|undefined)?.length??1)).toEqual([128,128,4]);
    expect(entries.flatMap(entry=>entry.tickets)).toEqual(Array.from({length:260},(_,i)=>i+1));
    expect(entries.map(entry=>entry.operation.text).join('')).toBe('مِ'.repeat(260));
  });
  it('keeps composition, paste and manual scroll generations as boundaries', () => {
    const queue=new EditorQueue();
    const operations=[{type:'text',text:'a',followCursorGeneration:1},{type:'text',text:'中',inputBoundary:'composition'},
      {type:'text',text:'pasted',inputBoundary:'paste'},{type:'text',text:'b',followCursorGeneration:1},{type:'text',text:'c',followCursorGeneration:2}];
    operations.forEach((op,i)=>queue.push(op,i+1,true,i));
    const actual=[];while(queue.length)actual.push(queue.shift()!.operation);
    expect(actual).toEqual(operations);
  });
  it('replaces a waiting plain viewport paint with the edit that reads the live viewport', () => {
    const queue=new EditorQueue();queue.push({type:'refresh'},1,false,0);queue.push({type:'text',text:'يس',followCursorGeneration:3},2,true,1);
    expect(queue.shift()).toEqual({operation:{type:'text',text:'يس',followCursorGeneration:3},tickets:[1,2],edited:true,epoch:1});
    queue.push({type:'refresh',followCursorGeneration:2},3,false,1);queue.push({type:'text',text:'مِ',followCursorGeneration:3},4,true,2);
    expect(queue.length).toBe(2);
  });
  it('rejects backlog growth explicitly and releases admission capacity after draining', () => {
    const queue=new EditorQueue();
    for(let i=0;i<256;i++)expect(queue.push({type:'key',key:'ArrowLeft'},i,false,0)).toBe(true);
    expect(queue.push({type:'key',key:'ArrowRight'},257,false,0)).toBe(false);
    queue.shift();expect(queue.push({type:'key',key:'ArrowRight'},258,false,0)).toBe(true);
    const large=new EditorQueue();expect(large.push({type:'text',text:'x'.repeat(2_200_000)},1,true,1)).toBe(false);
    expect(large.length).toBe(0);
  });

});
