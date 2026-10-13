import React,{useEffect,useRef,useState} from 'react';
import {bindingFromEvent,defaultShortcutSettings,formatShortcut,shortcutActions,updateShortcutBindings,useShortcutSettings,type ShortcutActionId} from './shortcuts';
import './ShortcutSettings.css';
import {CommandSequences} from './CommandSequences';

export function ShortcutSettings({onClose}:{onClose?:()=>void}){
 const {settings,error,save,reset}=useShortcutSettings();
 const [selected,setSelected]=useState<ShortcutActionId>('workspace.save'),[capturing,setCapturing]=useState(false),[message,setMessage]=useState('');
 const capture=useRef<HTMLButtonElement>(null),dialog=useRef<HTMLElement>(null);
 useEffect(()=>{const previous=document.activeElement as HTMLElement|null;dialog.current?.querySelector<HTMLButtonElement>('button')?.focus();return()=>{if(previous?.isConnected)previous.focus();};},[]);
 function dialogKey(event:React.KeyboardEvent){
  if(capturing)return;
  if(event.code==='Escape'){event.preventDefault();event.stopPropagation();onClose?.();return;}
  if(event.code!=='Tab')return;
  const controls=[...(dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),summary,[tabindex="0"]')??[])].filter(element=>element.getClientRects().length>0);
  if(!controls.length)return;
  const first=controls[0],last=controls[controls.length-1],active=document.activeElement;
  if(event.shiftKey&&(active===first||!dialog.current?.contains(active))){event.preventDefault();last.focus();}
  else if(!event.shiftKey&&(active===last||!dialog.current?.contains(active))){event.preventDefault();first.focus();}
 }
 const definition=shortcutActions.find(action=>action.id===selected)!;
 function record(event:React.KeyboardEvent){
  if(!capturing)return;event.preventDefault();event.stopPropagation();
  if(event.code==='Escape'){setCapturing(false);setMessage('Shortcut capture cancelled.');return;}
  try{
   const binding=bindingFromEvent(event);if(!binding)return;
   save(updateShortcutBindings(settings,selected,[binding]));setCapturing(false);setMessage('Saved '+formatShortcut(binding)+' for '+definition.label+'.');
  }catch(error){setMessage(String(error));}
 }
 function change(action:()=>void){try{action();setCapturing(false);setMessage('Shortcut settings saved.');}catch(error){setMessage(String(error));}}
 return <section ref={dialog} className="shortcut-settings" role="dialog" aria-modal="true" data-shortcut-settings data-shortcut-capturing={capturing||undefined} onKeyDown={dialogKey} aria-label="Keyboard and mouse shortcuts">
  <header><h2>Keyboard and mouse shortcuts</h2>{onClose&&<button onClick={onClose} aria-label="Close shortcut settings">Close</button>}</header>
  <p>Shortcuts use physical key positions, including when you write with an Arabic keyboard layout. These settings apply to Novel Workspace on this computer.</p>
  {error&&<p role="alert" className="shortcut-message">{error}</p>}
  <label>Action<select value={selected} onChange={event=>{setSelected(event.target.value as ShortcutActionId);setCapturing(false);setMessage('');}}>{(['workspace','editor'] as const).map(scope=><optgroup key={scope} label={scope==='workspace'?'Tabs and windows':'Manuscript editing'}>{shortcutActions.filter(action=>action.scope===scope).map(action=><option key={action.id} value={action.id}>{action.label}</option>)}</optgroup>)}</select></label>
  <p><b>{definition.label}</b> · {definition.scope==='workspace'?'Workspace':'Manuscript editor'} · {settings.bindings[selected].map(formatShortcut).join(' or ')||'No keyboard shortcut'}</p>
  <div className="shortcut-buttons">
   <button ref={capture} onClick={()=>{setCapturing(true);setMessage('Press a key combination. Escape cancels.');capture.current?.focus();}} onKeyDown={record} aria-pressed={capturing}>{capturing?'Press shortcut…':'Set shortcut'}</button>
   <button disabled={capturing} onClick={()=>change(()=>save(updateShortcutBindings(settings,selected,[])))}>Clear action shortcut</button>
   <button disabled={capturing} onClick={()=>change(()=>save(updateShortcutBindings(settings,selected,defaultShortcutSettings().bindings[selected])))}>Reset action</button>
  </div>
  <p className="shortcut-hint">Setting a shortcut replaces this action’s current bindings. Conflicting combinations are refused. Clear the other action first. Letters, numbers and Space need Ctrl, Alt or Meta, so normal typing stays available.</p>
  {message&&<p role="status" className="shortcut-message">{message}</p>}
  <details><summary>All current keyboard shortcuts</summary><div className="shortcut-list">{shortcutActions.map(action=><div key={action.id}><button onClick={()=>{setSelected(action.id);setCapturing(false);setMessage('');}}>{action.label}</button><span>{settings.bindings[action.id].map(formatShortcut).join(' or ')||'None'}</span></div>)}</div></details>
  <fieldset><legend>Manuscript mouse selection</legend>{(['doubleClick','tripleClick'] as const).map(kind=><label key={kind}>{kind==='doubleClick'?'Double click':'Triple click'}<select value={settings.mouse[kind]} onChange={event=>change(()=>save({...settings,mouse:{...settings.mouse,[kind]:event.target.value as typeof settings.mouse.doubleClick}}))}><option value="caret">Place caret</option><option value="word">Select word</option><option value="paragraph">Select paragraph</option></select></label>)}</fieldset>
  <CommandSequences onClose={onClose}/>
  <button disabled={capturing} onClick={()=>change(reset)}>Reset all keyboard and mouse shortcuts</button>
 </section>;
}
