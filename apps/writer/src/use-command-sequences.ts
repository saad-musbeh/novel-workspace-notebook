import {useEffect,useRef,useState,type RefObject} from 'react';
import {dispatchShortcut,type ShortcutActionId} from './shortcuts';
import {commandSequenceState,playCommandSequence,subscribeCommandSequences,type CommandSequenceExecutor} from './command-sequences';
export function useCommandSequenceState(){const [state,setState]=useState(commandSequenceState);useEffect(()=>subscribeCommandSequences(()=>setState(commandSequenceState())),[]);return state;}
export function useCommandSequenceExecutor(executor:CommandSequenceExecutor,keyboardActive:boolean,element:RefObject<HTMLElement|null>){
 const latest=useRef(executor);latest.current=executor;const [error,setError]=useState('');
 useEffect(()=>{
  if(!keyboardActive)return;
  const handlers=Object.fromEntries(Array.from({length:8},(_,i)=>['workspace.sequence'+(i+1),()=>{setError('');void playCommandSequence(i+1,latest.current).catch(reason=>setError(String(reason)));}])) as Partial<Record<ShortcutActionId,()=>void>>;
  const key=(event:KeyboardEvent)=>{
   if(!element.current?.contains(event.target as Node))return;
   dispatchShortcut(event,'workspace',handlers);
  };
  window.addEventListener('keydown',key);return()=>window.removeEventListener('keydown',key);
 },[keyboardActive,element]);return {error};
}
