import {useEffect,useState} from 'react';

export interface DeskSettings {schemaVersion:1;outline:boolean;direction:'ltr'|'rtl';writingWidth:number;peek:'references'|'outline'}
export const DESK_STORAGE_KEY='novel-workspace.desk.v1';
const changedEvent='novel-desk-settings';
export const defaultDeskSettings=():DeskSettings=>({schemaVersion:1,outline:true,direction:'ltr',writingWidth:0,peek:'references'});
export function parseDeskSettings(value:unknown):DeskSettings {
 if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('Invalid desk settings.');
 const data=value as Record<string,unknown>;
 if(Object.keys(data).sort().join(',')!=='direction,outline,peek,schemaVersion,writingWidth'||data.schemaVersion!==1||typeof data.outline!=='boolean'||!['ltr','rtl'].includes(String(data.direction))||!['references','outline'].includes(String(data.peek))||typeof data.writingWidth!=='number'||!Number.isInteger(data.writingWidth)||(data.writingWidth!==0&&(data.writingWidth<360||data.writingWidth>1200)))throw new Error('Invalid desk settings.');
 return data as unknown as DeskSettings;
}
function read():DeskSettings {try{const raw=localStorage.getItem(DESK_STORAGE_KEY);return raw&&raw.length<2000?parseDeskSettings(JSON.parse(raw)):defaultDeskSettings();}catch{return defaultDeskSettings();}}
export function useDeskSettings(){
 const [settings,setSettings]=useState(read),[error,setError]=useState('');
 useEffect(()=>{const refresh=()=>setSettings(read());const stored=(event:StorageEvent)=>{if(event.key===DESK_STORAGE_KEY||event.key===null)refresh();};window.addEventListener(changedEvent,refresh);window.addEventListener('storage',stored);return()=>{window.removeEventListener(changedEvent,refresh);window.removeEventListener('storage',stored);};},[]);
 function save(next:DeskSettings){try{const valid=parseDeskSettings(next);localStorage.setItem(DESK_STORAGE_KEY,JSON.stringify(valid));setSettings(valid);setError('');window.dispatchEvent(new Event(changedEvent));}catch(reason){setError('Desk settings could not be saved. '+String(reason));}}
 return {settings,save,error};
}
