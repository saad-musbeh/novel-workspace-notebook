import { sha256, validateAnchor, validateSource, validateSuggestion, type SceneSnapshot, type Suggestion } from './model';
export type ProviderKind = 'languagetool' | 'llm';
export interface ProviderStatus {
  kind: ProviderKind; state: 'unavailable' | 'loading' | 'ready' | 'busy' | 'error'; label: string;
  detail: string; languages: string[]; version?: string; model?: string; license?: string; modelHash?: string;
}
export type JobState = 'pending_local' | 'queued' | 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
export type ReviewOperation = 'grammar' | 'wording' | 'consistency' | 'extract_facts';
export interface ReviewRequest {
  schemaVersion: 1; id: string; idempotencyKey: string; projectId: string; provider: ProviderKind; operation: ReviewOperation;
  language: string; sources: SceneSnapshot[]; selection?: { sceneId: string; start: number; end: number };
  contextBudget: number; timeoutMs: number; createdAt: string;
}
export interface ReviewJob { request: ReviewRequest; state: JobState; attempt: number; updatedAt: string; error?: string; suggestions?: Suggestion[]; resultProvider?: string; resultModel?: string; resultRuntime?: string; resultModelHash?: string }
export const REVIEW_LIMITS = { maxSources: 20, maxCharacters: 50000, maxContext: 16000, maxTimeoutMs: 120000, maxJobs: 50 } as const;
export const unavailableProviders: ProviderStatus[] = [
  { kind: 'languagetool', state: 'unavailable', label: 'Local LanguageTool', detail: 'No verified local proofreading service is connected.', languages: [] },
  { kind: 'llm', state: 'unavailable', label: 'Local model', detail: 'No verified PC model is connected. A model must be supplied and evaluated separately.', languages: [] },
];
export async function validateReviewRequest(request: ReviewRequest): Promise<void> {
  if (!request || request.schemaVersion !== 1 || !request.id || !request.idempotencyKey || !request.projectId || [request.id,request.idempotencyKey,request.projectId].some(value => typeof value !== 'string' || value.length > 200)) throw new Error('Invalid review job identity.');
  if (!['languagetool','llm'].includes(request.provider) || !['grammar','wording','consistency','extract_facts'].includes(request.operation)) throw new Error('Unsupported local review operation.');
  if (request.provider === 'languagetool' && request.operation !== 'grammar') throw new Error('LanguageTool only handles grammar review.');
  if (typeof request.language !== 'string' || !request.language || request.language.length > 50) throw new Error('Choose an explicit review language.');
  if (!Array.isArray(request.sources) || request.sources.length < 1 || request.sources.length > REVIEW_LIMITS.maxSources || request.sources.reduce((size,source) => size + (typeof source.text === 'string' ? source.text.length : Infinity),0) > REVIEW_LIMITS.maxCharacters) throw new Error('Review scope exceeds the scene or text limit.');
  if (new Set(request.sources.map(source => source.sceneId)).size !== request.sources.length) throw new Error('A job cannot contain two revisions of one scene.');
  if (!Number.isSafeInteger(request.contextBudget) || request.contextBudget < 256 || request.contextBudget > REVIEW_LIMITS.maxContext) throw new Error('Invalid context budget.');
  if (!Number.isSafeInteger(request.timeoutMs) || request.timeoutMs < 1000 || request.timeoutMs > REVIEW_LIMITS.maxTimeoutMs) throw new Error('Invalid review timeout.');
  if (typeof request.createdAt !== 'string' || !Number.isFinite(Date.parse(request.createdAt))) throw new Error('Invalid job creation time.');
  for (const source of request.sources) { validateSource(source); if (source.kind !== undefined && !['scene','facts'].includes(source.kind)) throw new Error('Unsupported review source kind.'); if (source.kind === 'facts' && (request.provider !== 'llm' || source.sceneId !== 'planning:' + request.projectId || source.language !== 'und' || new TextEncoder().encode(source.text).length > 30000)) throw new Error('Invalid bounded canonical fact context.'); if (typeof source.text !== 'string' || await sha256(source.text) !== source.hash) throw new Error('Source text hash does not match its immutable snapshot.'); }
  if (request.selection) {
    const source = request.sources.find(source => source.sceneId === request.selection!.sceneId && source.kind !== 'facts');
    if (!source || !Number.isSafeInteger(request.selection.start) || !Number.isSafeInteger(request.selection.end) || request.selection.start < 0 || request.selection.end <= request.selection.start || request.selection.end > source.text.length) throw new Error('Invalid selected review passage.');
    validateAnchor({ sceneId: source.sceneId,revisionId: source.revisionId,hash: source.hash,offsetUnit: 'utf16',start: request.selection.start,end: request.selection.end,quote: source.text.slice(request.selection.start,request.selection.end),prefix: '',suffix: '' },source);
  }
}
const transitions: Record<JobState, readonly JobState[]> = {
  pending_local: ['queued','cancelled','failed'], queued: ['running','cancelling','cancelled','failed'], running: ['succeeded','failed','cancelling','interrupted'],
  cancelling: ['cancelled','failed','interrupted'], succeeded: [], failed: [], cancelled: [], interrupted: [],
};
export function transitionJob(job: ReviewJob, next: JobState, at: string, detail?: string): ReviewJob {
  if (!Number.isFinite(Date.parse(at))) throw new Error('Invalid job update time.');
  if (next === job.state) return job;
  if (!transitions[job.state].includes(next)) throw new Error('Invalid review job transition: ' + job.state + ' → ' + next);
  return { ...job, state: next, updatedAt: at, error: ['failed','interrupted'].includes(next) ? detail ?? 'The local provider did not complete this review.' : undefined };
}
export function recoverInterruptedJobs(jobs: readonly ReviewJob[], at: string): ReviewJob[] {
  return jobs.map(job => ['running','cancelling'].includes(job.state) ? transitionJob(job, 'interrupted', at, 'The PC stopped during this attempt. Retry explicitly; inference cannot resume from its previous token.') : job);
}
export function retryJob(job: ReviewJob, at: string): ReviewJob {
  if (!Number.isFinite(Date.parse(at))) throw new Error('Invalid retry time.');
  if (!['failed','interrupted'].includes(job.state)) throw new Error('Only failed or interrupted jobs may be retried.');
  if (job.attempt >= 3) throw new Error('This job has reached its three-attempt limit. Create a new request.');
  return { ...job, state: 'pending_local', attempt: job.attempt + 1, updatedAt: at, error: undefined, suggestions: undefined, resultProvider: undefined, resultModel: undefined, resultRuntime: undefined, resultModelHash: undefined };
}
export function canSubmit(provider: ProviderStatus | undefined, language: string): boolean { return Boolean(provider && provider.state === 'ready' && provider.languages.includes(language)); }
export function queueCapacity(jobs: readonly ReviewJob[]): boolean { return jobs.filter(job => ['pending_local','queued','running','cancelling'].includes(job.state)).length < 1; }

