import {describe,expect,it} from 'vitest';
import {changedParagraphs,revisionExcerpt} from '../apps/writer/src/revision-changes';
const doc=(texts:string[])=>({type:'doc',content:texts.map(text=>({type:'paragraph',attrs:{dir:'auto'},content:[{type:'text',text}]}))});
describe('read-only paragraph comparisons',()=>{
 it('retains exact Arabic combining marks, mixed scripts and private Unicode',()=>{const before=doc(['يس مِ English 中文 \uE000']),after=doc(['يس مِم English 中文 \uE000']);const frozen=JSON.stringify([before,after]);expect(changedParagraphs(before,after)).toEqual([{index:0,before:'يس مِ English 中文 \uE000',after:'يس مِم English 中文 \uE000',formattingOnly:false}]);expect(JSON.stringify([before,after])).toBe(frozen);});
 it('reports additions, deletions and formatting without inventing text changes',()=>{const before=doc(['same','old']),after=doc(['same','new','extra']);after.content[0].attrs.dir='rtl';expect(changedParagraphs(before,after)).toEqual([{index:0,before:'same',after:'same',formattingOnly:true},{index:1,before:'old',after:'new',formattingOnly:false},{index:2,before:'',after:'extra',formattingOnly:false}]);expect(changedParagraphs(doc(['x','deleted']),doc(['x']))[0].after).toBe('');});
 it('never mutates documents and bounds displayed excerpts without splitting surrogate pairs',()=>{expect(changedParagraphs(doc(['stable']),doc(['stable']))).toEqual([]);expect(revisionExcerpt('😀a',1)).toEqual({text:'😀',truncated:true});});
});
