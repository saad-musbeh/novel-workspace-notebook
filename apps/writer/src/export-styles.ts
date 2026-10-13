import type {ExportProfile,ExportStyle} from './generated/workspace-contract';
export type {ExportProfile,ExportStyle};

export type ExportPreset = 'current' | 'standard' | 'compact';
export type ExportStylePart = 'title' | 'chapter' | 'scene' | 'body';

const style = (font: string, size: number, bold = false, italic = false): ExportStyle => ({font,size,bold,italic});
export const DEFAULT_EXPORT_PROFILE: ExportProfile = {
  schemaVersion:1,pageSize:'letter',marginMm:25.4,lineSpacing:'source',
  title:style('Aptos Display',28),chapter:style('Aptos Display',20),scene:style('Aptos Display',16),body:style('Aptos',12),
};

export const EXPORT_PRESETS: Record<ExportPreset,ExportProfile> = {
  current:DEFAULT_EXPORT_PROFILE,
  standard:{schemaVersion:1,pageSize:'a4',marginMm:25.4,lineSpacing:'2',title:style('Times New Roman',20),chapter:style('Times New Roman',14,true),scene:style('Times New Roman',12,true),body:style('Times New Roman',12)},
  compact:{schemaVersion:1,pageSize:'a4',marginMm:20,lineSpacing:'1.15',title:style('Aptos Display',24),chapter:style('Aptos Display',18),scene:style('Aptos Display',14),body:style('Aptos',12)},
};

const profileKeys=['schemaVersion','pageSize','marginMm','lineSpacing','title','chapter','scene','body'];
const styleKeys=['font','size','bold','italic'];
const ownKeys=(value:Record<string,unknown>,expected:string[])=>Object.keys(value).length===expected.length&&expected.every(key=>Object.hasOwn(value,key));
function xmlSafeFont(font:string):boolean {
  const points=[...font];
  return points.length>=1&&points.length<=80&&font.trim()===font&&
    points.every(character=>{const code=character.codePointAt(0)!;return code>=0x20&&!(code>=0x7f&&code<=0x9f)&&!(code>=0xd800&&code<=0xdfff)&&code<=0x10ffff&&code%0x10000!==0xfffe&&code%0x10000!==0xffff;});
}
export function validateExportStyle(value:unknown,label='style'): asserts value is ExportStyle {
  if(!value||typeof value!=='object'||Array.isArray(value))throw new Error(`The ${label} must be an object.`);
  const item=value as Record<string,unknown>;
  if(!ownKeys(item,styleKeys))throw new Error(`The ${label} has unsupported fields.`);
  if(typeof item.font!=='string'||!xmlSafeFont(item.font))throw new Error(`The ${label} font must be 1–80 trimmed XML-safe characters.`);
  if(typeof item.size!=='number'||!Number.isFinite(item.size)||item.size<8||item.size>72||Math.round(item.size*2)!==item.size*2)throw new Error(`The ${label} size must be 8–72 points in half-point steps.`);
  if(typeof item.bold!=='boolean'||typeof item.italic!=='boolean')throw new Error(`The ${label} bold and italic values must be selected or cleared.`);
}
export function validateExportProfile(value:unknown): asserts value is ExportProfile {
  if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('The Word export profile must be an object.');
  const profile=value as Record<string,unknown>;
  if(!ownKeys(profile,profileKeys)||profile.schemaVersion!==1)throw new Error('The Word export profile has unsupported fields or version.');
  if(profile.pageSize!=='letter'&&profile.pageSize!=='a4')throw new Error('Choose Letter or A4 paper.');
  if(typeof profile.marginMm!=='number'||!Number.isFinite(profile.marginMm)||profile.marginMm<10||profile.marginMm>40)throw new Error('Page margins must be between 10 and 40 mm.');
  if(typeof profile.lineSpacing!=='string'||!['source','1','1.15','1.5','2'].includes(profile.lineSpacing))throw new Error('Choose a supported body line spacing.');
  for(const part of ['title','chapter','scene','body'] as const)validateExportStyle(profile[part],part);
}

export interface ExportProfileSnapshot {revisionId:string;payload:ExportProfile}
export type ExportProfileRpc=<T>(operation:string,args:Record<string,unknown>)=>Promise<T>;

export async function loadExportProfile(projectId:string,rpc:ExportProfileRpc):Promise<{snapshot:ExportProfileSnapshot;conflicted:boolean}> {
  const [saved,heads]=await Promise.all([
    rpc<ExportProfileSnapshot|null>('load_project_data',{projectId,kind:'exportProfile'}),
    rpc<{revisionIds:string[]}>('project_data_heads',{projectId,entityId:projectId+':exportProfile',entityType:'projectData',kind:'exportProfile'}),
  ]);
  const payload=saved?.payload??DEFAULT_EXPORT_PROFILE;
  validateExportProfile(payload);
  const revisionId=saved?.revisionId??'';
  const headMismatch=saved? !heads.revisionIds.includes(revisionId):heads.revisionIds.length>0;
  return {snapshot:{revisionId,payload},conflicted:heads.revisionIds.length>1||headMismatch};
}

export async function saveExportProfile(projectId:string,payload:ExportProfile,parent:string|null,rpc:ExportProfileRpc):Promise<ExportProfileSnapshot> {
  validateExportProfile(payload);
  const saved=await rpc<ExportProfileSnapshot>('save_project_data',{projectId,kind:'exportProfile',payload,parent});
  validateExportProfile(saved.payload);
  return saved;
}