export async function validateReviewJob(job: ReviewJob): Promise<void> {
  await validateReviewRequest(job.request);
  if (!Object.prototype.hasOwnProperty.call(transitions,job.state) || !Number.isSafeInteger(job.attempt) || job.attempt < 1 || job.attempt > 3 || !Number.isFinite(Date.parse(job.updatedAt))) throw new Error('Invalid durable review job state.');
  for (const identity of [job.resultProvider,job.resultModel,job.resultRuntime,job.resultModelHash]) if (identity !== undefined && (typeof identity !== 'string' || identity.length > 300)) throw new Error('Invalid result provider identity.');
  if (job.error !== undefined && (typeof job.error !== 'string' || job.error.length > 5000)) throw new Error('Invalid review error.');
  if (job.suggestions !== undefined) {
    if (job.state !== 'succeeded' || !Array.isArray(job.suggestions) || job.suggestions.length > 100) throw new Error('Results require a completed, bounded review job.');
    for (const suggestion of job.suggestions) validateSuggestionForRequest(suggestion,job.request);
    if (new Set(job.suggestions.map(suggestion => suggestion.id)).size !== job.suggestions.length) throw new Error('Duplicate review result identity.');
  }
}

export function validateSuggestionForRequest(suggestion:Suggestion,request:ReviewRequest):void {
  validateSuggestion(suggestion,request.sources);
  if (request.selection && suggestion.action.kind !== 'question') {
    for (const anchor of suggestion.evidence) {
      const source=request.sources.find(source=>source.sceneId===anchor.sceneId);
      if (source?.kind !== 'facts' && (anchor.sceneId!==request.selection.sceneId || anchor.start<request.selection.start || anchor.end>request.selection.end)) throw new Error('The local provider proposed a change outside the selected review passage.');
    }
  }
}
