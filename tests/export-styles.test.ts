import {describe,expect,it,vi} from 'vitest';
import {DEFAULT_EXPORT_PROFILE,EXPORT_PRESETS,loadExportProfile,saveExportProfile,validateExportProfile,type ExportProfile,type ExportProfileRpc} from '../apps/writer/src/export-styles';

const valid=():ExportProfile=>structuredClone(DEFAULT_EXPORT_PROFILE);
const rpcMock=(impl:(operation:string,args:Record<string,unknown>)=>unknown)=><T,>(operation:string,args:Record<string,unknown>)=>Promise.resolve(impl(operation,args) as T);

describe('saved Word export styles',()=>{
 it('uses the source-matching defaults and keeps the three presets exact',()=>{
  expect(DEFAULT_EXPORT_PROFILE).toMatchObject({pageSize:'letter',marginMm:25.4,lineSpacing:'source',body:{font:'Aptos',size:12,bold:false,italic:false}});
  expect(EXPORT_PRESETS.standard).toMatchObject({pageSize:'a4',marginMm:25.4,lineSpacing:'2',body:{font:'Times New Roman',size:12},chapter:{size:14,bold:true},scene:{size:12,bold:true}});
  expect(EXPORT_PRESETS.compact).toMatchObject({pageSize:'a4',marginMm:20,lineSpacing:'1.15',body:{font:'Aptos',size:12},title:{size:24},chapter:{size:18},scene:{size:14}});
  expect(EXPORT_PRESETS.current).toEqual(DEFAULT_EXPORT_PROFILE);
 });

 it('loads a default profile without writing it and detects competing saved heads',async()=>{
  const calls:string[]=[];
  const rpc=rpcMock((operation,args)=>{calls.push(operation);return operation==='load_project_data'?null:{revisionIds:[]};});
  const result=await loadExportProfile('p',rpc);
  expect(result).toEqual({snapshot:{revisionId:'',payload:DEFAULT_EXPORT_PROFILE},conflicted:false});
  expect(calls).toEqual(['load_project_data','project_data_heads']);
  const conflicting=await loadExportProfile('p',rpcMock(operation=>operation==='load_project_data'?{revisionId:'head',payload:valid()}:{revisionIds:['head','other']}));
  expect(conflicting.conflicted).toBe(true);
  expect(conflicting.snapshot.revisionId).toBe('head');
 });

 it('saves through project-data CAS with the exact expected parent',async()=>{
  const profile=valid();profile.body.font='Noto Serif';
  const calls:{operation:string;args:Record<string,unknown>}[]=[];
  const rpc:ExportProfileRpc=async <T>(operation:string,args:Record<string,unknown>)=>{calls.push({operation,args});return {revisionId:'new-head',payload:args.payload} as T;};
  const result=await saveExportProfile('p',profile,'old-head',rpc);
  expect(calls).toEqual([{operation:'save_project_data',args:{projectId:'p',kind:'exportProfile',payload:profile,parent:'old-head'}}]);
  expect(result).toEqual({revisionId:'new-head',payload:profile});
 });

 it('keeps save conflicts as failures so callers retain and can retry the draft',async()=>{
  const draft=valid();draft.body.size=13.5;
  const rpc=vi.fn(async()=>{throw new Error('The saved styles changed in another window.');});
  await expect(saveExportProfile('p',draft,'stale-head',rpc)).rejects.toThrow('changed in another window');
  expect(draft.body.size).toBe(13.5);
  expect(rpc).toHaveBeenCalledWith('save_project_data',{projectId:'p',kind:'exportProfile',payload:draft,parent:'stale-head'});
 });

 it('rejects unknown keys, invalid bounds, unsupported line spacing, and XML-unsafe font names',()=>{
  const unknown={...valid(),unknown:true};
  expect(()=>validateExportProfile(unknown)).toThrow('unsupported fields');
  const margin=valid();margin.marginMm=9.9;expect(()=>validateExportProfile(margin)).toThrow('between 10 and 40');
  const spacing=valid();spacing.lineSpacing='1.2' as ExportProfile['lineSpacing'];expect(()=>validateExportProfile(spacing)).toThrow('supported body line spacing');
  const font=valid();font.body.font='Bad\u0000Font';expect(()=>validateExportProfile(font)).toThrow('XML-safe');
  const c1=valid();c1.scene.font='A\u0085B';expect(()=>validateExportProfile(c1)).toThrow('XML-safe');
  const noncharacter=valid();noncharacter.body.font='A\uFFFF';expect(()=>validateExportProfile(noncharacter)).toThrow('XML-safe');
  const wrongSpacing=valid();wrongSpacing.lineSpacing=2 as unknown as string;expect(()=>validateExportProfile(wrongSpacing)).toThrow('supported body line spacing');
  const points=valid();points.title.size=12.25;expect(()=>validateExportProfile(points)).toThrow('half-point steps');
 });
});
