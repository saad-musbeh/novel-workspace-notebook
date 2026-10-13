export type MainLanguage = 'en' | 'ar' | 'zh-Hans' | 'zh-Hant';
export const translationLanguages: readonly {code: MainLanguage; label: string}[] = [
  {code: 'en', label: 'English'}, {code: 'ar', label: 'العربية'},
  {code: 'zh-Hans', label: '简体中文'}, {code: 'zh-Hant', label: '繁體中文'},
];
export interface TranslationDraft {
  id: string; sceneId: string; sceneRevisionId: string;
  sourceLanguage: MainLanguage; targetLanguage: MainLanguage;
  sourceText: string; text: string;
}
export interface TranslationsDocument {schemaVersion: 1; drafts: TranslationDraft[]}
const languages = new Set<string>(translationLanguages.map(x => x.code));
export function emptyTranslations(): TranslationsDocument { return {schemaVersion: 1, drafts: []}; }
export function validateTranslations(value: unknown): asserts value is TranslationsDocument {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid translations document.');
  const doc = value as Record<string, unknown>;
  if (Object.keys(doc).sort().join(',') !== 'drafts,schemaVersion' || doc.schemaVersion !== 1 || !Array.isArray(doc.drafts) || doc.drafts.length > 1000) throw new Error('Invalid translations schema or draft count.');
  const ids = new Set<string>();
  const validId = (id: unknown): id is string => typeof id === 'string' && /^[A-Za-z0-9:_-]{1,128}$/.test(id);
  for (const item of doc.drafts) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) throw new Error('Invalid translation draft.');
    const d = item as Record<string, unknown>;
    if (Object.keys(d).sort().join(',') !== 'id,sceneId,sceneRevisionId,sourceLanguage,sourceText,targetLanguage,text' ||
        !validId(d.id) || ids.has(d.id) ||
        !validId(d.sceneId) ||
        !validId(d.sceneRevisionId) ||
        !languages.has(d.sourceLanguage as string) || !languages.has(d.targetLanguage as string) || d.sourceLanguage === d.targetLanguage ||
        typeof d.sourceText !== 'string' || !d.sourceText || d.sourceText.length > 500000 ||
        typeof d.text !== 'string' || d.text.length > 500000) throw new Error('Invalid translation draft fields.');
    ids.add(d.id);
  }
  if (new TextEncoder().encode(JSON.stringify(value)).length > 8_000_000) throw new Error('Translations exceed 8 MB.');
}
export function latestTranslations(document: TranslationsDocument, sceneId: string): TranslationDraft[] {
  const latest = new Map<string, TranslationDraft>();
  for (const draft of document.drafts) if (draft.sceneId === sceneId) latest.set(draft.targetLanguage, draft);
  return document.drafts.filter(draft => draft.sceneId === sceneId && latest.get(draft.targetLanguage) === draft);
}
export function translationNeedsUpdate(draft: TranslationDraft, currentText: string): boolean { return draft.sourceText !== currentText; }
