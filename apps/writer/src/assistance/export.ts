import { invoke } from '@tauri-apps/api/core';
export interface ExportReceipt { cancelled:boolean; name?:string }
/** Desktop exports use the system chooser; browser previews retain their local download path. */
export async function exportLocalJson(name:string,value:unknown,format:'dictionary'|'alphabet'|'project'='dictionary'):Promise<ExportReceipt> {
  const content=typeof value==='string'?value:JSON.stringify(value,null,2);
  if (typeof window!=='undefined' && '__TAURI_INTERNALS__' in window) return invoke<ExportReceipt>('export_document',{ format,content,suggestedName:name });
  const url=URL.createObjectURL(new Blob([content],{ type:'application/json;charset=utf-8' }));const anchor=document.createElement('a');anchor.href=url;anchor.download=name;anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000);return { cancelled:false,name };
}
