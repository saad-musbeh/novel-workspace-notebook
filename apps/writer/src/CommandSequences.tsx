import React,{useState} from 'react';
import {cancelRecording,dismissSequenceMessage,removeCommandSequence,saveRecording,sequenceActionLabel,startRecording} from './command-sequences';
import {useCommandSequenceState} from './use-command-sequences';
import './CommandSequences.css';
export function CommandSequences({onClose}:{onClose?:()=>void}){
 const state=useCommandSequenceState(),[slot,setSlot]=useState(1),[name,setName]=useState(''),[error,setError]=useState('');
 const selected=state.settings.sequences.find(sequence=>sequence.slot===slot);
 function act(action:()=>void){try{action();setError('');}catch(reason){setError(String(reason));}}
 return <details className="command-sequences"><summary>Command sequences</summary><p>Record formatting, paragraph direction and line spacing. Eight sequences, up to 32 commands each. Text and clipboard contents are excluded.</p>
  <label>Sequence<select value={slot} onChange={event=>{setSlot(Number(event.target.value));setName('');}}>{Array.from({length:8},(_,i)=><option key={i} value={i+1}>{i+1}{state.settings.sequences.find(item=>item.slot===i+1)?.name?' · '+state.settings.sequences.find(item=>item.slot===i+1)!.name:''}</option>)}</select></label>
  <label>Name<input value={name} maxLength={80} onChange={event=>setName(event.target.value)} placeholder={selected?.name??'Sequence name'}/></label>
  <button disabled={!!state.recording||!!state.playback} onClick={()=>act(()=>{startRecording(slot,name||selected?.name||'Sequence '+slot);onClose?.();})}>{selected?'Record replacement':'Record'}</button>
  <button disabled={!selected||!!state.recording||!!state.playback} onClick={()=>act(()=>removeCommandSequence(slot))}>Remove</button>
  <p>Assign “Command sequence {slot}” in the shortcut action list, then play it from the active draft.</p>
  {selected&&<ol>{selected.actions.map((action,i)=><li key={i}>{sequenceActionLabel(action)}</li>)}</ol>}
  {(error||state.error)&&<p role="alert">{error||state.error}</p>}
 </details>;
}
export function CommandSequenceBanner(){
 const state=useCommandSequenceState(),[error,setError]=useState('');
 if(!state.recording&&!state.playback&&!state.message&&!state.error&&!error)return null;
 function save(){try{saveRecording();setError('');}catch(reason){setError(String(reason));}}
 return <aside className="command-sequence-banner" aria-label="Command sequence" aria-live="polite">
  {state.recording?<><span>Recording {state.recording.name} · {state.recording.actions.length}/32 commands</span><button disabled={!state.recording.actions.length} onClick={save}>Save sequence</button><button onClick={()=>{cancelRecording();setError('');}}>Cancel recording</button></>:state.playback?<span>Playing {state.playback.name} · {state.playback.step}/{state.playback.total} saved</span>:<><span>{state.message}</span><button onClick={()=>{dismissSequenceMessage();setError('');}}>Dismiss</button></>}
  {(error||state.error)&&<span role="alert">{error||state.error}</span>}
 </aside>;
}
