import React,{useLayoutEffect,useRef,useState} from 'react';

/** Native modal focus and Escape handling keep shortcuts out of the draft. */
export function ManuscriptDialog({title,initialName,chapterNumber,chapterCount,chapterNumberStart=1,onSave,onClose}:{title:string;initialName:string;chapterNumber?:number;chapterCount?:number;chapterNumberStart?:number;onSave:(name:string,number:number)=>Promise<void>;onClose:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null),submitting=useRef(false);
 const [name,setName]=useState(initialName),[number,setNumber]=useState(String(chapterNumber??1)),[saving,setSaving]=useState(false),[error,setError]=useState('');
 const start=chapterNumber===0&&Number(number)===1?1:chapterNumberStart;
 const maximum=chapterCount===undefined?undefined:chapterCount+start-1;
 useLayoutEffect(()=>{const node=dialog.current!;node.showModal();return()=>{node.close();};},[]);
 async function submit(event:React.FormEvent){
  event.preventDefault();if(submitting.current)return;
  submitting.current=true;setSaving(true);setError('');
  try{await onSave(name.trim(),Number(number));onClose();}catch(reason){setError(String(reason));}finally{submitting.current=false;setSaving(false);}
 }
 return <dialog ref={dialog} className="manuscript-dialog" aria-label={title} onCancel={event=>{event.preventDefault();if(!submitting.current)onClose();}} onKeyDown={event=>event.stopPropagation()}>
  <form onSubmit={event=>void submit(event)}>
   <h2>{title}</h2>
   <label>{chapterNumber===undefined?'Novel name':'Chapter name'}<input autoFocus required maxLength={500} dir="auto" value={name} onChange={event=>setName(event.target.value)} disabled={saving}/></label>
   {chapterNumber!==undefined&&<><label>Chapter number<input type="number" required min={0} max={maximum} step={1} value={number} onChange={event=>setNumber(event.target.value)} disabled={saving}/></label><p className="dialog-hint">Use 0 for a prologue. Changing the number renumbers the following chapters.</p></>}
   {error&&<p role="alert">{error}</p>}
   <div className="dialog-actions"><button type="button" disabled={saving} onClick={onClose}>Cancel</button><button type="submit" disabled={saving||!name.trim()}>{saving?'Saving…':chapterNumber===undefined?'Create novel':'Save chapter'}</button></div>
  </form>
 </dialog>;
}
