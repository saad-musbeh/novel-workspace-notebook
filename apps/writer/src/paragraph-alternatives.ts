import {plainText,validateScene,type DocumentNode} from './document';
export interface ParagraphCapture {blockIndex:number;documentHash:string;block:DocumentNode}
export interface ParagraphVersion {id:string;label:string;block:DocumentNode}
export interface ParagraphAlternativeGroup {
 id:string;label:string;anchor:ParagraphCapture;sceneRevisionId:string;originalId:string;activeId:string;versions:ParagraphVersion[];
}
export interface ParagraphAlternativesDocument {schemaVersion:1;scenes:{sceneId:string;groups:ParagraphAlternativeGroup[]}[]}
export const emptyParagraphAlternatives=():ParagraphAlternativesDocument=>({schemaVersion:1,scenes:[]});
export function validateParagraphBlock(block:DocumentNode):void {
 if(!block||!['paragraph','heading'].includes(block.type??''))throw new Error('Choose one complete paragraph or heading.');
 validateScene({schemaVersion:1,title:'',document:{type:'doc',content:[block]}});
}
export function validateParagraphAlternatives(document:ParagraphAlternativesDocument):void {
 if(!document||document.schemaVersion!==1||!Array.isArray(document.scenes)||document.scenes.length>10000||new TextEncoder().encode(JSON.stringify(document)).length>8_000_000)throw new Error('Unsupported saved paragraph alternatives.');
 const scenes=new Set<string>(),groups=new Set<string>(),versions=new Set<string>();
 for(const scene of document.scenes){
  if(typeof scene.sceneId!=='string'||!scene.sceneId||scene.sceneId.length>300||scenes.has(scene.sceneId)||!Array.isArray(scene.groups)||scene.groups.length>100)throw new Error('Invalid paragraph alternative scene.');scenes.add(scene.sceneId);
  for(const group of scene.groups){
   if(!group||typeof group.id!=='string'||!group.id||group.id.length>300||groups.has(group.id)||typeof group.label!=='string'||group.label.length>200||typeof group.sceneRevisionId!=='string'||!group.sceneRevisionId||group.sceneRevisionId.length>300||!group.anchor||!Number.isSafeInteger(group.anchor.blockIndex)||group.anchor.blockIndex<0||group.anchor.blockIndex>100000||!/^\w{64}$/.test(group.anchor.documentHash)||!/^[0-9a-f]{64}$/.test(group.anchor.documentHash)||!Array.isArray(group.versions)||!group.versions.length||group.versions.length>20)throw new Error('Invalid saved paragraph anchor.');groups.add(group.id);validateParagraphBlock(group.anchor.block);
   for(const version of group.versions){if(!version||typeof version.id!=='string'||!version.id||version.id.length>300||versions.has(version.id)||typeof version.label!=='string'||version.label.length>200)throw new Error('Invalid paragraph version.');versions.add(version.id);validateParagraphBlock(version.block);}
   if(!group.versions.some(version=>version.id===group.originalId)||!group.versions.some(version=>version.id===group.activeId))throw new Error('Missing original or selected paragraph version.');
  }
 }
}
/** Editing text creates a new version. The original rich block is never overwritten. */
export function alternativeWithText(block:DocumentNode,text:string):DocumentNode {
 validateParagraphBlock(block);
 const current=plainText(block);if(current===text)return structuredClone(block);
 const original=(block.content??[]).filter(node=>node.type==='text');
 // Preserve exact runs outside the edited range; changed text inherits local marks.
 let prefix=0,suffix=0;while(prefix<current.length&&prefix<text.length&&current[prefix]===text[prefix])prefix++;
 while(suffix<current.length-prefix&&suffix<text.length-prefix&&current[current.length-1-suffix]===text[text.length-1-suffix])suffix++;
 // Boundaries must not split a surrogate pair.
 if(prefix>0&&/[\uD800-\uDBFF]/.test(current[prefix-1]))prefix--;
 if(suffix>0&&/[\uDC00-\uDFFF]/.test(current[current.length-suffix]))suffix--;
 const slice=(start:number,end:number):DocumentNode[]=>{let position=0;return (block.content??[]).flatMap(node=>{const value=plainText(node),left=Math.max(start,position),right=Math.min(end,position+value.length);position+=value.length;if(right<=left)return [];return node.type==='hardBreak'?[structuredClone(node)]:[{...structuredClone(node),text:value.slice(left-(position-value.length),right-(position-value.length))}];});};
 let position=0;const local=(block.content??[]).find(node=>{const length=plainText(node).length;const matches=prefix>=position&&prefix<=position+length;position+=length;return matches;})??original[0];
 const inserted=text.slice(prefix,text.length-suffix).split('\n').flatMap((part,index)=>[...(index?[{type:'hardBreak'}]:[]),...(part?[{type:'text',text:part,...(local?.marks?{marks:structuredClone(local.marks)}:{})}]:[])]);
 return {...structuredClone(block),content:[...slice(0,prefix),...inserted,...slice(current.length-suffix,current.length)]};
}
