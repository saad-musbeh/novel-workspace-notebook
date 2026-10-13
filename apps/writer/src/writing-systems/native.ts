import { workspaceCommand, type DataSnapshot } from '../workspace';
import { createWritingSystemsDocument, validateWritingSystemsDocument, validateWritingSystemsTransition, type ProjectFontAsset, type WritingSystemsAdapter, type WritingSystemsDocument } from './model';
export const nativeWritingSystemsAdapter:WritingSystemsAdapter={
  async load(projectId){const snapshot=await workspaceCommand<DataSnapshot<WritingSystemsDocument>|null>('load_project_data',{projectId,kind:'writingSystems'});const document=snapshot?.payload??createWritingSystemsDocument();validateWritingSystemsDocument(document);return {document,revisionId:snapshot?.revisionId??null};},
  async save(projectId,document,expectedRevisionId){validateWritingSystemsDocument(document);const previous=await nativeWritingSystemsAdapter.load(projectId);if(previous.revisionId!==expectedRevisionId)throw new Error('Conflict: alphabet definitions changed in another window. Original versions are preserved.');validateWritingSystemsTransition(previous.document,document);const snapshot=await workspaceCommand<DataSnapshot<WritingSystemsDocument>>('save_project_data',{projectId,kind:'writingSystems',payload:document,parent:expectedRevisionId});return {document:snapshot.payload,revisionId:snapshot.revisionId};},
  putAsset(projectId,asset){return workspaceCommand<void>('put_asset',{projectId,asset});},
  listAssets(projectId){return workspaceCommand<ProjectFontAsset[]>('list_assets',{projectId});},
};
