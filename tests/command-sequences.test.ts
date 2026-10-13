import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {COMMAND_SEQUENCE_STORAGE_KEY,cancelRecording,commandRecordingGeneration,commandSequenceState,dismissSequenceMessage,parseCommandSequences,playCommandSequence,recordCommand,saveRecording,sequenceActionFromOperation,startRecording,validateSequenceAction,type CommandSequenceExecutor} from '../apps/writer/src/command-sequences';
let stored:Map<string,string>;
beforeEach(()=>{stored=new Map();vi.stubGlobal('localStorage',{getItem:(key:string)=>stored.get(key)??null,setItem:(key:string,value:string)=>stored.set(key,value)});cancelRecording();dismissSequenceMessage();});
afterEach(()=>{cancelRecording();vi.unstubAllGlobals();});
function saved(){startRecording(1,'مِ / 中文');recordCommand({kind:'format',command:'bold'});recordCommand({kind:'spacing',value:1.5});saveRecording();}
describe('explicit bounded command sequences',()=>{
 it.each([{kind:'text',text:'private prose'},{kind:'format',command:'undo'},{kind:'format',command:'unicodeToggle'},{kind:'format',command:['bold']},{kind:'format',command:'bold',text:'prose'},{kind:'spacing',value:0},{kind:'paragraph',style:'heading',language:'unknown',direction:'rtl'}])('refuses unsupported command payload %j',value=>{expect(()=>validateSequenceAction(value)).toThrow();});
 it('strictly bounds unique slots and command counts without altering Unicode names',()=>{
  const sequence={slot:1,name:'مِ / 繁體 \uE000',actions:[{kind:'format',command:'bold'}]};
  expect(parseCommandSequences({schemaVersion:1,sequences:[sequence]})).toEqual({schemaVersion:1,sequences:[sequence]});
  for(const sequences of [[sequence,sequence],[{...sequence,slot:9}],[{...sequence,actions:[]}],[{...sequence,actions:Array(33).fill(sequence.actions[0])}]])expect(()=>parseCommandSequences({schemaVersion:1,sequences})).toThrow();
 });
 it('records only whitelisted acknowledged operations and never text, clipboard, edits, search or navigation',()=>{
  for(const operation of [{type:'text',text:'secret'},{type:'key',key:'Enter'},{type:'command',command:'paste'},{type:'command',command:'replace',query:'old',replacement:'new'},{type:'command',command:'unicodeToggle'},{type:'command',command:'undo'},{type:'command',command:'captureNavigation'}])expect(sequenceActionFromOperation(operation)).toBeNull();
  expect(sequenceActionFromOperation({type:'command',command:'bold'})).toEqual({kind:'format',command:'bold'});
  expect(sequenceActionFromOperation({type:'command',command:'blockStyle',style:'paragraph',language:'ar',direction:'rtl'})).toEqual({kind:'paragraph',style:'paragraph',language:'ar',direction:'rtl'});
 });
 it('caps recordings at32 and rejects late acknowledgments from a cancelled recording generation',()=>{
  startRecording(1,'Original');const old=commandRecordingGeneration();cancelRecording();startRecording(2,'New');
  expect(recordCommand({kind:'format',command:'bold'},old)).toBe(false);
  for(let i=0;i<32;i++)expect(recordCommand({kind:'format',command:'italic'})).toBe(true);
  expect(recordCommand({kind:'format',command:'bold'})).toBe(false);expect(commandSequenceState().recording?.actions).toHaveLength(32);
 });
 it('a storage failure preserves the recording and the previous saved preferences for retry',()=>{
  saved();const before=stored.get(COMMAND_SEQUENCE_STORAGE_KEY);startRecording(1,'Replacement');recordCommand({kind:'format',command:'underline'});
  vi.stubGlobal('localStorage',{getItem:(key:string)=>stored.get(key)??null,setItem:()=>{throw new Error('Storage full');}});
  expect(saveRecording).toThrow('Storage full');expect(commandSequenceState().recording?.name).toBe('Replacement');expect(stored.get(COMMAND_SEQUENCE_STORAGE_KEY)).toBe(before);
 });
 it('plays accepted commands sequentially with a save barrier before playback and after each step',async()=>{
  saved();const order:string[]=[];const executor:CommandSequenceExecutor={begin:async()=>{order.push('begin');},flush:async()=>{order.push('save');},execute:async action=>{order.push(action.kind);expect(recordCommand({kind:'format',command:'italic'})).toBe(false);},end:()=>{order.push('end');}};
  await playCommandSequence(1,executor);expect(order).toEqual(['begin','save','format','save','spacing','save','end']);expect(commandSequenceState().playback).toBeNull();expect(commandSequenceState().settings.sequences[0].actions).toHaveLength(2);
 });
 it.each(['Finish entering the current word','CAS conflict'])('stops before the next command after a failed barrier: %s',async message=>{
  saved();let saves=0;const execute=vi.fn(async()=>{}),end=vi.fn();
  await expect(playCommandSequence(1,{begin:async()=>{},flush:async()=>{if(++saves===2)throw new Error(message);},execute,end})).rejects.toThrow(message);
  expect(execute).toHaveBeenCalledTimes(1);expect(end).toHaveBeenCalledTimes(1);expect(commandSequenceState().error).toContain(message);expect(commandSequenceState().playback).toBeNull();
 });
 it('refuses recursive playback and leaves the first recording immutable during playback',async()=>{
  saved();const before=stored.get(COMMAND_SEQUENCE_STORAGE_KEY),executor:CommandSequenceExecutor={begin:async()=>{},flush:async()=>{},execute:async()=>{await expect(playCommandSequence(1,executor)).rejects.toThrow('Finish');expect(()=>startRecording(2,'Nested')).toThrow('Finish');},end:()=>{}};
  await playCommandSequence(1,executor);expect(stored.get(COMMAND_SEQUENCE_STORAGE_KEY)).toBe(before);
 });
});
