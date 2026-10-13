import React, { useEffect, useMemo, useRef, useState } from 'react';
import { workspaceCommand } from '../workspace';
import { completeReviewAttempt, enqueueReviewJob, prepareReviewJob, providerRunToken } from './execution';
import { AssistanceWorkspace, type AssistanceWorkspaceProps } from './AssistanceWorkspace';
import { freshness, sha256, validateSuggestion, type Suggestion } from './model';
import { canSubmit, recoverInterruptedJobs, retryJob, transitionJob, unavailableProviders, validateReviewRequest, validateSuggestionForRequest, type ProviderStatus, type ReviewJob, type ReviewRequest } from './jobs';
import { filterProofreadingExceptions, type LanguagePack } from './packs';
import { ProjectDataCell, defaultProviderConfiguration, validateJobPayload, validateLocalEndpoint, validatePackPayload, validateSettingsPayload, type AssistanceSettings, type JobPayload, type PackPayload, type ProviderConfiguration, type SettingsPayload } from './persistence';

export interface NativeAssistanceWorkspaceProps extends Pick<AssistanceWorkspaceProps,'projectId'|'scene'|'sources'|'continuity'|'selectedPassage'|'onApplyPatch'|'onApproveFact'|'pageActions'> {
  onRegisterFlush?: (flush:(()=>Promise<void>)|null)=>void;
}
function errorMessage(error:unknown):string { return error instanceof Error ? error.message : String(error); }
function checkedStatus(status:ProviderStatus,kind:string):ProviderStatus {
  if (!status || status.kind !== kind || !['unavailable','loading','ready','busy','error'].includes(status.state) || typeof status.label !== 'string' || typeof status.detail !== 'string' || !Array.isArray(status.languages) || status.languages.some(value => typeof value !== 'string')) throw new Error('The local provider returned an invalid status.'); return status;
}
export function NativeAssistanceWorkspace(props:NativeAssistanceWorkspaceProps) {
  const cells = useMemo(() => ({ packs:new ProjectDataCell<PackPayload>(workspaceCommand,props.projectId,'languagePacks',{ schemaVersion:1,packs:[] }), jobs:new ProjectDataCell<JobPayload>(workspaceCommand,props.projectId,'assistanceJobs',{ schemaVersion:1,jobs:[] }),settings:new ProjectDataCell<SettingsPayload>(workspaceCommand,props.projectId,'settings',{ schemaVersion:1 }) }),[props.projectId]);
  const [ready,setReady] = useState(false); const [error,setError] = useState(''); const [actionBusy,setActionBusy] = useState(false);
  const [packs,setPacks] = useState<LanguagePack[]>([]); const [jobs,setJobs] = useState<ReviewJob[]>([]);
  const [settings,setSettings] = useState<AssistanceSettings>({ schemaVersion:1,providers:structuredClone(defaultProviderConfiguration),exceptions:[],suggestions:[] });
  const [draftConfig,setDraftConfig] = useState<ProviderConfiguration[]>(structuredClone(defaultProviderConfiguration));
  const [providers,setProviders] = useState<ProviderStatus[]>(unavailableProviders.map(provider => ({ ...provider,detail:'Status has not been checked. Refresh the configured local provider explicitly.' })));
  const cancelled = useRef(new Set<string>()); const running = useRef(new Set<string>()); const mounted = useRef(false);
  const setJobsIfMounted = (next:ReviewJob[]) => { if (mounted.current) setJobs(next); };
  useEffect(() => {
    let disposed = false; mounted.current = true; setReady(false); setError('');
    void (async () => {
      const loaded = await Promise.all([cells.packs.load(),cells.jobs.load(),cells.settings.load()]);
      const [packData,jobData] = await Promise.all([validatePackPayload(loaded[0]),validateJobPayload(loaded[1])]); const savedSettings = validateSettingsPayload(loaded[2]);
      let recovered = recoverInterruptedJobs(jobData.jobs,new Date().toISOString());
      // No provider invocation is automatically retried after opening the app.
      recovered = recovered.map(job => job.state === 'queued' || job.state === 'pending_local' ? { ...job,state:'interrupted' as const,error:'The saved request was not completed. Retry explicitly.',updatedAt:new Date().toISOString() } : job);
      if (JSON.stringify(recovered) !== JSON.stringify(jobData.jobs)) await cells.jobs.update(() => ({ schemaVersion:1,jobs:recovered }));
      if (disposed) return; setPacks(packData.packs); setJobs(recovered); setSettings(savedSettings); setDraftConfig(savedSettings.providers); setReady(true);
    })().catch(error => { if (!disposed) setError('Could not open assistance data. Nothing has been overwritten. ' + errorMessage(error)); });
    return () => { disposed = true; mounted.current = false; };
  },[cells]);
  useEffect(() => { props.onRegisterFlush?.(async () => { await Promise.all([cells.packs.drain(),cells.jobs.drain(),cells.settings.drain()]); }); return () => props.onRegisterFlush?.(null); },[cells,props.onRegisterFlush]);
  async function persistSettings(change:(value:AssistanceSettings)=>AssistanceSettings):Promise<void> {
    const payload = await cells.settings.update(previous => { const next = change(validateSettingsPayload(previous)); validateSettingsPayload({ ...previous,assistance:next }); return { ...previous,assistance:next }; });
    const next = validateSettingsPayload(payload); if (mounted.current) setSettings(next);
  }
  async function persistJobs(change:(value:ReviewJob[])=>ReviewJob[]):Promise<ReviewJob[]> {
    const saved = await cells.jobs.update(previous => ({ schemaVersion:1,jobs:change(previous.jobs) })); setJobsIfMounted(saved.jobs); return saved.jobs;
  }
  async function move(id:string,next:ReviewJob['state'],detail?:string):Promise<ReviewJob> {
    const saved = await persistJobs(previous => previous.map(job => job.request.id === id ? transitionJob(job,next,new Date().toISOString(),detail) : job)); const updated = saved.find(job => job.request.id === id); if (!updated) throw new Error('Saved review job is unavailable.'); return updated;
  }
  async function execute(job:ReviewJob):Promise<void> {
    const id = job.request.id; if (running.current.has(id)) return; running.current.add(id);
    try {
      if (cancelled.current.has(id)) { await move(id,'cancelled'); return; }
      await move(id,'running');
      if (cancelled.current.has(id)) { await move(id,'cancelled'); return; }
      const config = validateSettingsPayload(cells.settings.current()).providers.find(provider => provider.kind === job.request.provider)!;
      validateLocalEndpoint(config.endpoint);
      const result = await workspaceCommand<{ suggestions:Suggestion[]; provider:string; model?:string; runtimeVersion?:string; modelHash?:string }>('provider_run',{ request:job.request,endpoint:config.endpoint,model:config.model ?? null,runToken:providerRunToken(job) });
      if (cancelled.current.has(id)) { await move(id,'cancelled'); return; }
      if (!result || typeof result.provider !== 'string' || result.provider.length > 300 || (result.model !== undefined && (typeof result.model !== 'string' || result.model.length > 300)) || !Array.isArray(result.suggestions) || result.suggestions.length > 100) throw new Error('Local provider returned an invalid or excessive result.');
      for (const suggestion of result.suggestions) {
        if (suggestion.origin !== (job.request.provider === 'llm' ? 'llm' : 'proofreading') || suggestion.status !== 'pending') throw new Error('Local results must remain pending proposals from the requested provider.'); validateSuggestionForRequest(suggestion,job.request);
      }
      for (const identity of [result.runtimeVersion,result.modelHash]) if (identity !== undefined && (typeof identity !== 'string' || identity.length > 300)) throw new Error('Invalid local result identity.');
      const proposalIds = new Set<string>();
      const proposals = await Promise.all(filterProofreadingExceptions(result.suggestions,validateSettingsPayload(cells.settings.current()).exceptions,job.request.language).map(async suggestion => {
        if (proposalIds.has(suggestion.id)) throw new Error('Duplicate local suggestion identity.'); proposalIds.add(suggestion.id);
        return { ...suggestion,sources:job.request.sources.map(source => ({ sceneId:source.sceneId,revisionId:source.revisionId,hash:source.hash })),id:'job-' + await sha256(id + '\u0000' + suggestion.id) };
      }));
      await persistJobs(previous => previous.map(item => item.request.id === id ? { ...completeReviewAttempt(item,job.attempt,proposals,cancelled.current.has(id),new Date().toISOString()),resultProvider:result.provider,resultModel:result.model,resultRuntime:result.runtimeVersion,resultModelHash:result.modelHash } : item));
    } catch (error) {
      try {
        const current = cells.jobs.current().jobs.find(item => item.request.id === id);
        if (current && ['queued','running','cancelling'].includes(current.state)) await move(id,cancelled.current.has(id) ? 'cancelled' : 'failed',errorMessage(error));
      } catch (storageError) { if (mounted.current) setError('The review state could not be saved. ' + errorMessage(storageError)); }
    } finally { running.current.delete(id); }
  }
  async function submit(request:ReviewRequest):Promise<ReviewJob> {
    await validateReviewRequest(request); const status = providers.find(provider => provider.kind === request.provider);
    if (!canSubmit(status,request.language)) throw new Error('Refresh a ready local provider before submitting this language.');
    const pending = await prepareReviewJob(request);
    const saved = await persistJobs(previous => enqueueReviewJob(previous,pending));
    const existing = saved.find(job => job.request.idempotencyKey === request.idempotencyKey)!;
    if (existing.state !== 'pending_local') return existing;
    const queued = await move(existing.request.id,'queued'); void execute(queued); return queued;
  }
  async function cancel(id:string):Promise<void> {
    const current = cells.jobs.current().jobs.find(job => job.request.id === id); if (!current) throw new Error('Review job not found.');
    if (['succeeded','failed','interrupted','cancelled'].includes(current.state)) return;
    cancelled.current.add(id);
    if (current.state === 'pending_local' || (current.state === 'queued' && !running.current.has(id))) { await move(id,'cancelled'); return; }
    if (current.state !== 'cancelling') await move(id,'cancelling');
    await workspaceCommand('provider_cancel',{ jobId:id,runToken:providerRunToken(current) });
    const latest = cells.jobs.current().jobs.find(job => job.request.id === id); if (latest?.state === 'cancelling') await move(id,'cancelled');
  }
  async function retry(id:string):Promise<ReviewJob> {
    const existing = cells.jobs.current().jobs.find(job => job.request.id === id); if (!existing) throw new Error('Review job not found.');
    if (!canSubmit(providers.find(provider => provider.kind === existing.request.provider),existing.request.language)) throw new Error('Refresh the local provider before retrying.');
    if (cells.jobs.current().jobs.some(job => ['pending_local','queued','running','cancelling'].includes(job.state))) throw new Error('Another review is active.');
    const pending = retryJob(existing,new Date().toISOString()); cancelled.current.delete(id);
    await persistJobs(previous => previous.map(job => job.request.id === id ? pending : job)); const queued = await move(id,'queued'); void execute(queued); return queued;
  }
  async function refreshStatus():Promise<void> {
    setActionBusy(true); setError('');
    try {
      for (const provider of draftConfig) validateLocalEndpoint(provider.endpoint);
      await persistSettings(previous => ({ ...previous,providers:structuredClone(draftConfig) }));
      const statuses = await Promise.all(draftConfig.map(async config => {
        try { return checkedStatus(await workspaceCommand<ProviderStatus>('provider_status',{ kind:config.kind,endpoint:config.endpoint,model:config.model ?? null }),config.kind); }
        catch (error) { return { ...unavailableProviders.find(provider => provider.kind === config.kind)!,state:'error' as const,detail:errorMessage(error) }; }
      })); if (mounted.current) setProviders(statuses);
    } catch (error) { setError(errorMessage(error)); } finally { setActionBusy(false); }
  }
  async function retrySaves():Promise<void> {
    setActionBusy(true); try { await Promise.all([cells.packs.retryFailed(),cells.jobs.retryFailed(),cells.settings.retryFailed()]); const storedJobs = await validateJobPayload(cells.jobs.current()); const abandoned = storedJobs.jobs.map(job => ['pending_local','queued','running','cancelling'].includes(job.state) && !running.current.has(job.request.id) ? { ...job,state:'interrupted' as const,error:'This attempt was not completed. Retry explicitly.',updatedAt:new Date().toISOString() } : job); if (JSON.stringify(abandoned) !== JSON.stringify(storedJobs.jobs)) await persistJobs(() => abandoned); else setJobs(storedJobs.jobs); setPacks((await validatePackPayload(cells.packs.current())).packs); setSettings(validateSettingsPayload(cells.settings.current())); setError(''); } catch (error) { setError(errorMessage(error)); } finally { setActionBusy(false); }
  }
  const controls = <details className="assistance-card"><summary>Connection settings</summary>{draftConfig.map(config => <div key={config.kind}><label>{config.kind === 'llm' ? 'Local model endpoint' : 'LanguageTool endpoint'}<input type="url" value={config.endpoint} onChange={event => setDraftConfig(previous => previous.map(value => value.kind === config.kind ? { ...value,endpoint:event.target.value } : value))}/></label>{config.kind === 'llm' && <label>Model (optional)<input value={config.model ?? ''} maxLength={200} onChange={event => setDraftConfig(previous => previous.map(value => value.kind === config.kind ? { ...value,model:event.target.value || undefined } : value))}/></label>}</div>)}<button disabled={actionBusy} onClick={() => void refreshStatus()}>Check connection</button><p className="assistance-muted">Start the local service before checking its connection.</p></details>;
  const currentSources = [...(props.sources ?? []),...(props.scene ? [props.scene] : [])];
  return <>{(error||!ready)&&<div className="assistance">{!ready&&props.pageActions}{error && <p className="assistance-error" role="alert">{error} {ready && <button disabled={actionBusy} onClick={() => void retrySaves()}>Retry saves</button>}</p>}{!ready && !error && <p role="status">Opening saved reviews…</p>}</div>}{ready && <AssistanceWorkspace {...props} packs={packs} jobs={jobs} providers={providers} exceptions={settings.exceptions} suggestions={settings.suggestions} providerControls={controls}
    onPersistPacks={async next => { const payload = await validatePackPayload({ schemaVersion:1,packs:next }); await cells.packs.update(() => payload); setPacks(next); }}
    onPersistExceptions={async next => { await persistSettings(previous => ({ ...previous,exceptions:next })); }}
    onPersistSuggestions={async next => { await persistSettings(previous => ({ ...previous,suggestions:next })); }}
    onSubmitJob={submit} onCancelJob={cancel} onRetryJob={retry}
    onRefreshJobs={async () => { await cells.jobs.drain(); setJobs((await validateJobPayload(cells.jobs.current())).jobs); }}
    onApplyPatch={props.onApplyPatch ? async patch => { const known = [...settings.suggestions,...jobs.flatMap(job => job.suggestions ?? [])].find(suggestion => suggestion.id === patch.suggestionId); if (known && !freshness(known,currentSources).fresh) throw new Error('A reviewed source changed before acceptance.'); return props.onApplyPatch!(patch); } : undefined}/>}</>;
}
