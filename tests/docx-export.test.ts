import {describe,expect,it,vi} from 'vitest';
import {acknowledgeWorkspaceExportSave,exportDocx} from '../apps/writer/src/docx-export';

describe('Word manuscript export',()=>{
 it('drains project windows before invoking the native save dialog with the expected contract',async()=>{
  const order:string[]=[];
  const drain=vi.fn(async(projectId:string)=>{order.push('drain:'+projectId);});
  const invoke=vi.fn(async(command:string,args:{projectId:string;suggestedName:string;profileRevisionId:string|null})=>{order.push('invoke:'+command);return {cancelled:false,name:'draft.docx'};});
  const message=await exportDocx('project-1',invoke,drain,'style-revision-1');
  expect(order).toEqual(['drain:project-1','invoke:export_docx']);
  expect(invoke).toHaveBeenCalledWith('export_docx',{projectId:'project-1',suggestedName:'manuscript.docx',profileRevisionId:'style-revision-1'});
  expect(message).toContain('draft.docx');
  expect(message).toContain('manuscript text and formatting');
  expect(message).toContain('planning, revision history, and custom font assets remain in the project archive');
 });

 it('does not claim success when the save dialog is cancelled',async()=>{
  const invoke=vi.fn(async()=>({cancelled:true}));
  await expect(exportDocx('p',invoke)).resolves.toBeNull();
 });

 it('propagates native export failures for the workspace error handler',async()=>{
  const failure=new Error('DOCX conversion failed');
  const invoke=vi.fn(async()=>{throw failure;});
  await expect(exportDocx('p',invoke)).rejects.toBe(failure);
 });

 it('does not invoke native export when draining project windows fails',async()=>{
  const failure=new Error('A project window could not save its draft');
  const drain=vi.fn(async()=>{throw failure;});
  const invoke=vi.fn(async()=>({cancelled:false}));
  await expect(exportDocx('p',invoke,drain)).rejects.toBe(failure);
  expect(drain).toHaveBeenCalledWith('p');
  expect(invoke).not.toHaveBeenCalled();
 });

 it('acknowledges only after every matching local tab has drained',async()=>{
  const order:string[]=[];
  await acknowledgeWorkspaceExportSave({requestId:7,projectId:'p'},[{id:'a'},{id:'b'}],{
   busy:false,isIncoming:()=>false,
   drain:async id=>{order.push('drain:'+id);},
   acknowledge:async result=>{order.push('ack');expect(result).toEqual({requestId:7,error:null});},
  });
  expect(order).toEqual(['drain:a','drain:b','ack']);
 });

 it('reports a drain failure and refuses busy, transferring, or missing project tabs',async()=>{
  const failure=vi.fn(),drain=vi.fn(async(id:string)=>{if(id==='bad')throw new Error('save failed');});
  const ack=vi.fn(async(result:{requestId:number;error:string|null})=>{failure(result);});
  await acknowledgeWorkspaceExportSave({requestId:8,projectId:'p'},[{id:'bad'}],{busy:false,isIncoming:()=>false,drain,acknowledge:ack});
  expect(failure).toHaveBeenLastCalledWith({requestId:8,error:'Error: save failed'});
  await acknowledgeWorkspaceExportSave({requestId:9,projectId:'p'},[{id:'busy'}],{busy:true,isIncoming:()=>false,drain,acknowledge:ack});
  expect(failure).toHaveBeenLastCalledWith({requestId:9,error:'Error: This window is busy with another workspace operation.'});
  await acknowledgeWorkspaceExportSave({requestId:10,projectId:'p'},[{id:'incoming'}],{busy:false,isIncoming:id=>id==='incoming',drain,acknowledge:ack});
  expect(failure).toHaveBeenLastCalledWith({requestId:10,error:'Error: A project tab is still transferring into this window.'});
  await acknowledgeWorkspaceExportSave({requestId:11,projectId:'p'},[],{busy:false,isIncoming:()=>false,drain,acknowledge:ack});
  expect(failure).toHaveBeenLastCalledWith({requestId:11,error:'Error: This window has no ready tab for the requested project.'});
 });
});
