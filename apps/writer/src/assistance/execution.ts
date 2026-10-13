import { canonicalJson } from './packs';
import { transitionJob, validateReviewRequest, validateSuggestionForRequest, type ReviewJob, type ReviewRequest } from './jobs';
import { validateSuggestion, type Suggestion } from './model';
export function providerRunToken(job:ReviewJob):string { return job.request.id + ':' + job.attempt; }
/** Same key and immutable body replay the durable job; a changed body must use a new key. */
export function enqueueReviewJob(previous:readonly ReviewJob[],pending:ReviewJob):ReviewJob[] {
  const existing = previous.find(job => job.request.idempotencyKey === pending.request.idempotencyKey);
  if (existing) {
    if (canonicalJson(existing.request) !== canonicalJson(pending.request)) throw new Error('This idempotency key belongs to a different immutable request.');
    return [...previous];
  }
  if (previous.some(job => job.request.id === pending.request.id)) throw new Error('This job identity belongs to another idempotency key.');
  if (previous.some(job => ['pending_local','queued','running','cancelling'].includes(job.state))) throw new Error('Wait for the current local review or cancel it.');
  if (previous.length >= 1000) throw new Error('The retained review-job limit has been reached. Export the project before managing its history.');
  return [...previous,pending];
}
export async function prepareReviewJob(request:ReviewRequest):Promise<ReviewJob> { await validateReviewRequest(request); return { request:structuredClone(request),state:'pending_local',attempt:1,updatedAt:new Date().toISOString() }; }
/** Late/cancelled/previous-attempt output cannot become an authoritative saved result. */
export function completeReviewAttempt(current:ReviewJob,attempt:number,suggestions:Suggestion[],cancelled:boolean,at:string):ReviewJob {
  if (current.attempt !== attempt || ['succeeded','failed','cancelled','interrupted'].includes(current.state)) return current;
  if (cancelled || current.state === 'cancelling') {
    const cancelling = current.state === 'running' ? transitionJob(current,'cancelling',at) : current;
    return transitionJob(cancelling,'cancelled',at);
  }
  if (current.state !== 'running') throw new Error('Only a running attempt can publish a review result.');
  if (suggestions.length > 100) throw new Error('Too many review suggestions.'); suggestions.forEach(suggestion => validateSuggestionForRequest(suggestion,current.request));
  return { ...transitionJob(current,'succeeded',at),suggestions:structuredClone(suggestions) };
}
