export type SequenceFormat='bold'|'italic'|'underline'|'clearFormatting'|'ltr'|'rtl';
export type CommandSequenceAction={kind:'format';command:SequenceFormat}|{kind:'spacing';value:1|1.15|1.5|2}|{kind:'paragraph';style:'paragraph'|'heading';language:'en'|'ar'|'zh-Hans'|'zh-Hant'|'und';direction:'auto'|'ltr'|'rtl'};
export interface CommandSequence {slot:number;name:string;actions:CommandSequenceAction[]}
export interface CommandSequenceSettings {schemaVersion:1;sequences:CommandSequence[]}
export interface CommandSequenceExecutor {begin():Promise<void>;flush():Promise<void>;execute(action:CommandSequenceAction):Promise<void>;end():void}
export const COMMAND_SEQUENCE_STORAGE_KEY='novel-workspace.command-sequences.v1';
export const COMMAND_SEQUENCE_CHANGE_EVENT='novel-command-sequences-changed';
const formats=['bold','italic','underline','clearFormatting','ltr','rtl'];
const languages=['en','ar','zh-Hans','zh-Hant','und'];
function object(value:unknown):value is Record<string,unknown>{return !!value&&typeof value==='object'&&!Array.isArray(value);}
function fields(value:Record<string,unknown>,names:string[]){if(Object.keys(value).length!==names.length||Object.keys(value).some(key=>!names.includes(key)))throw new Error('Unsupported command sequence fields.');}
export function validateSequenceAction(value:unknown):CommandSequenceAction{
 if(!object(value))throw new Error('Invalid sequence command.');
 if(value.kind==='format'){fields(value,['kind','command']);if(!formats.includes(value.command as string))throw new Error('Only supported formatting commands can be recorded.');}
 else if(value.kind==='spacing'){fields(value,['kind','value']);if(![1,1.15,1.5,2].includes(value.value as number))throw new Error('Unsupported line spacing.');}
 else if(value.kind==='paragraph'){fields(value,['kind','style','language','direction']);if(!['paragraph','heading'].includes(value.style as string)||!languages.includes(value.language as string)||!['auto','ltr','rtl'].includes(value.direction as string))throw new Error('Unsupported paragraph formatting.');}
 else throw new Error('Text, clipboard and navigation commands cannot be recorded.');
 return {...value} as CommandSequenceAction;
}
export function parseCommandSequences(value:unknown):CommandSequenceSettings{
 if(!object(value))throw new Error('Invalid command sequences.');fields(value,['schemaVersion','sequences']);
 if(value.schemaVersion!==1||!Array.isArray(value.sequences)||value.sequences.length>8)throw new Error('At most eight command sequences are supported.');
 const seen=new Set<number>();const sequences=value.sequences.map(item=>{
  if(!object(item))throw new Error('Invalid command sequence.');fields(item,['slot','name','actions']);
  if(!Number.isInteger(item.slot)||(item.slot as number)<1||(item.slot as number)>8||seen.has(item.slot as number))throw new Error('Invalid or duplicate sequence slot.');
  if(typeof item.name!=='string'||!item.name.trim()||item.name.length>80)throw new Error('Use a sequence name of at most 80 characters.');
  if(!Array.isArray(item.actions)||item.actions.length<1||item.actions.length>32)throw new Error('A sequence needs between one and 32 commands.');
  seen.add(item.slot as number);return {slot:item.slot as number,name:item.name,actions:item.actions.map(validateSequenceAction)};
 });return {schemaVersion:1,sequences};
}
export function sequenceActionLabel(action:CommandSequenceAction):string{
 if(action.kind==='format')return ({bold:'Bold',italic:'Italic',underline:'Underline',clearFormatting:'Clear character formatting',ltr:'Left to right',rtl:'Right to left'})[action.command];
 if(action.kind==='spacing')return 'Line spacing '+action.value;
 return (action.style==='heading'?'Heading':'Paragraph')+' · '+action.language+' · '+action.direction;
}
export function sequenceActionFromOperation(value:Record<string,unknown>):CommandSequenceAction|null{
 try{
  if(value.type!=='command')return null;
  if(formats.includes(String(value.command)))return validateSequenceAction({kind:'format',command:value.command});
  if(value.command==='lineSpacing')return validateSequenceAction({kind:'spacing',value:value.lineSpacing});
  if(value.command==='blockStyle')return validateSequenceAction({kind:'paragraph',style:value.style,language:value.language,direction:value.direction});
 }catch{}return null;
}
let recording:CommandSequence|null=null,playback:{slot:number;name:string;step:number;total:number}|null=null,message='',error='',generation=0;
const listeners=new Set<()=>void>();
function notify(){for(const listener of listeners)listener();if(typeof window!=='undefined')window.dispatchEvent(new Event(COMMAND_SEQUENCE_CHANGE_EVENT));}
function read():CommandSequenceSettings{
 const raw=typeof localStorage==='undefined'?null:localStorage.getItem(COMMAND_SEQUENCE_STORAGE_KEY);
 if(!raw)return {schemaVersion:1,sequences:[]};if(raw.length>65536)throw new Error('Saved command sequences exceed their size limit.');return parseCommandSequences(JSON.parse(raw));
}
export function commandSequenceState(){let settings:CommandSequenceSettings={schemaVersion:1,sequences:[]},readError='';try{settings=read();}catch(reason){readError=String(reason);}return {settings,recording:recording?structuredClone(recording):null,playback:playback?{...playback}:null,message,error:error||readError};}
export function subscribeCommandSequences(listener:()=>void){listeners.add(listener);const stored=(event:StorageEvent)=>{if(event.key===COMMAND_SEQUENCE_STORAGE_KEY||event.key===null)listener();};if(typeof window!=='undefined')window.addEventListener('storage',stored);return()=>{listeners.delete(listener);if(typeof window!=='undefined')window.removeEventListener('storage',stored);};}
export function startRecording(slot:number,name:string){
 if(recording||playback)throw new Error('Finish the current recording or playback first.');
 parseCommandSequences({schemaVersion:1,sequences:[{slot,name,actions:[{kind:'format',command:'bold'}]}]});
 recording={slot,name,actions:[]};generation++;message='Recording formatting commands';error='';notify();
}
export function commandRecordingGeneration():number|null{return recording&&!playback?generation:null;}
/** Called only after an explicit native command has been acknowledged. */
export function recordCommand(action:CommandSequenceAction,acceptedGeneration:number|null=commandRecordingGeneration()):boolean{
 if(!recording||playback||acceptedGeneration!==generation)return false;let validated:CommandSequenceAction;try{validated=validateSequenceAction(action);}catch{return false;}
 if(recording.actions.length===32){message='32-command limit reached. Save or cancel the recording.';notify();return false;}
 recording.actions.push(validated);notify();return true;
}
function write(settings:CommandSequenceSettings){const validated=parseCommandSequences(settings);if(typeof localStorage==='undefined')throw new Error('Command sequences cannot be saved in this window.');localStorage.setItem(COMMAND_SEQUENCE_STORAGE_KEY,JSON.stringify(validated));}
export function saveRecording(){
 if(!recording)throw new Error('No sequence is being recorded.');const settings=read(),sequence=structuredClone(recording);
 write({schemaVersion:1,sequences:[...settings.sequences.filter(item=>item.slot!==sequence.slot),sequence].sort((a,b)=>a.slot-b.slot)});
 recording=null;message='Sequence saved';error='';notify();
}
export function cancelRecording(){recording=null;message='Recording cancelled';error='';notify();}
export function removeCommandSequence(slot:number){if(recording||playback)throw new Error('Finish the current recording or playback first.');const settings=read();write({...settings,sequences:settings.sequences.filter(item=>item.slot!==slot)});message='Sequence removed';error='';notify();}
export function dismissSequenceMessage(){message='';error='';notify();}
export async function playCommandSequence(slot:number,executor:CommandSequenceExecutor):Promise<void>{
 if(recording||playback)throw new Error('Finish the current recording or playback first.');
 const sequence=read().sequences.find(item=>item.slot===slot);if(!sequence){error='No sequence is saved in slot '+slot+'.';notify();throw new Error(error);}
 playback={slot,name:sequence.name,step:0,total:sequence.actions.length};message='';error='';notify();
 try{
  await executor.begin();await executor.flush();
  for(const [index,action] of sequence.actions.entries()){await executor.execute(action);await executor.flush();playback={slot,name:sequence.name,step:index+1,total:sequence.actions.length};notify();}
  message='Sequence completed';
 }catch(reason){error='Sequence stopped after '+(playback?.step??0)+' saved commands. '+String(reason);throw reason;}
 finally{try{executor.end();}finally{playback=null;notify();}}
}
