import {useEffect,useRef,useState} from 'react';
import {matchShortcut,type ShortcutActionId} from './shortcuts';

/** A peek never moves focus, changes a document or consumes composing input. */
export function useHoldToPeek(enabled:boolean,action:ShortcutActionId='workspace.peek'){
 const [held,setHeld]=useState(false),code=useRef<string|null>(null);
 useEffect(()=>{
  if(!enabled){code.current=null;setHeld(false);return;}
  const release=()=>{code.current=null;setHeld(false);};
  const down=(event:KeyboardEvent)=>{if(event.key==='Escape'){release();return;}if(code.current||!matchShortcut(event,'workspace')||matchShortcut(event,'workspace')!==action)return;event.preventDefault();code.current=event.code;setHeld(true);};
  const up=(event:KeyboardEvent)=>{if(code.current===event.code)release();};
  const visible=()=>{if(document.visibilityState==='hidden')release();};
  window.addEventListener('keydown',down);window.addEventListener('keyup',up);window.addEventListener('blur',release);document.addEventListener('visibilitychange',visible);
  return()=>{window.removeEventListener('keydown',down);window.removeEventListener('keyup',up);window.removeEventListener('blur',release);document.removeEventListener('visibilitychange',visible);code.current=null;};
 },[enabled,action]);
 return held&&enabled;
}
