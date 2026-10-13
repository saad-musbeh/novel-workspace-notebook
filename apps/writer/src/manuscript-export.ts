import { plainText, validateScene, type DocumentNode, type Scene } from './document';
export type ExportFormat='text'|'markdown'|'html';
export const escapeHtml=(text:string)=>text.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;');
function html(node:DocumentNode):string {
  if(node.type==='text'){let text=escapeHtml(node.text??'');for(const mark of node.marks??[])text=mark.type==='bold'?'<strong>'+text+'</strong>':mark.type==='italic'?'<em>'+text+'</em>':'<u>'+text+'</u>';return text;}
  if(node.type==='hardBreak')return '<br>';
  const content=(node.content??[]).map(html).join('');if(node.type==='doc')return content;
  const spacing=node.attrs?.lineSpacing===undefined?'':` style="line-height:${node.attrs.lineSpacing}"`;
  const tag=node.type==='heading'?'h2':'p';return `<${tag} dir="${escapeHtml(String(node.attrs?.dir??'auto'))}" lang="${escapeHtml(String(node.attrs?.lang??'en'))}"${spacing}>${content}</${tag}>`;
}
function markdown(node:DocumentNode):string {
  if(node.type==='text'){let text=(node.text??'').replace(/[\\`*_\[\]<>]/g,'\\$&');for(const mark of node.marks??[])text=mark.type==='bold'?'**'+text+'**':mark.type==='italic'?'*'+text+'*':text;return text;}
  if(node.type==='hardBreak')return '  \n';
  const content=(node.content??[]).map(markdown).join(node.type==='doc'?'\n\n':'');return (node.type==='heading'?'## ':'')+content;
}
export function exportManuscript(scenes:Scene[],format:ExportFormat,title:string):{content:string;extension:string;losses:string} {
  scenes.forEach(validateScene);
  if(format==='text')return {content:title+'\n\n'+scenes.map(scene=>scene.title+'\n\n'+plainText(scene.document)).join('\n\n\n'),extension:'txt',losses:'Text export omits formatting, paragraph metadata, planning and font assets.'};
  if(format==='markdown')return {content:'# '+title.replace(/[\r\n]/g,' ')+'\n\n'+scenes.map(scene=>'# '+scene.title.replace(/[\r\n]/g,' ')+'\n\n'+markdown(scene.document)).join('\n\n'),extension:'md',losses:'Markdown omits underline, paragraph language/direction, line spacing, planning and font assets.'};
  return {content:`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; font-src 'none'; img-src 'none'"><title>${escapeHtml(title)}</title><style>body{max-width:850px;margin:40px auto;padding:0 24px;color:#283c37;background:#fffef9;font:18px/1.9 serif}h1,h2{line-height:1.4}p{white-space:pre-wrap}[lang=ar]{font-family:Vazirmatn,serif}</style></head><body><h1 dir="auto">${escapeHtml(title)}</h1>${scenes.map(scene=>'<section><h2 dir="auto">'+escapeHtml(scene.title)+'</h2>'+html(scene.document)+'</section>').join('')}</body></html>`,extension:'html',losses:'HTML retains formatting and paragraph metadata. Custom font assets and planning remain in the complete project archive.'};
}
