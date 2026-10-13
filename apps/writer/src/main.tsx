import React,{useEffect,useRef,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {StartupBoundary} from './StartupBoundary';
import {DesktopWorkspace} from './DesktopWorkspace';
import {CommandSequenceBanner} from './CommandSequences';
import {commandSequenceState} from './command-sequences';
import {WorkspaceTabs} from './WorkspaceTabs';
import {Writer} from './Writer';
import {createRepository} from './storage';
import type {Snapshot} from './document';
import './style.css';
const native='__TAURI_INTERNALS__' in window;
const repository=createRepository();
function App(){
 const flushRef=useRef<(()=>Promise<void>)|null>(null);const [closing,setClosing]=useState(false);const [error,setError]=useState(''),[probe,setProbe]=useState<boolean|null>(native?null:false),[workspaceProbe,setWorkspaceProbe]=useState(false),[closeProbe,setCloseProbe]=useState(false),[snapshot,setSnapshot]=useState<Snapshot|null>(null);
 useEffect(()=>{
  if(!native)return;
  let disposed=false,busy=false;const unsubscribe:(()=>void)[]=[];
  void(async()=>{
   const {windowLabel}=await invoke<{windowLabel:string}>('workspace_window_state');
   const target={target:windowLabel};
   const off=await listen<{wholeApp?:boolean}>('save-before-close',event=>{
    if(busy)return;busy=true;setClosing(true);setError('');
    void(async()=>{
     if(commandSequenceState().recording)throw new Error('Save or cancel the command recording before closing.');await flushRef.current?.();await invoke('complete_close');
     if(!event.payload?.wholeApp)setClosing(false);
    })().catch(async reason=>{
     setError('Could not close safely. '+String(reason)+' Your window stays open.');
     await invoke('workspace_cancel_close').catch(()=>{});setClosing(false);
    }).finally(()=>{busy=false;});
   },target);if(disposed){off();return;}unsubscribe.push(off);
   const cancel=await listen('workspace-close-cancelled',()=>{busy=false;setClosing(false);},target);
   if(disposed){cancel();return;}unsubscribe.push(cancel);
   const lifecycle=await listen<{error:string}>('workspace-lifecycle-error',event=>setError(event.payload.error),target);
   if(disposed){lifecycle();return;}unsubscribe.push(lifecycle);
   await invoke('enable_close_guard');
  })().catch(reason=>setError(String(reason)));
  return()=>{disposed=true;unsubscribe.forEach(off=>off());};
 },[]);
 useEffect(()=>{void(async()=>{const enabled=native&&await invoke<boolean>('performance_probe_enabled');setWorkspaceProbe(native&&await invoke<boolean>('workspace_probe_enabled'));setProbe(enabled);setCloseProbe(native&&await invoke<boolean>('close_probe_enabled'));if(enabled||!native){const loaded=await repository.load();const longProbe=enabled&&await invoke<boolean>('long_writing_probe_enabled');if(enabled)loaded.scene={schemaVersion:1,title:'Synthetic performance test',document:{type:'doc',content:longProbe?[{type:'paragraph',attrs:{dir:'auto',lang:'en'},content:[{type:'text',text:'Long synthetic text مِمِ Chinese 中文 English. '.repeat(1200)}]}]:Array.from({length:20},(_,i)=>({type:'paragraph',attrs:{dir:'rtl',lang:'ar'},content:[{type:'text',text:('سلام محمد مِمِمِ '+i+' ').repeat(20)}]}))}};setSnapshot(loaded);}})().catch(e=>setError(String(e)));},[]);
 return <StartupBoundary><div inert={closing}>{native&&<CommandSequenceBanner/>}{error&&<div className="save-error" role="alert">{error}</div>}{probe===null?<p className="loading">Opening local workspace…</p>:native&&(!probe||workspaceProbe)?workspaceProbe?<DesktopWorkspace flushRef={flushRef} debugProbe/>:<WorkspaceTabs flushRef={flushRef}/>:snapshot?<div className="app workspace-writing"><aside className="sidebar"><div className="brand">Novel Workspace</div><p>{probe?'Isolated synthetic test':'Read-only browser preview'}</p></aside><main><Writer initial={snapshot} repository={repository} flushRef={flushRef} onTitle={()=>{}} performanceProbe={Boolean(probe)} closeProbe={closeProbe}/></main></div>:<p className="loading">Opening saved scene…</p>}</div></StartupBoundary>;
}
createRoot(document.getElementById('root')!).render(<App/>);
