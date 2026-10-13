import type {DocumentNode} from './document';
export interface ChangedParagraph {index:number;before:string;after:string;formattingOnly:boolean}
function text(block:DocumentNode|undefined):string {if(!block)return '';return (block.content??[]).map(node=>node.type==='hardBreak'?'\n':node.text??'').join('');}
/** Read-only comparison by paragraph position; insertions may shift later rows. */
export function changedParagraphs(before:DocumentNode,after:DocumentNode):ChangedParagraph[]{
 const old=before.content??[],next=after.content??[],result:ChangedParagraph[]=[];
 for(let index=0;index<Math.max(old.length,next.length);index++)if(JSON.stringify(old[index])!==JSON.stringify(next[index])){const previous=text(old[index]),current=text(next[index]);result.push({index,before:previous,after:current,formattingOnly:previous===current});}
 return result;
}
export function revisionExcerpt(value:string,limit=2000):{text:string;truncated:boolean}{const points=Array.from(value);return {text:points.slice(0,limit).join(''),truncated:points.length>limit};}
