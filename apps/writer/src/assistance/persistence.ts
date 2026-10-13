import { validateSource, type Suggestion } from './model';
import { importLanguagePack, type DictionaryException, type LanguagePack } from './packs';
import { validateReviewJob, type ProviderKind, type ReviewJob } from './jobs';
export interface ProviderConfiguration { kind: ProviderKind; endpoint: string; model?: string }
export interface AssistanceSettings { schemaVersion: 1; providers: ProviderConfiguration[]; exceptions: DictionaryException[]; suggestions: Suggestion[] }
export interface SettingsPayload { schemaVersion?: 1; assistance?: AssistanceSettings; [key: string]: unknown }
export interface PackPayload { schemaVersion: 1; packs: LanguagePack[] }
export interface JobPayload { schemaVersion: 1; jobs: ReviewJob[] }
export const defaultProviderConfiguration: ProviderConfiguration[] = [{ kind:'languagetool',endpoint:'http://127.0.0.1:8081' },{ kind:'llm',endpoint:'http://127.0.0.1:8080' }];
export function validateLocalEndpoint(endpoint:string):void {
  let url:URL; try { url = new URL(endpoint); } catch { throw new Error('Use an HTTP address on the local PC.'); }
  if (!/^http:\/\/(?:127\.0\.0\.1|\[::1\])(?::[0-9]{1,5})?\/?$/.test(endpoint) || url.protocol !== 'http:' || !['127.0.0.1','[::1]'].includes(url.hostname) || url.username || url.password || url.search || url.hash || url.pathname !== '/' || endpoint.length > 200) throw new Error('Providers must use literal loopback HTTP addresses without credentials, redirects, paths, or query parameters.');
}
function text(value:unknown,max:number):boolean { return typeof value === 'string' && value.length > 0 && value.length <= max; }
/** Historical decisions can outlive scene revisions. Exact text is rechecked when presented/applied. */
export function validateStoredSuggestion(value: Suggestion):void {
  if (!value || !text(value.id,300) || !text(value.title,300) || typeof value.explanation !== 'string' || value.explanation.length > 5000 || !['manual','rule','proofreading','llm','lexical'].includes(value.origin) || !['pending','accepted','rejected'].includes(value.status)) throw new Error('Invalid saved review suggestion.');
  if (!Array.isArray(value.sources) || !value.sources.length || value.sources.length > 50 || !Array.isArray(value.evidence) || value.evidence.length > 100) throw new Error('Invalid saved review scope.');
  value.sources.forEach(validateSource);
  if (new Set(value.sources.map(source => source.sceneId + ':' + source.revisionId)).size !== value.sources.length) throw new Error('Duplicate saved review source.');
  for (const anchor of value.evidence) {
    validateSource(anchor);
    if (!value.sources.some(source => source.sceneId === anchor.sceneId && source.revisionId === anchor.revisionId && source.hash === anchor.hash) || anchor.offsetUnit !== 'utf16' || !Number.isSafeInteger(anchor.start) || !Number.isSafeInteger(anchor.end) || anchor.start < 0 || anchor.end <= anchor.start || typeof anchor.quote !== 'string' || anchor.quote.length !== anchor.end - anchor.start || typeof anchor.prefix !== 'string' || anchor.prefix.length > 80 || typeof anchor.suffix !== 'string' || anchor.suffix.length > 80) throw new Error('Invalid saved review evidence.');
  }
  if (!value.action || !['question','replace','fact'].includes(value.action.kind)) throw new Error('Invalid saved review action.');
  if (value.action.kind === 'replace' && (value.evidence.length !== 1 || typeof value.action.replacement !== 'string' || value.action.replacement.length > 10000)) throw new Error('Invalid saved replacement.');
  if (value.action.kind === 'fact' && (!value.evidence.length || ![value.action.subject,value.action.property,value.action.value].every(field => text(field,1000)))) throw new Error('Invalid saved fact proposal.');
}
export function validateSettingsPayload(payload:SettingsPayload):AssistanceSettings {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload) || (payload.schemaVersion !== undefined && payload.schemaVersion !== 1)) throw new Error('Unsupported project settings.');
  const settings = payload.assistance ?? { schemaVersion:1,providers:structuredClone(defaultProviderConfiguration),exceptions:[],suggestions:[] };
  if (settings.schemaVersion !== 1 || !Array.isArray(settings.providers) || settings.providers.length !== 2 || new Set(settings.providers.map(provider => provider.kind)).size !== 2) throw new Error('Invalid saved local-provider configuration.');
  for (const provider of settings.providers) { if (!['languagetool','llm'].includes(provider.kind) || (provider.model !== undefined && !text(provider.model,200))) throw new Error('Invalid local model identifier.'); validateLocalEndpoint(provider.endpoint); }
  if (!Array.isArray(settings.exceptions) || settings.exceptions.length > 10000 || settings.exceptions.some(entry => !text(entry.term,200) || !text(entry.language,50) || !text(entry.reason,1000))) throw new Error('Invalid saved spelling exceptions.');
  if (!Array.isArray(settings.suggestions) || settings.suggestions.length > 10000) throw new Error('Saved suggestion count exceeds the supported limit.');
  settings.suggestions.forEach(validateStoredSuggestion);
  if (new Set(settings.suggestions.map(suggestion => suggestion.id)).size !== settings.suggestions.length) throw new Error('Duplicate saved suggestion identity.');
  return structuredClone(settings);
}
export async function validatePackPayload(payload:PackPayload):Promise<PackPayload> {
  if (!payload || payload.schemaVersion !== 1 || !Array.isArray(payload.packs) || payload.packs.length > 20 || new TextEncoder().encode(JSON.stringify(payload)).length > 8000000) throw new Error('Invalid saved language-pack collection.');
  const packs = await Promise.all(payload.packs.map(pack => importLanguagePack(JSON.stringify(pack))));
  if (new Set(packs.map(pack => pack.id)).size !== packs.length) throw new Error('Duplicate installed pack identity.');
  return { schemaVersion:1,packs };
}
export async function validateJobPayload(payload:JobPayload):Promise<JobPayload> {
  if (!payload || payload.schemaVersion !== 1 || !Array.isArray(payload.jobs) || payload.jobs.length > 1000 || new TextEncoder().encode(JSON.stringify(payload)).length > 8000000) throw new Error('Invalid saved review-job collection.');
  await Promise.all(payload.jobs.map(validateReviewJob));
  if (new Set(payload.jobs.map(job => job.request.id)).size !== payload.jobs.length || new Set(payload.jobs.map(job => job.request.idempotencyKey)).size !== payload.jobs.length) throw new Error('Duplicate saved job identity or idempotency key.');
  if (payload.jobs.filter(job => ['pending_local','queued','running','cancelling'].includes(job.state)).length > 1) throw new Error('Saved queue exceeds its one-active-job limit.');
  return structuredClone(payload);
}
export interface DataCellSnapshot<T> { revisionId:string; payload:T }
export type DataRpc = <T>(operation:string,args:Record<string,unknown>)=>Promise<T>;
/** Serializes each CAS write without coupling close/drain to provider inference. */
export class ProjectDataCell<T> {
  private revisionId:string|null = null; private value:T; private tail:Promise<void> = Promise.resolve(); private initialized = false; private lastError:unknown; private failedTransform:((payload:T)=>T)|undefined;
  constructor(private rpc:DataRpc,private projectId:string,private kind:string,initial:T) { this.value = structuredClone(initial); }
  async load():Promise<T> { const snapshot = await this.rpc<DataCellSnapshot<T>|null>('load_project_data',{ projectId:this.projectId,kind:this.kind }); if (snapshot) { this.revisionId = snapshot.revisionId; this.value = snapshot.payload; } this.initialized = true; return structuredClone(this.value); }
  current():T { return structuredClone(this.value); }
  update(transform:(payload:T)=>T):Promise<T> {
    const operation = this.tail.then(async () => {
      if (!this.initialized) throw new Error('Project data is still opening.');
      const payload = transform(structuredClone(this.value));
      const saved = await this.rpc<DataCellSnapshot<T>>('save_project_data',{ projectId:this.projectId,kind:this.kind,payload,parent:this.revisionId });
      this.revisionId = saved.revisionId; this.value = saved.payload; this.lastError = undefined; this.failedTransform = undefined; return structuredClone(this.value);
    });
    this.tail = operation.then(() => undefined,error => { this.lastError = error; this.failedTransform = transform; }); return operation;
  }
  async retryFailed():Promise<void> { await this.tail; if (this.failedTransform) await this.update(this.failedTransform); }
  async drain():Promise<void> { await this.tail; if (this.lastError) throw this.lastError; }
}
