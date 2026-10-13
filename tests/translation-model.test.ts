import {describe, expect, it} from 'vitest';
import {emptyTranslations, latestTranslations, translationNeedsUpdate, validateTranslations} from '../apps/writer/src/translations/model';

describe('translation model', () => {
  it('keeps exact multilingual source and translation text and picks latest per target in insertion order', () => {
    const a={id:'a',sceneId:'s',sceneRevisionId:'r',sourceLanguage:'ar' as const,targetLanguage:'zh-Hans' as const,sourceText:'مَرْحَبًا 世界 𠀀 😀',text:'你好，世界！'};
    const b={...a,id:'b',sceneRevisionId:'r2',sourceText:'updated source',text:''};
    const en={...a,id:'c',targetLanguage:'en' as const,text:'Hello'};
    const doc={...emptyTranslations(),drafts:[a,b,en]};
    validateTranslations(doc);
    expect(latestTranslations(doc,'s')).toEqual([b,en]);
    expect(translationNeedsUpdate(a,'مرحبا')).toBe(true);
    expect(translationNeedsUpdate(a,a.sourceText)).toBe(false);
  });
  it('rejects duplicate identities and same source and target languages', () => {
    const d={id:'x',sceneId:'s',sceneRevisionId:'r',sourceLanguage:'en',targetLanguage:'ar',sourceText:'source',text:''};
    expect(() => validateTranslations({schemaVersion:1,drafts:[d,d]})).toThrow();
    expect(() => validateTranslations({schemaVersion:1,drafts:[{...d,targetLanguage:'en'}]})).toThrow();
    expect(() => validateTranslations({schemaVersion:1,drafts:[],extra:true})).toThrow();
    expect(() => validateTranslations({schemaVersion:1,drafts:[{...d,id:'has space'}]})).toThrow();
  });
});
