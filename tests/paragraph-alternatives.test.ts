import {describe,it,expect,vi} from 'vitest';
import {alternativeWithText,validateParagraphAlternatives,type ParagraphAlternativesDocument} from '../apps/writer/src/paragraph-alternatives';
import {plainText} from '../apps/writer/src/document';
import {ProjectDataCell} from '../apps/writer/src/assistance/persistence';
const block={type:'heading',attrs:{level:2,lang:'ar',dir:'rtl',lineSpacing:1.5},content:[{type:'text',text:'مِ 中文 ',marks:[{type:'bold'}]},{type:'text',text:'👩🏽‍🚀 e\u0301',marks:[{type:'italic'},{type:'underline'}]}]};
function document():ParagraphAlternativesDocument{return {schemaVersion:1,scenes:[{sceneId:'scene',groups:[{id:'group',label:'Paragraph1',anchor:{blockIndex:0,documentHash:'a'.repeat(64),block:structuredClone(block)},sceneRevisionId:'scene-r1',originalId:'original',activeId:'original',versions:[{id:'original',label:'Original',block:structuredClone(block)}]}]}]};}
describe('exact durable paragraph alternatives',()=>{
 it('keeps untouched rich blocks byte-for-byte and does not mutate the original',()=>{
  const frozen=JSON.stringify(block);expect(alternativeWithText(block,plainText(block))).toEqual(block);
  const alternative=alternativeWithText(block,'مِ 中文 different 👩🏽‍🚀 e\u0301');
  expect(alternative.attrs).toEqual(block.attrs);expect(plainText(alternative)).toBe('مِ 中文 different 👩🏽‍🚀 e\u0301');expect(alternative.content?.at(-1)?.marks).toEqual(block.content[1].marks);expect(JSON.stringify(block)).toBe(frozen);
 });
 it('replaces emoji without surrogate damage and preserves Arabic kasra and hard breaks',()=>{
  const original={type:'paragraph',attrs:{lang:'ar',dir:'rtl'},content:[{type:'text',text:'مِ 👩‍💻',marks:[{type:'bold'}]},{type:'hardBreak'},{type:'text',text:'繁體'}]};
  const edited=alternativeWithText(original,'مِ 👩‍🚀\n繁體');expect(plainText(edited)).toBe('مِ 👩‍🚀\n繁體');expect(edited.content?.filter(node=>node.type==='hardBreak')).toHaveLength(1);expect(JSON.stringify(edited)).not.toMatch(/\\ud[89ab][0-9a-f]{2}/i);
 });
 it('validates anchors, original membership, bounded counts and supported rich block structure',()=>{
  expect(()=>validateParagraphAlternatives(document())).not.toThrow();
  for(const mutate of [(d:ParagraphAlternativesDocument)=>{d.scenes[0].groups[0].anchor.blockIndex=-1;},(d:ParagraphAlternativesDocument)=>{d.scenes[0].groups[0].anchor.documentHash='bad';},(d:ParagraphAlternativesDocument)=>{d.scenes[0].groups[0].originalId='missing';},(d:ParagraphAlternativesDocument)=>{d.scenes[0].groups[0].versions[0].block.type='doc';}]){const d=document();mutate(d);expect(()=>validateParagraphAlternatives(d)).toThrow();}
 });
 it('persists original and alternative as separate exact versions through projectData CAS',async()=>{
  let stored=document(),parent='metadata-r1';const rpc=vi.fn(async(op:string,args:any)=>{if(op==='load_project_data')return {payload:stored,revisionId:parent};expect(args.parent).toBe(parent);expect(args.kind).toBe('paragraphAlternatives');stored=structuredClone(args.payload);parent='metadata-r2';return {payload:stored,revisionId:parent};});
  const cell=new ProjectDataCell(rpc as any,'project','paragraphAlternatives',document());await cell.load();
  await cell.update(value=>{value.scenes[0].groups[0].versions.push({id:'alternative',label:'Alternative',block:alternativeWithText(block,'بديل مِ 中文')});return value;});await cell.drain();
  expect(stored.scenes[0].groups[0].versions[0].block).toEqual(block);expect(plainText(stored.scenes[0].groups[0].versions[1].block)).toBe('بديل مِ 中文');expect(rpc).toHaveBeenCalledTimes(2);
 });
 it('retains a failed CAS transformation and refuses close drain without overwriting confirmed versions',async()=>{
  const original=document(),rpc=vi.fn(async(op:string)=>{if(op==='load_project_data')return {revisionId:'metadata-r1',payload:original};throw new Error('Conflict: alternatives changed in another window');});
  const cell=new ProjectDataCell(rpc as any,'project','paragraphAlternatives',document());await cell.load();
  await expect(cell.update(value=>{value.scenes[0].groups[0].versions.push({id:'candidate',label:'Pending',block:alternativeWithText(block,'Pending مِ')});return value;})).rejects.toThrow('Conflict');
  await expect(cell.drain()).rejects.toThrow('Conflict');expect(cell.current()).toEqual(original);
  rpc.mockImplementation(async(op:string,args?:any)=>op==='save_project_data'?{revisionId:'metadata-r2',payload:args.payload}:{revisionId:'metadata-r1',payload:original});
  await cell.retryFailed();await cell.drain();expect(cell.current().scenes[0].groups[0].versions.map(version=>version.id)).toEqual(['original','candidate']);
 });
});
