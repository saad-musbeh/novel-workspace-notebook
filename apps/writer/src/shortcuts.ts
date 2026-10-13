import {useEffect,useState} from 'react';

export type ShortcutScope='workspace'|'editor';
export interface ShortcutBinding {code:string;ctrl:boolean;alt:boolean;shift:boolean;meta:boolean}
type WorkspaceAction='save'|'newTab'|'closeTab'|'nextTab'|'previousTab'|'detachTab'|'returnTab'|'quit'|'shortcuts'|'fullscreen'|'commandPalette'|'split'|'peek'|'navigateBack'|'navigateForward'|'peekChanges'|`sequence${1|2|3|4|5|6|7|8}`;
type EditorAction='copy'|'cut'|'paste'|'bold'|'italic'|'underline'|'clearFormatting'|'unicodeToggle'|'undo'|'redo'|'selectAll'|'singleSpacing'|'oneAndHalfSpacing'|'doubleSpacing'|'find'|'replace'|'findNext'|'backspace'|'delete'|'deletePreviousWord'|'deleteNextWord'|'paragraphBreak'|'lineBreak'|'tab'|'backTab'|'escape';
type NavigationName='left'|'right'|'up'|'down'|'home'|'end'|'pageUp'|'pageDown';
export type ShortcutActionId=`workspace.${WorkspaceAction}`|`editor.${EditorAction}`|`editor.${NavigationName}${''|'.extend'|'.control'|'.control.extend'}`;
export type EditorShortcutOperation={type:'key';key:string;ctrl:boolean;shift:boolean}|{type:'command';command:string;lineSpacing?:number};
export interface ShortcutActionDefinition {id:ShortcutActionId;label:string;scope:ShortcutScope;defaults:readonly ShortcutBinding[];repeat:boolean;operation?:EditorShortcutOperation}
export interface ShortcutSettingsModel {schemaVersion:1;bindings:Record<ShortcutActionId,ShortcutBinding[]>;mouse:{doubleClick:'caret'|'word'|'paragraph';tripleClick:'caret'|'word'|'paragraph'}}
export const SHORTCUT_STORAGE_KEY='novel-workspace.shortcuts.v1';
export const SHORTCUT_CHANGE_EVENT='workspace-shortcuts-changed';
const bind=(code:string,ctrl=false,shift=false,alt=false,meta=false):ShortcutBinding=>({code,ctrl,shift,alt,meta});
const action=(id:ShortcutActionId,label:string,defaults:ShortcutBinding[],operation?:EditorShortcutOperation,repeat=false):ShortcutActionDefinition=>({id,label,scope:id.startsWith('workspace.')?'workspace':'editor',defaults,repeat,...(operation?{operation}:{})});
const command=(id:EditorAction,label:string,code:string,extra:{shift?:boolean;lineSpacing?:number;command?:string}={})=>action(`editor.${id}`,label,[bind(code,true,extra.shift)],{type:'command',command:extra.command??id,...(extra.lineSpacing?{lineSpacing:extra.lineSpacing}:{})});
const key=(id:EditorAction,label:string,code:string,ctrl=false,shift=false,repeat=false)=>action(`editor.${id}`,label,[bind(code,ctrl,shift)],{type:'key',key:code,ctrl,shift},repeat);
const navigation:readonly [NavigationName,string,string][]=[['left','ArrowLeft','Move left'],['right','ArrowRight','Move right'],['up','ArrowUp','Move up'],['down','ArrowDown','Move down'],['home','Home','Line start'],['end','End','Line end'],['pageUp','PageUp','Page up'],['pageDown','PageDown','Page down']];
export const shortcutActions:readonly ShortcutActionDefinition[]=[
 action('workspace.save','Save tab',[bind('KeyS',true)]),action('workspace.newTab','New tab',[bind('KeyT',true)]),action('workspace.closeTab','Close tab',[bind('KeyW',true)]),
 action('workspace.nextTab','Next tab',[bind('Tab',true)]),action('workspace.previousTab','Previous tab',[bind('Tab',true,true)]),action('workspace.detachTab','Move tab to a new window',[bind('KeyN',true,true)]),action('workspace.returnTab','Move tab to main window',[bind('KeyM',true,true)]),action('workspace.quit','Quit Novel Workspace',[bind('KeyQ',true,true)]),
 action('workspace.shortcuts','Keyboard and mouse shortcuts',[bind('F1')]),action('workspace.fullscreen','Toggle fullscreen',[bind('F11')]),action('workspace.commandPalette','Commands',[bind('KeyK',true)]),action('workspace.split','Toggle split drafts',[bind('Backslash',true,true)]),action('workspace.peek','Hold to peek',[bind('F6')]),action('workspace.peekChanges','Hold to show revision changes',[bind('F7')]),action('workspace.navigateBack','Back to previous place',[bind('ArrowLeft',false,false,true)]),action('workspace.navigateForward','Forward to next place',[bind('ArrowRight',false,false,true)]),
 ...Array.from({length:8},(_,index)=>action(`workspace.sequence${index+1}` as ShortcutActionId,`Run command sequence ${index+1}`,[])),
 action('editor.copy','Copy selection',[bind('KeyC',true)]),action('editor.cut','Cut selection',[bind('KeyX',true)]),action('editor.paste','Paste text',[bind('KeyV',true)]),
 command('bold','Bold','KeyB'),command('italic','Italic','KeyI'),command('underline','Underline','KeyU'),command('clearFormatting','Clear character formatting','Space'),action('editor.unicodeToggle','Convert Unicode code or character',[bind('KeyX',false,false,true)],{type:'command',command:'unicodeToggle'}),command('undo','Undo','KeyZ'),
 action('editor.redo','Redo',[bind('KeyY',true),bind('KeyZ',true,true)],{type:'command',command:'redo'}),command('selectAll','Select all manuscript text','KeyA'),
 command('singleSpacing','Single line spacing','Digit1',{command:'lineSpacing',lineSpacing:1}),command('oneAndHalfSpacing','One and a half line spacing','Digit5',{command:'lineSpacing',lineSpacing:1.5}),command('doubleSpacing','Double line spacing','Digit2',{command:'lineSpacing',lineSpacing:2}),
 action('editor.find','Find manuscript text',[bind('KeyF',true)]),action('editor.replace','Find and replace manuscript text',[bind('KeyH',true)]),action('editor.findNext','Find next match',[bind('F3')]),
 ...navigation.flatMap(([name,code,label])=>[false,true].flatMap(ctrl=>[false,true].map(shift=>action(`editor.${name}${ctrl?'.control':''}${shift?'.extend':''}` as ShortcutActionId,`${ctrl?'Control + ':''}${shift?'Extend selection: ':''}${label}`,[bind(code,ctrl,shift)],{type:'key',key:code,ctrl,shift},true)))),
 key('backspace','Delete previous character','Backspace',false,false,true),key('delete','Delete next character','Delete',false,false,true),key('deletePreviousWord','Delete previous word','Backspace',true,false,true),key('deleteNextWord','Delete next word','Delete',true,false,true),
 key('paragraphBreak','New paragraph','Enter',false,false,true),key('lineBreak','Line break','Enter',false,true,true),key('tab','Insert tab','Tab',false,false,true),key('backTab','Back tab','Tab',false,true,true),key('escape','Cancel selection or command','Escape'),
];
const definitions=new Map(shortcutActions.map(definition=>[definition.id,definition]));
const codePattern=/^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-9]|2[0-4])|Arrow(Left|Right|Up|Down)|Home|End|PageUp|PageDown|Enter|Tab|Escape|Backspace|Delete|Space|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Comma|Period|Slash|Numpad([0-9]|Enter|Add|Subtract|Multiply|Divide|Decimal))$/;
const plainControls=/^(F([1-9]|1[0-9]|2[0-4])|Arrow(Left|Right|Up|Down)|Home|End|PageUp|PageDown|Enter|NumpadEnter|Tab|Escape|Backspace|Delete)$/;
function object(value:unknown):value is Record<string,unknown>{return Boolean(value)&&typeof value==='object'&&!Array.isArray(value);}
function exactKeys(value:Record<string,unknown>,keys:readonly string[]){if(Object.keys(value).length!==keys.length||Object.keys(value).some(key=>!keys.includes(key)))throw new Error('Unsupported shortcut settings fields.');}
export function validateShortcutBinding(value:unknown):ShortcutBinding{
 if(!object(value))throw new Error('Invalid keyboard shortcut.');exactKeys(value,['code','ctrl','alt','shift','meta']);
 if(typeof value.code!=='string'||!codePattern.test(value.code)||['ctrl','alt','shift','meta'].some(key=>typeof value[key]!=='boolean'))throw new Error('Invalid keyboard shortcut.');
 if(!value.ctrl&&!value.alt&&!value.meta&&!plainControls.test(value.code))throw new Error('Use Ctrl, Alt or Meta for letters, numbers, punctuation and Space. Normal typing is reserved.');
 return {code:value.code,ctrl:value.ctrl as boolean,alt:value.alt as boolean,shift:value.shift as boolean,meta:value.meta as boolean};
}
function identity(binding:ShortcutBinding){return `${binding.code}:${Number(binding.ctrl)}${Number(binding.alt)}${Number(binding.shift)}${Number(binding.meta)}`;}
export interface ShortcutConflict {binding:ShortcutBinding;actions:ShortcutActionId[]}
export function shortcutConflicts(settings:ShortcutSettingsModel):ShortcutConflict[]{
 const assigned=new Map<string,ShortcutConflict>();
 for(const definition of shortcutActions)for(const binding of settings.bindings[definition.id]??[]){const id=identity(binding),previous=assigned.get(id);if(previous)previous.actions.push(definition.id);else assigned.set(id,{binding,actions:[definition.id]});}
 return [...assigned.values()].filter(conflict=>conflict.actions.length>1);
}
export function defaultShortcutSettings():ShortcutSettingsModel{return {schemaVersion:1,bindings:Object.fromEntries(shortcutActions.map(definition=>[definition.id,definition.defaults.map(binding=>({...binding}))])) as ShortcutSettingsModel['bindings'],mouse:{doubleClick:'word',tripleClick:'paragraph'}};}
export function parseShortcutSettings(value:unknown):ShortcutSettingsModel{
 if(!object(value))throw new Error('Invalid shortcut settings.');exactKeys(value,['schemaVersion','bindings','mouse']);if(value.schemaVersion!==1||!object(value.bindings)||!object(value.mouse))throw new Error('Unsupported shortcut settings version.');
 const introducedActions=new Set<ShortcutActionId>(['editor.findNext','editor.copy','editor.cut','editor.paste','workspace.split','workspace.peek','workspace.peekChanges',...Array.from({length:8},(_,index)=>`workspace.sequence${index+1}` as ShortcutActionId),'workspace.navigateBack','workspace.navigateForward','editor.unicodeToggle']);
 if(Object.keys(value.bindings).some(id=>!definitions.has(id as ShortcutActionId))||shortcutActions.some(definition=>!Object.hasOwn(value.bindings as object,definition.id)&&!introducedActions.has(definition.id)))throw new Error('Unsupported shortcut settings fields.');
 exactKeys(value.mouse,['doubleClick','tripleClick']);
 if(!['caret','word','paragraph'].includes(String(value.mouse.doubleClick))||!['caret','word','paragraph'].includes(String(value.mouse.tripleClick)))throw new Error('Unsupported mouse shortcut.');
 const bindings={} as ShortcutSettingsModel['bindings'];
 const occupied=new Set<string>();
 for(const definition of shortcutActions){if(!Object.hasOwn(value.bindings,definition.id))continue;const list=value.bindings[definition.id];if(!Array.isArray(list)||list.length>3)throw new Error('An action can have at most three keyboard shortcuts.');bindings[definition.id]=list.map(validateShortcutBinding);for(const binding of bindings[definition.id])occupied.add(identity(binding));}
 // Additive version-one actions preserve explicit bindings, including cleared ones.
 // A new default yields to any existing remap rather than creating a conflict.
 for(const definition of shortcutActions)if(!Object.hasOwn(bindings,definition.id)){bindings[definition.id]=definition.defaults.filter(binding=>!occupied.has(identity(binding))).map(binding=>({...binding}));for(const binding of bindings[definition.id])occupied.add(identity(binding));}
 const settings:ShortcutSettingsModel={schemaVersion:1,bindings,mouse:{doubleClick:value.mouse.doubleClick as ShortcutSettingsModel['mouse']['doubleClick'],tripleClick:value.mouse.tripleClick as ShortcutSettingsModel['mouse']['tripleClick']}};
 const conflicts=shortcutConflicts(settings);if(conflicts.length)throw new Error(`Shortcut conflict: ${formatShortcut(conflicts[0].binding)} is assigned to ${conflicts[0].actions.map(id=>definitions.get(id)!.label).join(' and ')}.`);return settings;
}
export function updateShortcutBindings(settings:ShortcutSettingsModel,id:ShortcutActionId,bindings:ShortcutBinding[]):ShortcutSettingsModel{if(!definitions.has(id))throw new Error('Unknown shortcut action.');return parseShortcutSettings({...settings,bindings:{...settings.bindings,[id]:bindings}});}
export interface ShortcutKeyEvent {code:string;key?:string;ctrlKey:boolean;altKey:boolean;shiftKey:boolean;metaKey:boolean;isComposing?:boolean;keyCode?:number;repeat?:boolean;defaultPrevented?:boolean;target?:EventTarget|null;getModifierState?:(key:'AltGraph')=>boolean;preventDefault?:()=>void}
function blocked(event:ShortcutKeyEvent){if(event.isComposing||event.keyCode===229||event.key==='Process'||event.key==='Dead')return true;try{return Boolean(event.getModifierState?.('AltGraph'));}catch{return true;}}
export function bindingFromEvent(event:ShortcutKeyEvent):ShortcutBinding|null{if(blocked(event)||event.repeat||!codePattern.test(event.code))return null;return validateShortcutBinding(bind(event.code,event.ctrlKey,event.shiftKey,event.altKey,event.metaKey));}
export function formatShortcut(binding:ShortcutBinding):string{
 const code=binding.code.replace(/^Key/,'').replace(/^Digit/,'').replace(/^Arrow/,'');return [binding.ctrl?'Ctrl':'',binding.alt?'Alt':'',binding.shift?'Shift':'',binding.meta?'Meta':'',code].filter(Boolean).join('+');
}
let cache:{settings:ShortcutSettingsModel;error:string|null}|undefined;
let observedWindow:Window|undefined;
function storage(){return typeof localStorage==='undefined'?undefined:localStorage;}
function read(){try{const raw=storage()?.getItem(SHORTCUT_STORAGE_KEY);if(!raw)return {settings:defaultShortcutSettings(),error:null};if(raw.length>40000)throw new Error('Shortcut settings exceed their size limit.');return {settings:parseShortcutSettings(JSON.parse(raw)),error:null};}catch(error){return {settings:defaultShortcutSettings(),error:'Saved shortcuts could not be read. Defaults are active. '+String(error)};}}
function observe(){if(typeof window==='undefined'||observedWindow===window)return;observedWindow=window;cache=undefined;window.addEventListener('storage',event=>{if(event.key===SHORTCUT_STORAGE_KEY||event.key===null)cache=undefined;});window.addEventListener(SHORTCUT_CHANGE_EVENT,()=>{cache=undefined;});}
export function reloadShortcutSettings(){observe();cache=read();return cache;}
export function getShortcutSettings():ShortcutSettingsModel{observe();cache??=read();return cache.settings;}
export function saveShortcutSettings(settings:ShortcutSettingsModel):void{const validated=parseShortcutSettings(settings),store=storage();if(!store)throw new Error('Shortcut settings cannot be saved in this window.');store.setItem(SHORTCUT_STORAGE_KEY,JSON.stringify(validated));cache={settings:validated,error:null};if(typeof window!=='undefined')window.dispatchEvent(new Event(SHORTCUT_CHANGE_EVENT));}
export function resetShortcutSettings():void{saveShortcutSettings(defaultShortcutSettings());}
export function useShortcutSettings(){const [state,setState]=useState(()=>reloadShortcutSettings());useEffect(()=>{if(typeof window==='undefined')return;const changed=()=>setState(reloadShortcutSettings());const stored=(event:StorageEvent)=>{if(event.key===SHORTCUT_STORAGE_KEY||event.key===null)changed();};window.addEventListener(SHORTCUT_CHANGE_EVENT,changed);window.addEventListener('storage',stored);return()=>{window.removeEventListener(SHORTCUT_CHANGE_EVENT,changed);window.removeEventListener('storage',stored);};},[]);return {...state,save:saveShortcutSettings,reset:resetShortcutSettings};}
function editable(event:ShortcutKeyEvent){const target=event.target as (EventTarget&{tagName?:string;isContentEditable?:boolean;closest?:(selector:string)=>unknown})|null|undefined;return Boolean(target&&(target.isContentEditable||['INPUT','TEXTAREA','SELECT'].includes(target.tagName??'')));}
function settingsTarget(event:ShortcutKeyEvent){try{return Boolean((event.target as Element|null)?.closest?.('[data-shortcut-settings]'));}catch{return true;}}
export function matchShortcut(event:ShortcutKeyEvent,scope:ShortcutScope,settings=getShortcutSettings()):ShortcutActionId|null{
 if(event.defaultPrevented||blocked(event)||settingsTarget(event)||!codePattern.test(event.code))return null;
 // A customized bare navigation/control key must not replace ordinary field editing.
 if(scope==='workspace'&&editable(event)&&!event.ctrlKey&&!event.altKey&&!event.metaKey&&!/^F\d+$/.test(event.code))return null;
 const wanted=identity(bind(event.code,event.ctrlKey,event.shiftKey,event.altKey,event.metaKey));
 for(const definition of shortcutActions)if(definition.scope===scope&&(!event.repeat||definition.repeat)&&settings.bindings[definition.id].some(binding=>identity(binding)===wanted))return definition.id;
 return null;
}
export function dispatchShortcut(event:ShortcutKeyEvent,scope:ShortcutScope,handlers:Partial<Record<ShortcutActionId,()=>void>>,settings=getShortcutSettings()):boolean{const id=matchShortcut(event,scope,settings);if(!id||!handlers[id])return false;event.preventDefault?.();handlers[id]!();return true;}
export function editorShortcutOperation(id:ShortcutActionId):EditorShortcutOperation|null{const operation=definitions.get(id)?.operation;return operation?{...operation}:null;}
export function mouseClickCount(clicks:number,settings=getShortcutSettings()):1|2|3{if(!Number.isInteger(clicks)||clicks<2)return 1;const selection=clicks===2?settings.mouse.doubleClick:settings.mouse.tripleClick;return selection==='word'?2:selection==='paragraph'?3:1;}
