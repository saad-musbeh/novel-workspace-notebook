/** All offsets are UTF-16 code units into the exact saved plain-text projection. */
export interface SourceRef { sceneId: string; revisionId: string; hash: string }
export interface SceneSnapshot extends SourceRef { kind?: 'scene' | 'facts'; text: string; title?: string; language?: string }
export interface EvidenceAnchor extends SourceRef {
  offsetUnit: 'utf16'; start: number; end: number; quote: string; prefix: string; suffix: string;
}
export type SuggestionOrigin = 'manual' | 'rule' | 'proofreading' | 'llm' | 'lexical';
export type SuggestionAction = { kind: 'replace'; replacement: string } | { kind: 'question' } | { kind: 'fact'; subject: string; property: string; value: string };
export interface Suggestion {
  id: string; origin: SuggestionOrigin; title: string; explanation: string; sources: SourceRef[];
  evidence: EvidenceAnchor[]; action: SuggestionAction; status: 'pending' | 'accepted' | 'rejected'; ruleId?: string;
}
export interface ScenePatch { suggestionId: string; source: SourceRef; anchor: EvidenceAnchor; replacement: string; supportingSources?: SourceRef[] }
export interface FactProposal { suggestionId: string; subject: string; property: string; value: string; sources: SourceRef[]; evidence: EvidenceAnchor[] }
export interface PatchReceipt { revisionId: string; hash: string }
export const SHA256 = /^[a-f0-9]{64}$/;
export function sourceKey(ref: SourceRef): string { return ref.sceneId + '\u0000' + ref.revisionId; }
export function sameSource(a: SourceRef, b: SourceRef): boolean { return a.sceneId === b.sceneId && a.revisionId === b.revisionId && a.hash === b.hash; }
export async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('');
}
function assert(condition: unknown, message: string): asserts condition { if (!condition) throw new Error(message); }
export function validateSource(ref: SourceRef): void {
  assert(ref && typeof ref.sceneId === 'string' && ref.sceneId.length > 0 && ref.sceneId.length < 200, 'Missing scene identity.');
  assert(typeof ref.revisionId === 'string' && ref.revisionId.length > 0 && ref.revisionId.length < 200, 'Review needs a committed revision.');
  assert(typeof ref.hash === 'string' && SHA256.test(ref.hash), 'Review needs a SHA-256 source hash.');
}
function boundary(text: string, offset: number): boolean {
  if (offset === text.length) return true;
  return [...new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)].some(segment => segment.index === offset);
}
export function validateAnchor(anchor: EvidenceAnchor, source: SceneSnapshot): void {
  validateSource(anchor);
  assert(sameSource(anchor, source), 'Evidence belongs to another scene revision.');
  assert(anchor.offsetUnit === 'utf16', 'Unsupported evidence offset unit.');
  assert(Number.isSafeInteger(anchor.start) && Number.isSafeInteger(anchor.end) && anchor.start >= 0 && anchor.end > anchor.start && anchor.end <= source.text.length, 'Evidence offsets are invalid.');
  assert(typeof anchor.quote === 'string' && source.text.slice(anchor.start, anchor.end) === anchor.quote, 'Quoted evidence does not match the saved passage.');
  assert(boundary(source.text, anchor.start) && boundary(source.text, anchor.end), 'Evidence splits a character or combining sequence.');
  assert(typeof anchor.prefix === 'string' && typeof anchor.suffix === 'string' && anchor.prefix.length <= 80 && anchor.suffix.length <= 80, 'Evidence context is invalid.');
  assert(source.text.slice(Math.max(0, anchor.start - anchor.prefix.length), anchor.start) === anchor.prefix && source.text.slice(anchor.end, anchor.end + anchor.suffix.length) === anchor.suffix, 'Evidence context does not match.');
}
export function anchorFor(source: SceneSnapshot, start: number, end: number): EvidenceAnchor {
  const anchor: EvidenceAnchor = { sceneId: source.sceneId, revisionId: source.revisionId, hash: source.hash, offsetUnit: 'utf16', start, end, quote: source.text.slice(start, end), prefix: source.text.slice(Math.max(0, start - 35), start), suffix: source.text.slice(end, end + 35) };
  validateAnchor(anchor, source); return anchor;
}
export function validateSuggestion(suggestion: Suggestion, scope: readonly SceneSnapshot[]): void {
  assert(suggestion && typeof suggestion.id === 'string' && suggestion.id.length > 0 && suggestion.id.length <= 200, 'Invalid suggestion identity.');
  assert(['manual','rule','proofreading','llm','lexical'].includes(suggestion.origin), 'Unknown review provider.');
  assert(typeof suggestion.title === 'string' && suggestion.title.length > 0 && suggestion.title.length <= 300 && typeof suggestion.explanation === 'string' && suggestion.explanation.length <= 5000, 'Invalid review explanation.');
  assert(['pending','accepted','rejected'].includes(suggestion.status), 'Invalid suggestion status.');
  assert(Array.isArray(suggestion.sources) && suggestion.sources.length > 0 && suggestion.sources.length <= 50, 'A review must identify its source scope.');
  for (const ref of suggestion.sources) { validateSource(ref); assert(scope.some(source => sameSource(ref, source)), 'Review references a source outside its immutable scope.'); }
  assert(new Set(suggestion.sources.map(sourceKey)).size === suggestion.sources.length, 'Duplicate review source.');
  assert(Array.isArray(suggestion.evidence) && suggestion.evidence.length <= 100, 'Invalid review evidence.');
  for (const anchor of suggestion.evidence) {
    assert(suggestion.sources.some(source => sameSource(source, anchor)), 'Evidence is outside the declared review sources.');
    const source = scope.find(source => sameSource(source, anchor)); assert(source, 'Evidence source is unavailable.'); validateAnchor(anchor, source);
  }
  assert(suggestion.action && ['replace','question','fact'].includes(suggestion.action.kind), 'Unsupported suggestion action.');
  if (suggestion.action.kind === 'replace') {
    assert(suggestion.evidence.length === 1, 'A replacement must have one exact evidence anchor.');
    assert(scope.find(source => sameSource(source,suggestion.evidence[0]))?.kind !== 'facts', 'Canonical fact context cannot be replaced as manuscript text.');
    assert(typeof suggestion.action.replacement === 'string' && suggestion.action.replacement.length <= 10000, 'Replacement is too large.');
  }
  if (suggestion.action.kind === 'fact') {
    assert(suggestion.evidence.length > 0, 'A proposed fact must have quoted source evidence.');
    assert(suggestion.evidence.some(anchor => scope.find(source => sameSource(source,anchor))?.kind !== 'facts'), 'New fact proposals require manuscript evidence.');
    for (const value of [suggestion.action.subject, suggestion.action.property, suggestion.action.value]) assert(typeof value === 'string' && value.length > 0 && value.length <= 1000, 'Invalid proposed fact.');
  }
}
export function freshness(suggestion: Suggestion, current: readonly SceneSnapshot[]): { fresh: boolean; reason?: string } {
  if (suggestion.sources.some(ref => !current.some(source => sameSource(ref, source)))) return { fresh: false, reason: 'A reviewed scene or supporting source has changed or is unavailable. Run a new review.' };
  try { validateSuggestion(suggestion, current); return { fresh: true }; } catch (error) { return { fresh: false, reason: String(error instanceof Error ? error.message : error) }; }
}
export function suggestionPatch(suggestion: Suggestion, current: readonly SceneSnapshot[]): ScenePatch {
  const state = freshness(suggestion, current); assert(state.fresh, state.reason ?? 'Stale review.');
  assert(suggestion.status === 'pending' && suggestion.action.kind === 'replace', 'This suggestion cannot replace text.');
  return { suggestionId: suggestion.id, source: suggestion.evidence[0], anchor: suggestion.evidence[0], replacement: suggestion.action.replacement,supportingSources:suggestion.sources.map(source => ({ sceneId:source.sceneId,revisionId:source.revisionId,hash:source.hash })) };
}
/** Preview only. The native repository must perform the same precondition check atomically. */
export function applyPatchToSnapshot(patch: ScenePatch, scene: SceneSnapshot): string {
  validateAnchor(patch.anchor, scene); assert(sameSource(patch.source, scene), 'The scene changed before acceptance.');
  assert(typeof patch.replacement === 'string' && patch.replacement.length <= 10000, 'Invalid replacement.');
  return scene.text.slice(0, patch.anchor.start) + patch.replacement + scene.text.slice(patch.anchor.end);
}
export function parseModelSuggestions(json: string, scope: readonly SceneSnapshot[]): Suggestion[] {
  assert(json.length <= 100000, 'Model output exceeds the review limit.');
  const parsed: unknown = JSON.parse(json);
  assert(Array.isArray(parsed) && parsed.length <= 100, 'Model output must be a bounded suggestion array.');
  const suggestions = parsed as Suggestion[];
  for (const suggestion of suggestions) { assert(suggestion.origin === 'llm' && suggestion.status === 'pending', 'Model output must remain a pending LLM proposal.'); validateSuggestion(suggestion, scope); }
  assert(new Set(suggestions.map(s => s.id)).size === suggestions.length, 'Duplicate model suggestion identity.');
  return suggestions;
}
