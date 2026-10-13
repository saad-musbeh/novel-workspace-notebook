/** Story-world time is expressed in fictional day units, never device timestamps. */
export type RecordKind = 'character' | 'location' | 'item';
export type EpistemicStatus = 'fact' | 'belief' | 'rumor' | 'proposal';
export type FieldKind = 'text' | 'long-text' | 'number' | 'date' | 'single-choice' | 'multiple-choice' | 'links';
export interface RecordLink { kind: 'record' | 'scene' | 'event' | 'plot-point'; id: string }
export interface FieldDefinition { id: string; label: string; kind: FieldKind; recordKind: RecordKind; hidden: boolean; options: string[] }
export type FieldValue = string | number | string[] | RecordLink[];
export interface StoryRecord { id: string; kind: RecordKind; name: string; aliases: string[]; notes: string; values: Record<string, FieldValue>; links: RecordLink[] }
export interface StoryFact { id: string; subjectId: string; property: string; value: string; status: EpistemicStatus; confirmed: boolean; observerId: string | null; source: string; eventId: string | null; untilEventId: string | null }
export type StoryTime = { kind: 'unknown' } | { kind: 'fixed'; day: number; label: string } | { kind: 'interval'; start: number; end: number; label: string } | { kind: 'bounds'; start: number; end: number; label: string } | { kind: 'relative'; eventId: string; offsetDays: number };
export interface StoryEvent { id: string; title: string; notes: string; laneIds: string[]; sceneIds: string[]; time: StoryTime }
export interface PlotLane { id: string; name: string; kind: 'main' | 'subplot' }
export interface TimeConstraint { id: string; beforeId: string; afterId: string; minimumDays: number; strict: boolean; confirmed: boolean; notes: string }
export interface PlotPoint { id: string; title: string; notes: string; links: RecordLink[]; requiresPayoff: boolean; scopeClosed: boolean; payoffId: string | null }
export interface BoardCard { id: string; title: string; body: string; kind: 'plot-point' | 'note' | 'record' | 'scene' | 'event'; reference: RecordLink | null; x: number; y: number; groupId: string | null; labels: string[] }
export interface BoardGroup { id: string; name: string }
export type EdgeKind = 'reference' | 'causes' | 'happens-before' | 'reveals' | 'depends-on' | 'setup/payoff' | 'custom';
export interface BoardEdge { id: string; fromId: string; toId: string; kind: EdgeKind; label: string; confirmed?: boolean }
export interface StoryBoard { id: string; name: string; cards: BoardCard[]; groups: BoardGroup[]; edges: BoardEdge[] }
export type IssueStatus = 'open' | 'investigating' | 'intentional' | 'resolved' | 'dismissed' | 'stale';
export interface IssueEvidence { id: string; link: RecordLink | null; quote: string; sourceRevision: string; notes: string }
export interface ContinuityIssue { id: string; category: string; description: string; importance: 'low' | 'medium' | 'high'; links: RecordLink[]; evidence: IssueEvidence[]; status: IssueStatus; resolution: string }
export interface ReaderReveal { id:string; eventId:string|null; factId:string|null; scene:{kind:'scene';id:string}; position:number; notes:string }
export type KnowledgeScope = {kind:'scenes';sceneIds:string[]} | {kind:'events';fromEventId:string;untilEventId:string|null};
export interface CharacterKnowledge { id:string; characterId:string; factId:string; stance:'knows'|'believes'|'misunderstands'; factSnapshot:{subjectId:string;property:string;value:string}; interpretation:string; source:string; scope:KnowledgeScope }
export interface PlanningDocument { readerReveals?:ReaderReveal[]; knowledge?:CharacterKnowledge[]; schemaVersion: 1; fields: FieldDefinition[]; records: StoryRecord[]; plotPoints: PlotPoint[]; facts: StoryFact[]; boards: StoryBoard[]; lanes: PlotLane[]; events: StoryEvent[]; constraints: TimeConstraint[]; issues: ContinuityIssue[] }
export interface PlanningSnapshot { document: PlanningDocument; revisionId: string | null }
export interface PlanningAdapter { load(projectId: string): Promise<PlanningSnapshot>; save(projectId: string, document: PlanningDocument, expectedRevisionId: string | null): Promise<PlanningSnapshot> }
export const newId = () => crypto.randomUUID();
export function createPlanningDocument(): PlanningDocument {
  return { schemaVersion: 1, fields: [], records: [], plotPoints: [], facts: [], boards: [{ id: newId(), name: 'Story board', cards: [], edges: [], groups: [] }], lanes: [{ id: newId(), name: 'Main plot', kind: 'main' }], events: [], constraints: [], issues: [] };
}
function requireValid(condition: unknown, message: string): asserts condition { if (!condition) throw new Error(message); }
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const text = (value: unknown): value is string => typeof value === 'string';
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
function strings(value: unknown): value is string[] { return Array.isArray(value) && value.every(text); }
const kinds: RecordKind[] = ['character', 'location', 'item'];
const fieldKinds: FieldKind[] = ['text', 'long-text', 'number', 'date', 'single-choice', 'multiple-choice', 'links'];
export function validatePlanningDocument(value: unknown): asserts value is PlanningDocument {
  requireValid(object(value) && value.schemaVersion === 1, 'Unsupported planning schema version. Stored planning data has not been changed.');
  const doc = value as unknown as PlanningDocument;
  const allIds = new Set<string>();
  for (const key of ['fields','records','plotPoints','facts','boards','lanes','events','constraints','issues'] as const) {
    requireValid(Array.isArray(doc[key]), `Invalid planning ${key}.`);
    for (const entry of doc[key]) { requireValid(object(entry) && text(entry.id) && entry.id.length > 0 && !allIds.has(entry.id), 'Planning IDs must be unique and stable.'); allIds.add(entry.id); }
  }
  const recordIds = new Set(doc.records.map(r => r.id)); const eventIds = new Set(doc.events.map(e => e.id)); const laneIds = new Set(doc.lanes.map(l => l.id));
  const recordsById=new Map(doc.records.map(r=>[r.id,r]));const factsById=new Map(doc.facts.map(f=>[f.id,f]));
  for(const key of ['readerReveals','knowledge'] as const){
    if(doc[key]===undefined)continue;
    requireValid(Array.isArray(doc[key])&&doc[key]!.length<=10000,`Invalid or oversized planning ${key}.`);
    for(const entry of doc[key]!){requireValid(object(entry)&&text(entry.id)&&entry.id.length>0&&!allIds.has(entry.id),'Planning IDs must be unique and stable.');allIds.add(entry.id);}
  }
  for(const reveal of doc.readerReveals??[])requireValid(object(reveal.scene)&&reveal.scene.kind==='scene'&&text(reveal.scene.id)&&reveal.scene.id.length>0&&Number.isSafeInteger(reveal.position)&&reveal.position>=0&&text(reveal.notes)&&((reveal.eventId===null&&factsById.has(reveal.factId!))||(reveal.factId===null&&eventIds.has(reveal.eventId!))),'A reader reveal needs one canonical event or fact and an explicit scene/order.');
  for(const knowledge of doc.knowledge??[]){
    requireValid(recordsById.get(knowledge.characterId)?.kind==='character'&&factsById.has(knowledge.factId)&&['knows','believes','misunderstands'].includes(knowledge.stance)&&text(knowledge.interpretation)&&text(knowledge.source)&&knowledge.source.trim().length>0&&object(knowledge.scope),'Knowledge needs a character, canonical fact, stance and author source.');
    requireValid(object(knowledge.factSnapshot)&&text(knowledge.factSnapshot.subjectId)&&text(knowledge.factSnapshot.property)&&text(knowledge.factSnapshot.value)&&(knowledge.stance==='knows'||knowledge.interpretation.trim().length>0),'Knowledge needs an exact fact snapshot; beliefs and misunderstandings need the character’s interpretation.');
    const scope=knowledge.scope;
    requireValid((scope.kind==='scenes'&&strings(scope.sceneIds)&&scope.sceneIds.length>0&&new Set(scope.sceneIds).size===scope.sceneIds.length&&scope.sceneIds.every(id=>id.length>0))||(scope.kind==='events'&&eventIds.has(scope.fromEventId)&&(scope.untilEventId===null||(eventIds.has(scope.untilEventId)&&scope.untilEventId!==scope.fromEventId))),'Knowledge applicability needs explicit scenes or a valid event range.');
  }

  const linkValid = (link: unknown) => object(link) && ['record','event','scene','plot-point'].includes(String(link.kind)) && text(link.id) && link.id.length > 0;
  const linksValid = (links: unknown) => Array.isArray(links) && links.every(linkValid);
  // Dangling links remain valid: deletion of an external scene must not erase evidence.
  for (const field of doc.fields) requireValid(kinds.includes(field.recordKind) && fieldKinds.includes(field.kind) && text(field.label) && typeof field.hidden === 'boolean' && strings(field.options), 'Invalid custom field definition.');
  for (const record of doc.records) {
    requireValid(kinds.includes(record.kind) && text(record.name) && text(record.notes) && strings(record.aliases) && object(record.values) && linksValid(record.links), 'Invalid story record.');
    for (const [id, val] of Object.entries(record.values)) {
      const field = doc.fields.find(f => f.id === id && f.recordKind === record.kind);
      requireValid(!!field, 'Custom field values require a retained field definition.');
      const valid = field.kind === 'number' ? finite(val) : field.kind === 'multiple-choice' ? strings(val) : field.kind === 'links' ? linksValid(val) : text(val);
      requireValid(valid, `Invalid value for ${field.label}.`);
    }
  }
  for (const point of doc.plotPoints) requireValid(text(point.title) && text(point.notes) && linksValid(point.links) && typeof point.requiresPayoff === 'boolean' && typeof point.scopeClosed === 'boolean' && (point.payoffId === null || doc.plotPoints.some(p=>p.id===point.payoffId)), 'Invalid canonical plot point or payoff link.');
  for (const fact of doc.facts) requireValid(recordIds.has(fact.subjectId) && text(fact.property) && text(fact.value) && ['fact','belief','rumor','proposal'].includes(fact.status) && typeof fact.confirmed === 'boolean' && (!fact.confirmed || fact.status === 'fact') && text(fact.source) && (fact.observerId === null || recordIds.has(fact.observerId)) && (fact.eventId === null || eventIds.has(fact.eventId)) && (fact.untilEventId === null || eventIds.has(fact.untilEventId)), 'A confirmed fact must be explicitly approved canon with valid applicability.');
  requireValid(doc.lanes.filter(l => l.kind === 'main').length === 1, 'The chronology must have exactly one main plot lane.');
  for (const lane of doc.lanes) requireValid(text(lane.name) && ['main','subplot'].includes(lane.kind), 'Invalid plot lane.');
  for (const event of doc.events) {
    requireValid(text(event.title) && text(event.notes) && strings(event.laneIds) && new Set(event.laneIds).size === event.laneIds.length && event.laneIds.every(id => laneIds.has(id)) && strings(event.sceneIds), 'Event lane membership must use shared canonical event IDs.');
    const time = event.time;
    requireValid(object(time), 'Invalid story time.');
    requireValid(time.kind === 'unknown' || (time.kind === 'fixed' && finite(time.day) && text(time.label)) || ((time.kind === 'interval' || time.kind === 'bounds') && finite(time.start) && finite(time.end) && time.start <= time.end && text(time.label)) || (time.kind === 'relative' && eventIds.has(time.eventId) && time.eventId !== event.id && finite(time.offsetDays)), 'Invalid fictional time or relative reference.');
  }
  for (const constraint of doc.constraints) requireValid(eventIds.has(constraint.beforeId) && eventIds.has(constraint.afterId) && finite(constraint.minimumDays) && constraint.minimumDays >= 0 && typeof constraint.strict === 'boolean' && typeof constraint.confirmed === 'boolean' && text(constraint.notes), 'Invalid chronology constraint.');
  for (const board of doc.boards) {
    requireValid(text(board.name) && Array.isArray(board.cards) && Array.isArray(board.groups) && Array.isArray(board.edges), 'Invalid board.');
    const ids = new Set<string>();
    for (const entry of [...board.cards,...board.groups,...board.edges]) { requireValid(text(entry.id) && !allIds.has(entry.id) && !ids.has(entry.id), 'Board IDs must be unique.'); ids.add(entry.id); allIds.add(entry.id); }
    const cards = new Set(board.cards.map(c => c.id)); const groups = new Set(board.groups.map(g => g.id));
    for (const group of board.groups) requireValid(text(group.name), 'Invalid group.');
    for (const card of board.cards) requireValid(text(card.title) && text(card.body) && ['plot-point','note','record','scene','event'].includes(card.kind) && finite(card.x) && finite(card.y) && (card.reference === null || linkValid(card.reference)) && (card.groupId === null || groups.has(card.groupId)) && strings(card.labels), 'Invalid board card.');
    for (const edge of board.edges) requireValid(cards.has(edge.fromId) && cards.has(edge.toId) && ['reference','causes','happens-before','reveals','depends-on','setup/payoff','custom'].includes(edge.kind) && text(edge.label) && (edge.confirmed === undefined || typeof edge.confirmed === 'boolean'), 'Board edges require valid cards and an explicit relationship type.');
  }
  for (const issue of doc.issues) {
    requireValid(text(issue.category) && text(issue.description) && ['low','medium','high'].includes(issue.importance) && linksValid(issue.links) && ['open','investigating','intentional','resolved','dismissed','stale'].includes(issue.status) && text(issue.resolution) && Array.isArray(issue.evidence), 'Invalid continuity issue.');
    for (const evidence of issue.evidence) { requireValid(text(evidence.id) && !allIds.has(evidence.id), 'Evidence IDs must be unique.'); allIds.add(evidence.id); requireValid((evidence.link === null || linkValid(evidence.link)) && text(evidence.quote) && text(evidence.sourceRevision) && text(evidence.notes), 'Invalid evidence.'); }
  }
}
export function removeBoardCard(doc: PlanningDocument, boardId: string, cardId: string): PlanningDocument {
  return { ...doc, boards: doc.boards.map(b => b.id === boardId ? { ...b, cards: b.cards.filter(c => c.id !== cardId), edges: b.edges.filter(e => e.fromId !== cardId && e.toId !== cardId) } : b) };
}
export function approveFact(doc: PlanningDocument, id: string): PlanningDocument { return { ...doc, facts: doc.facts.map(f => f.id === id ? { ...f, status: 'fact', confirmed: true } : f) }; }
export function moveField(doc: PlanningDocument, id: string, delta: number): PlanningDocument {
  const fields = [...doc.fields]; const index = fields.findIndex(f => f.id === id); if (index < 0) return doc;
  const sameKind = fields.map((f,i) => ({f,i})).filter(e => e.f.recordKind === fields[index].recordKind); const target = sameKind[sameKind.findIndex(e => e.i === index) + delta];
  if (!target) return doc; [fields[index],fields[target.i]] = [fields[target.i],fields[index]]; return { ...doc, fields };
}
export interface ChronologyFinding { code: 'ordering-cycle' | 'time-conflict' | 'relative-cycle' | 'unresolved-link'; message: string; eventIds: string[]; constraintIds: string[] }
export function eventBounds(doc: PlanningDocument, id: string, visiting = new Set<string>()): [number, number] | null {
  if (visiting.has(id)) return null; const event = doc.events.find(e => e.id === id); if (!event) return null;
  const time = event.time; if (time.kind === 'fixed') return [time.day,time.day]; if (time.kind === 'interval' || time.kind === 'bounds') return [time.start,time.end];
  if (time.kind === 'relative') { const bounds = eventBounds(doc,time.eventId,new Set([...visiting,id])); return bounds ? [bounds[0]+time.offsetDays,bounds[1]+time.offsetDays] : null; }
  return null;
}
export function checkChronology(doc: PlanningDocument): ChronologyFinding[] {
  const findings: ChronologyFinding[] = []; const confirmed = doc.constraints.filter(c => c.confirmed);
  // Positive-weight cycles and strict zero-distance cycles are impossible. An informal note is never a constraint.
  const outgoing = (id: string) => confirmed.filter(c => c.beforeId === id);
  const reported = new Set<string>();
  // Each strict or positive edge must have no return path, including parallel edges.
  for(const first of confirmed.filter(c=>c.strict||c.minimumDays>0)) {
    const path:TimeConstraint[]=[];const seen=new Set<string>();
    const findPath=(id:string):boolean=>{if(id===first.beforeId)return true;if(seen.has(id))return false;seen.add(id);for(const edge of outgoing(id)){path.push(edge);if(findPath(edge.afterId))return true;path.pop();}return false;};
    if(findPath(first.afterId)){const cycle=[first,...path];const key=cycle.map(c=>c.id).sort().join(',');if(!reported.has(key)){reported.add(key);findings.push({code:'ordering-cycle',message:'Confirmed event order contains a cycle. Revise the contributing constraints.',eventIds:[first.beforeId,...cycle.map(c=>c.afterId)],constraintIds:cycle.map(c=>c.id)});}}
  }
  for (const event of doc.events) {
    const seen = new Set<string>(); let current: StoryEvent | undefined = event;
    while (current?.time.kind === 'relative') { if (seen.has(current.id)) { if (!findings.some(f => f.code === 'relative-cycle' && f.eventIds.includes(current!.id))) findings.push({ code:'relative-cycle',message:'Relative dates refer back to each other; no date can be established.',eventIds:[...seen],constraintIds:[] }); break; } seen.add(current.id); const anchorId: string = current.time.eventId; current = doc.events.find(e => e.id === anchorId); }
  }
  // Propagate only feasibility bounds through confirmed paths. This never assigns a
  // fabricated date to an unknown event, and exposes every contributing constraint.
  type Bound = { day:number; strict:boolean; eventIds:string[]; constraintIds:string[] };
  const lower = new Map<string,Bound>(); const queue:string[] = [];
  for (const event of doc.events) { const bound=eventBounds(doc,event.id); if(bound){lower.set(event.id,{day:bound[0],strict:false,eventIds:[event.id],constraintIds:[]});queue.push(event.id);} }
  let cursor=0; const budget=Math.max(1,doc.events.length * (confirmed.length + 1)); const conflictKeys=new Set<string>();
  while(cursor<queue.length && cursor<budget) {
    const id=queue[cursor++], base=lower.get(id)!;
    for(const constraint of outgoing(id)) {
      if(base.eventIds.includes(constraint.afterId))continue;
      const candidate:Bound={day:base.day+constraint.minimumDays,strict:base.strict||constraint.strict,eventIds:[...base.eventIds,constraint.afterId],constraintIds:[...base.constraintIds,constraint.id]};
      const upper=eventBounds(doc,constraint.afterId);
      if(upper && (candidate.day>upper[1] || (candidate.day===upper[1] && candidate.strict))) {
        const key=candidate.constraintIds.join(','); if(!conflictKeys.has(key)){conflictKeys.add(key);findings.push({code:'time-conflict',message:'Known story time cannot satisfy the contributing confirmed ordering constraints.',eventIds:candidate.eventIds,constraintIds:candidate.constraintIds});}
      }
      const previous=lower.get(constraint.afterId);
      if(!previous || candidate.day>previous.day || (candidate.day===previous.day&&candidate.strict&&!previous.strict)){lower.set(constraint.afterId,candidate);queue.push(constraint.afterId);}
    }
  }
  return findings;
}
export function previewEventTime(doc: PlanningDocument, eventId: string, time: StoryTime) {
  const next = { ...doc, events: doc.events.map(e => e.id === eventId ? { ...e,time } : e) }; validatePlanningDocument(next);
  const affected = doc.events.filter(e => e.id !== eventId && JSON.stringify(eventBounds(doc,e.id)) !== JSON.stringify(eventBounds(next,e.id))).map(e => e.id);
  return { document:next, affectedEventIds:affected, findings:checkChronology(next) };
}
export function linkLabel(doc: PlanningDocument, scenes: {id:string;title:string}[], link: RecordLink): string {
  return (link.kind === 'record' ? doc.records.find(r => r.id === link.id)?.name : link.kind === 'event' ? doc.events.find(e => e.id === link.id)?.title : link.kind === 'plot-point' ? doc.plotPoints.find(p=>p.id===link.id)?.title : scenes.find(s => s.id === link.id)?.title) ?? `Missing ${link.kind} (${link.id})`;
}

/** Author action freezes the referenced value; later canon edits retain, but stale, this claim. */
export function authorKnowledge(doc:PlanningDocument,entry:Omit<CharacterKnowledge,'factSnapshot'>):PlanningDocument {
  const fact=doc.facts.find(item=>item.id===entry.factId);
  requireValid(fact,'Choose a canonical fact.');
  requireValid(entry.stance!=='knows'||fact.confirmed&&fact.status==='fact','Knowing requires an approved fact.');
  const next={...doc,knowledge:[...(doc.knowledge??[]).filter(item=>item.id!==entry.id),{...entry,factSnapshot:{subjectId:fact.subjectId,property:fact.property,value:fact.value}}]};
  validatePlanningDocument(next);return next;
}
