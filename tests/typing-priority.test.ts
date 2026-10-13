import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {noteTyping,releaseTyping,setTypingComposing,setTypingPending,typingIsActive,waitForTypingIdle} from '../apps/writer/src/typing-priority';
const left={},right={};
beforeEach(()=>vi.useFakeTimers({toFake:['setTimeout','clearTimeout','performance']}));
afterEach(()=>{releaseTyping(left);releaseTyping(right);vi.useRealTimers();});
describe('cooperative writing priority',()=>{
  it('background work waits for the typing quiet interval and yields a browser turn',async()=>{
    noteTyping(left);let ran=false;const waiting=waitForTypingIdle().then(()=>{ran=true;});
    await vi.advanceTimersByTimeAsync(100);expect(ran).toBe(false);
    noteTyping(left);await vi.advanceTimersByTimeAsync(100);expect(ran).toBe(false);
    await vi.advanceTimersByTimeAsync(100);await waiting;expect(ran).toBe(true);
    ran=false;const idle=waitForTypingIdle().then(()=>{ran=true;});expect(ran).toBe(false);
    await vi.advanceTimersByTimeAsync(1);await idle;expect(ran).toBe(true);
  });
  it('either split editor or an active composition holds background work until released',async()=>{
    setTypingPending(left,true);setTypingComposing(right,true);
    let ran=false;const waiting=waitForTypingIdle().then(()=>{ran=true;});
    await vi.advanceTimersByTimeAsync(500);expect(ran).toBe(false);
    setTypingPending(left,false);await vi.advanceTimersByTimeAsync(100);expect(ran).toBe(false);
    setTypingComposing(right,false);await vi.advanceTimersByTimeAsync(20);await waiting;expect(ran).toBe(true);
  });
  it('rechecks new typing that starts during the idle browser turn',async()=>{
    let ran=false;const waiting=waitForTypingIdle().then(()=>{ran=true;});
    noteTyping(left);await vi.advanceTimersByTimeAsync(10);expect(ran).toBe(false);
    await vi.advanceTimersByTimeAsync(170);await waiting;expect(ran).toBe(true);
  });
  it('abort and unmount release outstanding waits and their timers',async()=>{
    setTypingPending(left,true);const abort=new AbortController();const waiting=waitForTypingIdle(abort.signal);
    const result=waiting.catch(error=>error);abort.abort(new Error('Closed'));
    expect((await result).message).toBe('Closed');expect(vi.getTimerCount()).toBe(0);
    expect(typingIsActive()).toBe(true);releaseTyping(left);expect(typingIsActive()).toBe(false);
  });
});
