import {useEffect,useRef,useState} from 'react';
/** Keep whole-scene segmentation away from the editor's input and paint thread. */
export function useCharacterCount(text:string):number|null {
 const [count,setCount]=useState<number|null>(null);
 const worker=useRef<Worker|null>(null),request=useRef(0);
 useEffect(()=>{
  let instance:Worker;
  try{instance=new Worker(new URL('./grapheme-worker.ts',import.meta.url),{type:'module'});}
  catch{setCount(-1);return;}
  worker.current=instance;
  instance.onmessage=(event:MessageEvent<{id:number;count:number}>)=>{
   if(event.data.id===request.current)setCount(event.data.count);
  };
  instance.onerror=()=>setCount(-1);
  return()=>{instance.terminate();worker.current=null;};
 },[]);
 useEffect(()=>{worker.current?.postMessage({id:++request.current,text});},[text]);
 return count;
}
