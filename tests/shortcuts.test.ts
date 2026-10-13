import React from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {ShortcutSettings} from '../apps/writer/src/ShortcutSettings';
import {bindingFromEvent,defaultShortcutSettings,dispatchShortcut,editorShortcutOperation,formatShortcut,getShortcutSettings,matchShortcut,mouseClickCount,parseShortcutSettings,reloadShortcutSettings,resetShortcutSettings,saveShortcutSettings,SHORTCUT_CHANGE_EVENT,SHORTCUT_STORAGE_KEY,shortcutConflicts,updateShortcutBindings,validateShortcutBinding,type ShortcutBinding,type ShortcutKeyEvent} from '../apps/writer/src/shortcuts';
const binding=(code:string,ctrl=false,shift=false,alt=false,meta=false):ShortcutBinding=>({code,ctrl,shift,alt,meta});
const key=(code:string,extra:Partial<ShortcutKeyEvent>={}):ShortcutKeyEvent=>({code,ctrlKey:false,altKey:false,shiftKey:false,metaKey:false,...extra});
let store:Map<string,string>,surface:EventTarget;
beforeEach(()=>{store=new Map();surface=new EventTarget();vi.stubGlobal('window',surface);vi.stubGlobal('localStorage',{getItem:(id:string)=>store.get(id)??null,setItem:(id:string,value:string)=>store.set(id,value)});reloadShortcutSettings();});
afterEach(()=>vi.unstubAllGlobals());
function storedEvent(id:string|null){const event=new Event('storage');Object.defineProperty(event,'key',{value:id});surface.dispatchEvent(event);}
describe('central shortcut contract',()=>{
 it('round-trips every exact required action with unique defaults and independent copies',()=>{
  const defaults=defaultShortcutSettings();expect(shortcutConflicts(defaults)).toEqual([]);expect(parseShortcutSettings(JSON.parse(JSON.stringify(defaults)))).toEqual(defaults);
  defaults.bindings['workspace.save'][0].code='KeyJ';expect(defaultShortcutSettings().bindings['workspace.save'][0].code).toBe('KeyS');
 });
 it('matches physical positions in Arabic layouts and exact scope/modifiers',()=>{
  expect(matchShortcut(key('KeyB',{key:'لا',ctrlKey:true}),'editor')).toBe('editor.bold');
  expect(matchShortcut(key('KeyB',{key:'b',ctrlKey:true}),'workspace')).toBeNull();
  expect(matchShortcut(key('KeyB',{ctrlKey:true,shiftKey:true}),'editor')).toBeNull();
  expect(matchShortcut(key('',{key:'b',ctrlKey:true}),'editor')).toBeNull();
  expect(matchShortcut(key('KeyK',{ctrlKey:true}),'workspace')).toBe('workspace.commandPalette');expect(matchShortcut(key('Backslash',{ctrlKey:true,shiftKey:true}),'workspace')).toBe('workspace.split');
 });
 it.each([{isComposing:true},{keyCode:229},{key:'Process'},{key:'Dead'},{getModifierState:()=>true},{getModifierState:()=>{throw new Error('unavailable');}}])('preserves composition and AltGraph input %j',extra=>{
  expect(matchShortcut(key('KeyB',{ctrlKey:true,altKey:true,...extra}),'editor')).toBeNull();expect(bindingFromEvent(key('KeyB',{ctrlKey:true,...extra}))).toBeNull();
 });
 it('captures complete combinations, ignores modifier-only/repeated keys and reserves printable typing',()=>{
  expect(bindingFromEvent(key('ControlLeft',{ctrlKey:true}))).toBeNull();expect(bindingFromEvent(key('KeyJ',{ctrlKey:true,repeat:true}))).toBeNull();
  expect(bindingFromEvent(key('KeyJ',{ctrlKey:true,shiftKey:true}))).toEqual(binding('KeyJ',true,true));
  for(const code of ['KeyA','Digit5','Space','Comma','Numpad0']){expect(()=>validateShortcutBinding(binding(code))).toThrow(/Normal typing/);expect(()=>validateShortcutBinding(binding(code,false,true))).toThrow(/Normal typing/);}
  for(const code of ['F24','ArrowRight','Home','PageUp','Enter','Tab','Backspace','Delete'])expect(validateShortcutBinding(binding(code))).toEqual(binding(code));
  expect(()=>validateShortcutBinding({...binding('F2'),unknown:true})).toThrow();expect(()=>validateShortcutBinding(binding('Unidentified',true))).toThrow();
 });
 it('supports redo aliases, find navigation, formatting and semantic movement after remapping',()=>{
  expect(matchShortcut(key('KeyY',{ctrlKey:true}),'editor')).toBe('editor.redo');expect(matchShortcut(key('KeyZ',{ctrlKey:true,shiftKey:true}),'editor')).toBe('editor.redo');
  expect(matchShortcut(key('Space',{ctrlKey:true}),'editor')).toBe('editor.clearFormatting');expect(matchShortcut(key('F3'),'editor')).toBe('editor.findNext');
  expect(editorShortcutOperation('editor.doubleSpacing')).toEqual({type:'command',command:'lineSpacing',lineSpacing:2});expect(editorShortcutOperation('editor.oneAndHalfSpacing')).toEqual({type:'command',command:'lineSpacing',lineSpacing:1.5});
  const remapped=updateShortcutBindings(defaultShortcutSettings(),'editor.right.control.extend',[binding('KeyJ',true)]);const id=matchShortcut(key('KeyJ',{ctrlKey:true}),'editor',remapped)!;
  expect(id).toBe('editor.right.control.extend');expect(editorShortcutOperation(id)).toEqual({type:'key',key:'ArrowRight',ctrl:true,shift:true});
  expect(editorShortcutOperation('editor.deletePreviousWord')).toEqual({type:'key',key:'Backspace',ctrl:true,shift:false});expect(editorShortcutOperation('editor.lineBreak')).toEqual({type:'key',key:'Enter',ctrl:false,shift:true});
  expect(editorShortcutOperation('editor.find')).toBeNull();expect(editorShortcutOperation('workspace.save')).toBeNull();
 });
 it('routes configurable clipboard actions to explicit callbacks rather than editor commands',()=>{
  const callbacks={copy:vi.fn(),cut:vi.fn(),paste:vi.fn()};
  for(const [name,code] of [['copy','KeyC'],['cut','KeyX'],['paste','KeyV']] as const){
   const id=`editor.${name}` as const;expect(matchShortcut(key(code,{ctrlKey:true}),'editor')).toBe(id);expect(editorShortcutOperation(id)).toBeNull();
   const settings=updateShortcutBindings(defaultShortcutSettings(),id,[binding('F2')]),preventDefault=vi.fn();
   expect(dispatchShortcut(key('F2',{preventDefault}),'editor',{[id]:callbacks[name]},settings)).toBe(true);expect(callbacks[name]).toHaveBeenCalledOnce();expect(preventDefault).toHaveBeenCalledOnce();
   expect(matchShortcut(key(code,{ctrlKey:true}),'editor',settings)).toBeNull();
  }
 });
 it('converts Unicode with physical Alt+X while preserving AltGraph and composition',()=>{
  expect(matchShortcut(key('KeyX',{key:'ء',altKey:true}),'editor')).toBe('editor.unicodeToggle');expect(editorShortcutOperation('editor.unicodeToggle')).toEqual({type:'command',command:'unicodeToggle'});
  expect(matchShortcut(key('KeyX',{altKey:true,isComposing:true}),'editor')).toBeNull();expect(matchShortcut(key('KeyX',{altKey:true,getModifierState:()=>true}),'editor')).toBeNull();
  const remapped=updateShortcutBindings(defaultShortcutSettings(),'editor.unicodeToggle',[binding('F4')]);expect(matchShortcut(key('F4'),'editor',remapped)).toBe('editor.unicodeToggle');expect(matchShortcut(key('KeyX',{altKey:true}),'editor',remapped)).toBeNull();expect(matchShortcut(key('KeyX'),'editor')).toBeNull();
 });
 it('adds new actions to valid previous settings without overwriting remaps or explicit clears',()=>{
  const previous=defaultShortcutSettings() as any;delete previous.bindings['editor.unicodeToggle'];previous.bindings['workspace.save']=[binding('F2')];previous.bindings['editor.bold']=[];previous.mouse.doubleClick='caret';const frozen=JSON.stringify(previous);store.set(SHORTCUT_STORAGE_KEY,frozen);
  const loaded=reloadShortcutSettings();expect(loaded.error).toBeNull();expect(loaded.settings.bindings['workspace.save']).toEqual([binding('F2')]);expect(loaded.settings.bindings['editor.bold']).toEqual([]);expect(loaded.settings.mouse.doubleClick).toBe('caret');expect(loaded.settings.bindings['editor.unicodeToggle']).toEqual([binding('KeyX',false,false,true)]);expect(store.get(SHORTCUT_STORAGE_KEY)).toBe(frozen);
 });
 it('yields a newly introduced default to an existing binding and refuses malformed old settings',()=>{
  const previous=defaultShortcutSettings() as any;delete previous.bindings['editor.unicodeToggle'];previous.bindings['editor.bold']=[binding('KeyX',false,false,true)];const migrated=parseShortcutSettings(previous);
  expect(migrated.bindings['editor.unicodeToggle']).toEqual([]);expect(migrated.bindings['editor.bold']).toEqual(previous.bindings['editor.bold']);expect(shortcutConflicts(migrated)).toEqual([]);
  delete previous.bindings['workspace.save'];expect(()=>parseShortcutSettings(previous)).toThrow(/Unsupported/);
 });
 it('allows movement and deletion repeat without repeating window or formatting actions',()=>{
  expect(matchShortcut(key('ArrowRight',{repeat:true}),'editor')).toBe('editor.right');expect(matchShortcut(key('Backspace',{repeat:true}),'editor')).toBe('editor.backspace');
  expect(matchShortcut(key('KeyS',{ctrlKey:true,repeat:true}),'workspace')).toBeNull();expect(matchShortcut(key('KeyB',{ctrlKey:true,repeat:true}),'editor')).toBeNull();
 });
 it('refuses cross-scope and alias conflicts atomically, while clear then assign is explicit',()=>{
  const original=defaultShortcutSettings();expect(()=>updateShortcutBindings(original,'editor.bold',[binding('KeyS',true)])).toThrow(/Save tab.*Bold/);expect(original.bindings['editor.bold']).toEqual([binding('KeyB',true)]);
  expect(()=>updateShortcutBindings(original,'editor.bold',[binding('KeyB',true),binding('KeyB',true)])).toThrow(/conflict/);
  const cleared=updateShortcutBindings(original,'workspace.save',[]),updated=updateShortcutBindings(cleared,'editor.bold',[binding('KeyS',true)]);
  expect(matchShortcut(key('KeyS',{ctrlKey:true}),'editor',updated)).toBe('editor.bold');expect(matchShortcut(key('KeyS',{ctrlKey:true}),'workspace',updated)).toBeNull();
  expect(()=>updateShortcutBindings(original,'editor.bold',Array.from({length:4},()=>binding('KeyJ',true)))).toThrow(/three/);
 });
 it('dispatches only installed handlers and preserves normal fields/settings capture',()=>{
  const prevent=vi.fn(),handler=vi.fn(),event=key('KeyS',{ctrlKey:true,preventDefault:prevent});expect(dispatchShortcut(event,'workspace',{})).toBe(false);expect(prevent).not.toHaveBeenCalled();
  expect(dispatchShortcut(event,'workspace',{'workspace.save':handler})).toBe(true);expect(prevent).toHaveBeenCalledOnce();expect(handler).toHaveBeenCalledOnce();
  expect(dispatchShortcut({...event,defaultPrevented:true},'workspace',{'workspace.save':handler})).toBe(false);
  const settings=updateShortcutBindings(defaultShortcutSettings(),'workspace.save',[binding('F2')]);expect(matchShortcut(key('F2',{target:{tagName:'INPUT'} as unknown as EventTarget}),'workspace',settings)).toBe('workspace.save');
  const bare=updateShortcutBindings(updateShortcutBindings(defaultShortcutSettings(),'editor.home',[]),'workspace.save',[binding('Home')]);expect(matchShortcut(key('Home',{target:{tagName:'INPUT'} as unknown as EventTarget}),'workspace',bare)).toBeNull();
  expect(matchShortcut({...event,target:{closest:()=>({})} as unknown as EventTarget},'workspace')).toBeNull();
 });
 it('formats physical key names and remaps mouse selections without editing system preferences',()=>{
  expect(formatShortcut(binding('KeyJ',true,true))).toBe('Ctrl+Shift+J');expect(mouseClickCount(2)).toBe(2);expect(mouseClickCount(3)).toBe(3);expect(mouseClickCount(4)).toBe(3);expect(mouseClickCount(NaN)).toBe(1);
  const next={...defaultShortcutSettings(),mouse:{doubleClick:'paragraph' as const,tripleClick:'caret' as const}};expect(mouseClickCount(2,next)).toBe(3);expect(mouseClickCount(3,next)).toBe(1);expect(parseShortcutSettings(next).mouse).toEqual(next.mouse);
 });
});
describe('versioned shortcut persistence',()=>{
 it('persists locally, refreshes subscribers and explicitly resets defaults',()=>{
  const changed=vi.fn();surface.addEventListener(SHORTCUT_CHANGE_EVENT,changed);saveShortcutSettings(updateShortcutBindings(defaultShortcutSettings(),'workspace.save',[binding('F2')]));
  expect(changed).toHaveBeenCalledOnce();expect(JSON.parse(store.get(SHORTCUT_STORAGE_KEY)!)).toMatchObject({schemaVersion:1});expect(reloadShortcutSettings().settings.bindings['workspace.save']).toEqual([binding('F2')]);resetShortcutSettings();expect(getShortcutSettings()).toEqual(defaultShortcutSettings());
 });
 it('invalidates cached bindings only for this storage key or storage clear',()=>{
  const next=updateShortcutBindings(defaultShortcutSettings(),'workspace.save',[binding('F2')]);store.set(SHORTCUT_STORAGE_KEY,JSON.stringify(next));storedEvent('unrelated');expect(getShortcutSettings().bindings['workspace.save']).toEqual([binding('KeyS',true)]);
  storedEvent(SHORTCUT_STORAGE_KEY);expect(getShortcutSettings()).toEqual(next);store.clear();storedEvent(null);expect(getShortcutSettings()).toEqual(defaultShortcutSettings());
 });
 it.each(['{',JSON.stringify({...defaultShortcutSettings(),schemaVersion:2}),JSON.stringify({...defaultShortcutSettings(),unknown:true}), 'x'.repeat(40001)])('fails closed on invalid saved settings without overwriting them',raw=>{
  store.set(SHORTCUT_STORAGE_KEY,raw);const state=reloadShortcutSettings();expect(state.settings).toEqual(defaultShortcutSettings());expect(state.error).toMatch(/Defaults are active/);expect(store.get(SHORTCUT_STORAGE_KEY)).toBe(raw);
 });
 it('does not publish a binding change if persistence fails',()=>{
  vi.stubGlobal('localStorage',{getItem:()=>null,setItem:()=>{throw new Error('Synthetic quota exceeded');}});reloadShortcutSettings();const changed=vi.fn();surface.addEventListener(SHORTCUT_CHANGE_EVENT,changed);
  expect(()=>saveShortcutSettings(updateShortcutBindings(defaultShortcutSettings(),'workspace.save',[binding('F2')]))).toThrow(/quota/);expect(changed).not.toHaveBeenCalled();expect(getShortcutSettings()).toEqual(defaultShortcutSettings());
 });
 it('renders an accessible settings dialog with both aliases and conflict guidance',()=>{
  const html=renderToStaticMarkup(React.createElement(ShortcutSettings,{onClose:()=>{}}));expect(html).toContain('role="dialog"');expect(html).toContain('aria-modal="true"');expect(html).toContain('Ctrl+Y or Ctrl+Shift+Z');expect(html).toContain('Conflicting combinations are refused');expect(html).toContain('Double click');expect(html).toContain('Triple click');expect(html).toContain('Manuscript editing');
 });
});
