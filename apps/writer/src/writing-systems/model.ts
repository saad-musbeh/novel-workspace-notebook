import type { Direction } from '../document';
export interface CharacterMapping { input:string; output:string; transliteration:string }
export interface FontLicense { name:string; source:string; permissionConfirmed:boolean }
export interface ProjectFont { fontAssetId:string; checksum:string; family:string; format:'ttf'|'otf'; license:FontLicense }
export interface WritingSystemDefinition {
  schemaVersion:1; id:string; mappingVersion:number; name:string; scriptId:string;
  encoding:'unicode'|'private-use'|'font-presentation'; direction:Direction;
  mappings:CharacterMapping[]; font:ProjectFont|null; glossary:{symbol:string;meaning:string}[]; notes:string;
}
export interface WritingSystemAssignment { scene:{kind:'scene';id:string}; definitionId:string; mappingVersion:number }
export interface WritingSystemsDocument { schemaVersion:1; systems:WritingSystemDefinition[]; assignments:WritingSystemAssignment[] }
export interface WritingSystemsSnapshot { document:WritingSystemsDocument; revisionId:string|null }
export interface ProjectFontAsset { id:string; path:string; mimeType:string; content:number[]; checksum:string }
export interface WritingSystemsAdapter {
  load(projectId:string):Promise<WritingSystemsSnapshot>;
  save(projectId:string,document:WritingSystemsDocument,expectedRevisionId:string|null):Promise<WritingSystemsSnapshot>;
  putAsset(projectId:string,asset:ProjectFontAsset):Promise<void>;
  listAssets(projectId:string):Promise<ProjectFontAsset[]>;
}
export const createWritingSystemsDocument=():WritingSystemsDocument=>({schemaVersion:1,systems:[],assignments:[]});
export const definitionKey=(definition:WritingSystemDefinition)=>`${definition.id}@${definition.mappingVersion}`;
export function newWritingSystem():WritingSystemDefinition {return {schemaVersion:1,id:crypto.randomUUID(),mappingVersion:1,name:'New alphabet',scriptId:'custom',encoding:'private-use',direction:'ltr',mappings:[],font:null,glossary:[],notes:''};}
function assert(condition:unknown,message:string):asserts condition{if(!condition)throw new Error(message);}
const str=(value:unknown):value is string=>typeof value==='string';
const scalar=(value:unknown):value is string=>str(value)&&[...value].length===1&&!(/[\uD800-\uDFFF]/u.test(value));
export const privateUse=(symbol:string)=>{const cp=symbol.codePointAt(0)!;return (cp>=0xe000&&cp<=0xf8ff)||(cp>=0xf0000&&cp<=0xffffd)||(cp>=0x100000&&cp<=0x10fffd);};
export function validateWritingSystem(value:unknown):asserts value is WritingSystemDefinition {
  const d=value as WritingSystemDefinition;assert(d&&d.schemaVersion===1,'Unsupported writing-system version.');
  assert(str(d.id)&&d.id.length>0&&d.id.length<200&&Number.isSafeInteger(d.mappingVersion)&&d.mappingVersion>0,'Writing systems need a stable identity and positive mapping version.');
  assert(str(d.name)&&d.name.trim().length>0&&d.name.length<=200&&str(d.scriptId)&&/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(d.scriptId),'Give this alphabet a name and a documented script identifier.');
  assert(['unicode','private-use','font-presentation'].includes(d.encoding)&&['auto','ltr','rtl'].includes(d.direction)&&str(d.notes)&&d.notes.length<=100_000,'Invalid writing-system encoding, direction, or notes.');
  assert(Array.isArray(d.mappings)&&d.mappings.length<=4096&&Array.isArray(d.glossary)&&d.glossary.length<=4096,'Too many or invalid alphabet mappings.');
  const inputs=new Set<string>(), outputs=new Set<string>();
  for(const mapping of d.mappings){assert(mapping&&scalar(mapping.input)&&scalar(mapping.output)&&str(mapping.transliteration)&&mapping.transliteration.length<=200,'Each typing mapping uses one Unicode scalar input and one Unicode scalar output.');assert(!inputs.has(mapping.input)&&!outputs.has(mapping.output),'Mapping input keys and output symbols must be unique.');inputs.add(mapping.input);outputs.add(mapping.output);assert(!/[\p{Cc}\p{Cf}]/u.test(mapping.input)&&!/[\p{Cc}\p{Cf}]/u.test(mapping.output),'Control and invisible format characters cannot be alphabet keys.');if(d.encoding==='private-use')assert(privateUse(mapping.output),'Private-use output must use a documented Unicode PUA range.');if(d.encoding==='font-presentation')assert(mapping.input===mapping.output,'Font presentation keeps underlying Unicode characters unchanged.');}
  for(const entry of d.glossary)assert(entry&&str(entry.symbol)&&str(entry.meaning)&&entry.symbol.length<=500&&entry.meaning.length<=2000,'Invalid alphabet glossary entry.');
  if(d.font!==null){const f=d.font;assert(f&&str(f.fontAssetId)&&f.fontAssetId.length>0&&/^[a-f0-9]{64}$/.test(f.checksum)&&str(f.family)&&f.family.trim().length>0&&f.family.length<=200&&['ttf','otf'].includes(f.format),'Invalid immutable project font reference or checksum.');assert(f.license&&str(f.license.name)&&f.license.name.trim().length>0&&str(f.license.source)&&f.license.permissionConfirmed===true,'Confirm permission to store and transfer this font with the project, and record its license.');}
}
export function validateWritingSystemsDocument(value:unknown):asserts value is WritingSystemsDocument {
  const doc=value as WritingSystemsDocument;assert(doc&&doc.schemaVersion===1&&Array.isArray(doc.systems)&&Array.isArray(doc.assignments),'Unsupported writing-systems document version or structure.');assert(doc.systems.length<=4096&&doc.assignments.length<=10000,'Too many alphabet definitions or scene assignments.');const keys=new Set<string>();
  for(const definition of doc.systems){validateWritingSystem(definition);const key=definitionKey(definition);assert(!keys.has(key),'An alphabet mapping version is immutable and may occur only once.');keys.add(key);}
  const scenes=new Set<string>();for(const assignment of doc.assignments){assert(assignment&&assignment.scene?.kind==='scene'&&str(assignment.scene.id)&&assignment.scene.id.length>0&&!scenes.has(assignment.scene.id),'Scene alphabet assignments must be unique.');scenes.add(assignment.scene.id);assert(keys.has(`${assignment.definitionId}@${assignment.mappingVersion}`),'The selected alphabet mapping version is missing. Existing text has not been reinterpreted.');}
}
export function selectedDefinition(doc:WritingSystemsDocument,sceneId:string):WritingSystemDefinition|null {
  const assignment=doc.assignments.find(a=>a.scene.id===sceneId);if(!assignment)return null;const definition=doc.systems.find(s=>s.id===assignment.definitionId&&s.mappingVersion===assignment.mappingVersion);if(!definition)throw new Error('This scene selects an unavailable alphabet version. Original text is preserved.');return definition;
}
export function assignWritingSystem(doc:WritingSystemsDocument,sceneId:string,definition:WritingSystemDefinition|null):WritingSystemsDocument {
  const next={...doc,assignments:[...doc.assignments.filter(a=>a.scene.id!==sceneId),...(definition?[{scene:{kind:'scene' as const,id:sceneId},definitionId:definition.id,mappingVersion:definition.mappingVersion}]:[])]};validateWritingSystemsDocument(next);return next;
}
/** Called only for committed typing while an author-selected profile is active. Paste is exact. */
export function mapInput(text:string,definition:WritingSystemDefinition|null):string {
  if(!definition)return text;validateWritingSystem(definition);const map=new Map(definition.mappings.map(m=>[m.input,m.output]));return [...text].map(c=>map.get(c)??c).join('');
}
export function transliterate(text:string,definition:WritingSystemDefinition):string {
  validateWritingSystem(definition);const map=new Map(definition.mappings.filter(m=>m.transliteration!=='').map(m=>[m.output,m.transliteration]));return [...text].map(c=>map.get(c)??c).join('');
}
export function exportWritingSystemText(text:string,definition:WritingSystemDefinition) {return {schemaVersion:1,original:text,transliterated:transliterate(text,definition),definition:structuredClone(definition),notice:'Transliteration is a derived copy. It need not be reversible and does not replace the original text.'};}
export function publishDefinition(doc:WritingSystemsDocument,draft:WritingSystemDefinition):WritingSystemsDocument {
  validateWritingSystem(draft);const same=doc.systems.filter(s=>s.id===draft.id);const latest=same.reduce((n,s)=>Math.max(n,s.mappingVersion),0);const version={...structuredClone(draft),mappingVersion:latest+1};const next={...doc,systems:[...doc.systems,version]};validateWritingSystemsDocument(next);return next;
}
export function codepointLabel(symbol:string):string{return [...symbol].map(c=>'U+'+c.codePointAt(0)!.toString(16).toUpperCase().padStart(4,'0')).join(' ');}
export function parseCodepoint(input:string):string {const match=input.trim().match(/^(?:U\+|0x)?([a-f\d]{1,6})$/i);if(!match)throw new Error('Enter one Unicode codepoint such as U+E000.');const cp=Number.parseInt(match[1],16);if(cp>0x10ffff||(cp>=0xd800&&cp<=0xdfff))throw new Error('This codepoint is not a Unicode scalar.');return String.fromCodePoint(cp);}

/** Saved versions remain immutable; corrections publish a new version. */
export function validateWritingSystemsTransition(previous:WritingSystemsDocument,next:WritingSystemsDocument):void {
  validateWritingSystemsDocument(previous);validateWritingSystemsDocument(next);
  for(const definition of previous.systems){const retained=next.systems.find(s=>definitionKey(s)===definitionKey(definition));if(!retained||JSON.stringify(retained)!==JSON.stringify(definition))throw new Error('A saved alphabet mapping version cannot be removed or changed. Publish a new mapping version.');}
}
