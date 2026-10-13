import { SHA256, sha256 } from './model';
export type PackOperation = 'dictionary' | 'synonyms' | 'glossary';
export interface LexicalSense {
  id: string; definition: string; partOfSpeech: string; synonyms: string[]; related: string[]; translations: { language: string; text: string }[];
}
export interface LexicalEntry { term: string; senses: LexicalSense[]; notes?: string }
export interface LanguagePack {
  schemaVersion: 1; id: string; name: string; language: string; variant: string; version: string;
  source: string; license: string; attribution: string; operations: PackOperation[];
  checksum: string; entries: LexicalEntry[];
}
export interface DictionaryException { term: string; language: string; reason: string }
/** Stable UTF-8 representation for checksums; the checksum itself is excluded. */
export function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(canonicalJson).join(',') + ']';
  if (value && typeof value === 'object') return '{' + Object.entries(value).sort(([a],[b]) => a < b ? -1 : a > b ? 1 : 0).map(([key, entry]) => JSON.stringify(key) + ':' + canonicalJson(entry)).join(',') + '}';
  return JSON.stringify(value);
}
export async function packChecksum(pack: Omit<LanguagePack, 'checksum'> | LanguagePack): Promise<string> {
  const { checksum: _, ...payload } = pack as LanguagePack; return sha256(canonicalJson(payload));
}
function required(value: unknown, name: string, max = 200): asserts value is string {
  if (typeof value !== 'string' || !value.trim() || value.length > max) throw new Error('Invalid pack ' + name + '.');
}
export async function importLanguagePack(json: string): Promise<LanguagePack> {
  if (new TextEncoder().encode(json).length > 2_000_000) throw new Error('Pack exceeds the 2 MB import limit.');
  const pack: LanguagePack = JSON.parse(json);
  if (!pack || pack.schemaVersion !== 1) throw new Error('Unsupported language-pack version.');
  for (const key of ['id','name','language','variant','version','source','license','attribution'] as const) required(pack[key], key, key === 'attribution' ? 5000 : 500);
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,99}$/.test(pack.id)) throw new Error('Invalid pack identity.');
  if (!Array.isArray(pack.operations) || !pack.operations.length || new Set(pack.operations).size !== pack.operations.length || pack.operations.some(op => !['dictionary','synonyms','glossary'].includes(op))) throw new Error('Unsupported pack operation.');
  if (!Array.isArray(pack.entries) || pack.entries.length > 10000) throw new Error('Invalid pack entries.');
  const terms = new Set<string>();
  for (const entry of pack.entries) {
    required(entry.term, 'term', 200);
    if (terms.has(entry.term)) throw new Error('Duplicate pack term.'); terms.add(entry.term);
    if (entry.notes !== undefined && (typeof entry.notes !== 'string' || entry.notes.length > 2000)) throw new Error('Invalid pack notes.');
    if (!Array.isArray(entry.senses) || entry.senses.length < 1 || entry.senses.length > 100) throw new Error('A term needs bounded senses.');
    const senses = new Set<string>();
    for (const sense of entry.senses) {
      required(sense.id, 'sense identity'); required(sense.definition, 'definition', 3000); required(sense.partOfSpeech, 'part of speech');
      if (senses.has(sense.id)) throw new Error('Duplicate sense identity.'); senses.add(sense.id);
      for (const values of [sense.synonyms, sense.related]) {
        if (!Array.isArray(values) || values.length > 100) throw new Error('Invalid lexical alternatives.'); values.forEach(value => required(value, 'alternative', 200));
      }
      if (!Array.isArray(sense.translations) || sense.translations.length > 100) throw new Error('Invalid translations.');
      sense.translations.forEach(value => { required(value.language, 'translation language'); required(value.text, 'translation', 200); });
      if (!pack.operations.includes('synonyms') && sense.synonyms.length) throw new Error('Synonyms are not declared by this pack.');
    }
  }
  if (typeof pack.checksum !== 'string' || !SHA256.test(pack.checksum) || await packChecksum(pack) !== pack.checksum) throw new Error('Pack checksum mismatch. Nothing was imported.');
  return structuredClone(pack);
}
export function installPack(installed: readonly LanguagePack[], incoming: LanguagePack): LanguagePack[] {
  const existing = installed.find(pack => pack.id === incoming.id);
  if (existing && existing.version === incoming.version && existing.checksum !== incoming.checksum) throw new Error('This pack version has different content. Give the changed pack a new version.');
  return [...installed.filter(pack => pack.id !== incoming.id), incoming];
}
export function lookup(packs: readonly LanguagePack[], term: string, language: string, caseInsensitive = false): { pack: LanguagePack; entry: LexicalEntry }[] {
  const match = (value: string) => caseInsensitive ? value.toLocaleLowerCase(language) === term.toLocaleLowerCase(language) : value === term;
  return packs.filter(pack => pack.language === language).flatMap(pack => pack.entries.filter(entry => match(entry.term)).map(entry => ({ pack, entry })));
}
export function isDictionaryException(exceptions: readonly DictionaryException[], term: string, language: string): boolean {
  return exceptions.some(exception => exception.term === term && exception.language === language);
}
export function addDictionaryException(exceptions: readonly DictionaryException[], incoming: DictionaryException): DictionaryException[] {
  required(incoming.term, 'exception term'); required(incoming.language, 'exception language'); required(incoming.reason, 'exception reason', 1000);
  return [...exceptions.filter(value => value.term !== incoming.term || value.language !== incoming.language), incoming];
}
/** Only a matching quoted issue is suppressed; other issues in its sentence remain visible. */
export function filterProofreadingExceptions<T extends { origin: string; evidence: { quote: string }[] }>(suggestions: readonly T[], exceptions: readonly DictionaryException[], language: string): T[] {
  return suggestions.filter(suggestion => suggestion.origin !== 'proofreading' || !suggestion.evidence.some(anchor => isDictionaryException(exceptions,anchor.quote,language)));
}
