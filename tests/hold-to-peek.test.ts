import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
const hooks=vi.hoisted(()=>({held:false,cleanup:undefined as undefined|(()=>void),ref:{current:null as string|null}}));
vi.mock('react',()=>({useState:()=>[hooks.held,(value:boolean)=>{hooks.held=value;}],useRef:()=>hooks.ref,useEffect:(effect:()=>void|(()=>void))=>{hooks.cleanup=effect()??undefined;}}));
import {useHoldToPeek} from '../apps/writer/src/hold-to-peek';
let surface:EventTarget,doc:EventTarget&{visibilityState:string};
function key(type:string,extra:Record<string,unknown>={}){const event=new Event(type,{cancelable:true});Object.assign(event,{code:'F6',key:'F6',ctrlKey:false,altKey:false,shiftKey:false,metaKey:false,...extra});surface.dispatchEvent(event);return event;}
beforeEach(()=>{hooks.held=false;hooks.ref.current=null;hooks.cleanup=undefined;surface=new EventTarget();doc=Object.assign(new EventTarget(),{visibilityState:'visible'});vi.stubGlobal('window',surface);vi.stubGlobal('document',doc);vi.stubGlobal('localStorage',{getItem:()=>null});});
afterEach(()=>{hooks.cleanup?.();vi.unstubAllGlobals();});
describe('held reference view',()=>{
 it('opens only on the matched shortcut and closes on matching release',()=>{useHoldToPeek(true);expect(key('keydown').defaultPrevented).toBe(true);expect(hooks.held).toBe(true);key('keyup',{code:'KeyA'});expect(hooks.held).toBe(true);key('keyup');expect(hooks.held).toBe(false);});
 it.each(['blur','Escape','hidden'])('cannot stick after %s',reason=>{useHoldToPeek(true);key('keydown');if(reason==='blur')surface.dispatchEvent(new Event('blur'));else if(reason==='Escape')key('keydown',{key:'Escape',code:'Escape'});else {doc.visibilityState='hidden';doc.dispatchEvent(new Event('visibilitychange'));}expect(hooks.held).toBe(false);expect(hooks.ref.current).toBeNull();});
 it('leaves composing input and inactive tabs untouched',()=>{useHoldToPeek(true);expect(key('keydown',{isComposing:true}).defaultPrevented).toBe(false);expect(hooks.held).toBe(false);hooks.cleanup?.();useHoldToPeek(false);key('keydown');expect(hooks.held).toBe(false);});
});
