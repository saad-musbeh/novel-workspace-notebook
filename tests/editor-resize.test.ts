import {afterEach,expect,it,vi} from 'vitest';
import {EDITOR_RESIZE_EVENT,observeEditorSize} from '../apps/writer/src/editor-resize';
afterEach(()=>{vi.unstubAllGlobals();vi.useRealTimers();});
it('measures immediately and deduplicates observer/window/host notifications without idle work',()=>{
 vi.useFakeTimers();let delivery:()=>void=()=>{};const disconnect=vi.fn(),observe=vi.fn();vi.stubGlobal('ResizeObserver',class {constructor(callback:()=>void){delivery=callback;}observe=observe;disconnect=disconnect;});
 const dimensions={clientWidth:354,clientHeight:500},surface=dimensions as HTMLElement,target=new EventTarget(),paint=vi.fn();let timer:ReturnType<typeof setTimeout>|undefined;
 const subscription=observeEditorSize(surface,surface,(width,height)=>{clearTimeout(timer);timer=setTimeout(()=>paint(width,height),100);},target);
 delivery();target.dispatchEvent(new Event('resize'));target.dispatchEvent(new Event(EDITOR_RESIZE_EVENT));vi.advanceTimersByTime(100);expect(paint).toHaveBeenCalledExactlyOnceWith(354,500);
 dimensions.clientWidth=258;target.dispatchEvent(new Event(EDITOR_RESIZE_EVENT));delivery();target.dispatchEvent(new Event('resize'));vi.advanceTimersByTime(99);expect(paint).toHaveBeenCalledTimes(1);vi.advanceTimersByTime(1);expect(paint).toHaveBeenLastCalledWith(258,500);
 vi.advanceTimersByTime(10000);expect(paint).toHaveBeenCalledTimes(2);subscription.dispose();expect(disconnect).toHaveBeenCalledOnce();dimensions.clientWidth=810;delivery();target.dispatchEvent(new Event(EDITOR_RESIZE_EVENT));vi.advanceTimersByTime(1000);expect(paint).toHaveBeenCalledTimes(2);expect(observe).toHaveBeenCalledTimes(2);
});
it('explicit host notifications work without any ResizeObserver delivery and ignore hidden dimensions',()=>{
 vi.stubGlobal('ResizeObserver',undefined);const dimensions={clientWidth:0,clientHeight:0},target=new EventTarget(),changed=vi.fn();const subscription=observeEditorSize(dimensions as HTMLElement,dimensions as HTMLElement,changed,target);
 expect(changed).not.toHaveBeenCalled();dimensions.clientWidth=534;dimensions.clientHeight=650;subscription.measure();expect(changed).toHaveBeenCalledExactlyOnceWith(534,650);
 dimensions.clientWidth=810;target.dispatchEvent(new Event(EDITOR_RESIZE_EVENT));expect(changed).toHaveBeenLastCalledWith(810,650);
 dimensions.clientHeight=700;target.dispatchEvent(new Event('resize'));expect(changed).toHaveBeenLastCalledWith(810,700);
 dimensions.clientWidth=0;target.dispatchEvent(new Event(EDITOR_RESIZE_EVENT));expect(changed).toHaveBeenCalledTimes(3);subscription.dispose();
});
