import type { EvidenceAnchor, SourceRef, Suggestion } from './model';
export interface ContinuityEvent { id: string; label: string; start?: number; end?: number; confirmed: boolean }
export interface ContinuityEvidence { sources: SourceRef[]; evidence: EvidenceAnchor[] }
export interface ContinuityWorld {
  events: ContinuityEvent[];
  before: { id: string; beforeId: string; afterId: string; confirmed: boolean }[];
  prerequisites?: ({ id: string; dependentEventId: string; prerequisiteEventId: string; confirmed: boolean } & ContinuityEvidence)[];
  knowledge: ({ id: string; characterId: string; factId: string; eventId: string; kind: 'learn' | 'use'; confirmed: boolean; exception?: string } & ContinuityEvidence)[];
  presence: ({ id: string; subjectId: string; locationId: string; eventId: string; exclusive: boolean; confirmed: boolean; exception?: string } & ContinuityEvidence)[];
  states: ({ id: string; subjectId: string; property: string; value: string; eventId: string; untilEventId?: string; confirmed: boolean; exception?: string } & ContinuityEvidence)[];
  setups: ({ id: string; label: string; requiresPayoff: boolean; payoffId?: string; closed: boolean } & ContinuityEvidence)[];
  sources: SourceRef[];
}
export interface RuleFinding { id: string; ruleId: string; title: string; explanation: string; recordIds: string[]; sources: SourceRef[]; evidence: EvidenceAnchor[] }
export interface ContinuityRule { id: string; label: string; assumptions: string; check: (world: ContinuityWorld) => RuleFinding[] }
function bounds(world: ContinuityWorld, id: string): [number,number] | undefined {
  const event = world.events.find(event => event.id === id);
  return event?.confirmed && Number.isFinite(event.start) && Number.isFinite(event.end) && event.start! <= event.end! ? [event.start!, event.end!] : undefined;
}
function evidence(...records: ContinuityEvidence[]): ContinuityEvidence {
  const refs = new Map(records.flatMap(record => record.sources).map(ref => [ref.sceneId + ':' + ref.revisionId, ref]));
  return { sources: [...refs.values()], evidence: records.flatMap(record => record.evidence) };
}
function finding(ruleId: string, title: string, explanation: string, recordIds: string[], refs: ContinuityEvidence): RuleFinding {
  return { id: ruleId + ':' + recordIds.join(':'), ruleId, title, explanation, recordIds, ...refs };
}
function before(world: ContinuityWorld, a: string, b: string): boolean {
  const seen = new Set<string>(); const pending = [a];
  while (pending.length) { const id = pending.pop()!; if (seen.has(id)) continue; seen.add(id); for (const edge of world.before.filter(edge => edge.confirmed && edge.beforeId === id)) { if (edge.afterId === b) return true; pending.push(edge.afterId); } }
  const first = bounds(world,a), second = bounds(world,b); return Boolean(first && second && first[1] < second[0]);
}
export const continuityRules: readonly ContinuityRule[] = [
  { id: 'missing-prerequisite', label: 'Missing prerequisite', assumptions: 'Only author-confirmed prerequisite links between canonical events with known strict order; missing notes and unknown times are not proof.', check(world) {
    return (world.prerequisites ?? []).filter(link => link.confirmed && before(world,link.dependentEventId,link.prerequisiteEventId) && !before(world,link.prerequisiteEventId,link.dependentEventId)).map(link => finding('missing-prerequisite','An event precedes its confirmed prerequisite','The dependent event is established before the prerequisite event it explicitly requires. Check the declared dependency or chronology.',[link.id,link.dependentEventId,link.prerequisiteEventId],evidence(link)));
  } },
  { id: 'chronology-cycle', label: 'Strict chronology cycle', assumptions: 'Only confirmed happens-before relationships participate.', check(world) {
    const results: RuleFinding[] = []; const reported = new Set<string>();
    for (const edge of world.before.filter(edge => edge.confirmed)) {
      const path: typeof world.before = []; const seen = new Set<string>();
      const visit = (id: string): boolean => { if (id === edge.beforeId) return true; if (seen.has(id)) return false; seen.add(id); for (const next of world.before.filter(next => next.confirmed && next.beforeId === id)) { path.push(next); if (visit(next.afterId)) return true; path.pop(); } return false; };
      if (visit(edge.afterId)) { const ids = [edge,...path].map(edge => edge.id).sort(); const key = ids.join(':'); if (!reported.has(key)) { reported.add(key); results.push(finding('chronology-cycle','Chronology cannot satisfy this cycle','These confirmed events are each required to happen before another event in the same cycle.',ids,{ sources: world.sources, evidence: [] })); } }
    } return results;
  } },
  { id: 'knowledge-before-learning', label: 'Knowledge used too early', assumptions: 'Confirmed use and learning records with explicit order; an exception suppresses the finding.', check(world) {
    return world.knowledge.filter(use => use.confirmed && use.kind === 'use' && !use.exception).flatMap(use => {
      const learns = world.knowledge.filter(learn => learn.confirmed && learn.kind === 'learn' && learn.characterId === use.characterId && learn.factId === use.factId);
      if (!learns.length || learns.some(learn => learn.exception || learn.eventId === use.eventId || !before(world,use.eventId,learn.eventId) || before(world,learn.eventId,use.eventId))) return [];
      return [finding('knowledge-before-learning','Knowledge is used before its recorded acquisition','The confirmed use precedes every recorded learning event. An off-page explanation or intentional exception can resolve this.',[use.id,...learns.map(learn => learn.id)],evidence(use,...learns))];
    });
  } },
  { id: 'exclusive-presence', label: 'Incompatible simultaneous presence', assumptions: 'Two confirmed, explicitly exclusive locations during overlapping established intervals; no travel-speed assumption.', check(world) {
    const results: RuleFinding[] = []; const records = world.presence.filter(record => record.confirmed && record.exclusive && !record.exception);
    for (let i = 0; i < records.length; i++) for (const right of records.slice(i + 1)) {
      const left = records[i], a = bounds(world,left.eventId), b = bounds(world,right.eventId);
      if (left.subjectId === right.subjectId && left.locationId !== right.locationId && a && b && Math.max(a[0],b[0]) <= Math.min(a[1],b[1])) results.push(finding('exclusive-presence','Two exclusive locations overlap','These records explicitly require the same subject in different exclusive locations at overlapping story times.',[left.id,right.id],evidence(left,right)));
    } return results;
  } },
  { id: 'confirmed-state-conflict', label: 'Conflicting confirmed state', assumptions: 'Confirmed values for one property with explicitly bounded overlapping applicability. An unknown ending is not treated as infinite.', check(world) {
    const results: RuleFinding[] = []; const records = world.states.filter(record => record.confirmed && !record.exception);
    function interval(record: typeof records[number]): [number,number] | undefined { const start = bounds(world,record.eventId); const end = record.untilEventId ? bounds(world,record.untilEventId) : start; return start && end && start[0] <= end[1] ? [start[0],end[1]] : undefined; }
    for (let i = 0; i < records.length; i++) for (const right of records.slice(i + 1)) { const left = records[i], a = interval(left), b = interval(right);
      if (left.subjectId === right.subjectId && left.property === right.property && left.value !== right.value && a && b && Math.max(a[0],b[0]) <= Math.min(a[1],b[1])) results.push(finding('confirmed-state-conflict','Confirmed states disagree','Different confirmed values apply to the same property during overlapping established times. Reconcile their sources.',[left.id,right.id],evidence(left,right)));
    } return results;
  } },
  { id: 'unresolved-setup', label: 'Unresolved setup reminder', assumptions: 'Only a setup explicitly requiring payoff in a closed scope; this is a reminder, not proof of a plot hole.', check(world) {
    return world.setups.filter(setup => setup.requiresPayoff && setup.closed && !setup.payoffId).map(setup => finding('unresolved-setup','Review the payoff for “' + setup.label + '”','This closed setup was explicitly marked as requiring a payoff and has no linked payoff. Check whether that is intentional.',[setup.id],evidence(setup)));
  } },
];
export function checkContinuity(world: ContinuityWorld, enabled: readonly string[] = continuityRules.map(rule => rule.id)): RuleFinding[] {
  return continuityRules.filter(rule => enabled.includes(rule.id)).flatMap(rule => rule.check(world)).map(finding => ({ ...finding,sources:[...new Map([...finding.sources,...world.sources.filter(source => source.sceneId.startsWith('planning:'))].map(source => [source.sceneId + ':' + source.revisionId,source])).values()] }));
}
export function continuitySuggestion(finding: RuleFinding): Suggestion | undefined {
  if (!finding.sources.length) return undefined;
  return { id: finding.id, ruleId: finding.ruleId, origin: 'rule', title: finding.title, explanation: finding.explanation, sources: finding.sources, evidence: finding.evidence, action: { kind: 'question' }, status: 'pending' };
}
