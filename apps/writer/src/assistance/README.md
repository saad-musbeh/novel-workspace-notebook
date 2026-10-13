# Desktop assistance integration

`AssistanceWorkspace` is a React panel using only supplied callbacks. No HTTP requests, downloads, native commands, model management, or dependency installation occur here. Import `assistance/index.ts` for public types and pure functions. CSS is scoped to `.assistance`.

## Integration contract

Supply the active **committed** `SceneSnapshot` with scene identity, immutable revision identity, SHA-256 hash of the UTF-8 `text` projection, and optional title/language. An unsaved editor buffer must not masquerade as a committed scene. `sources` contains other **current** saved scene/context snapshots; any missing or changed supporting source makes a result stale. A planning snapshot may be included as a separately identified JSON text source with its own immutable revision/hash, so fact changes invalidate dependent reviews. Preserve source IDs across re-renders.

`onApplyPatch` receives a `ScenePatch`: exact source revision/hash, UTF-16 anchor start/end, quote, prefix/suffix, and replacement. It must check all preconditions **atomically** against current storage, preserve document formatting, create a normal manuscript revision, and return `{revisionId,hash}`. The frontend helper is a preview, not the storage transaction. Refresh the scene after commit. Do not flatten the rich document just to apply a patch. The callbacks may reject unresolved conflicts, active IME composition, or pending unsaved changes.

`onApproveFact` is a distinct explicit approval callback. It receives the author-confirmed subject/property/value plus source references and exact evidence. Record it through the story-bible revision flow with the suggestion ID as an idempotency key; never silently turn extracted proposals into canon. No model output can invoke it. Ordinary review questions are only acknowledged/rejected.

`onPersistSuggestions`, `onPersistPacks`, and `onPersistExceptions` store project arrays in normal recoverable project revisions. Without these callbacks the panel labels imports/glossary/exception additions as session-only. Jobs from callbacks are native durable records; their receipt is shown only after `onSubmitJob` resolves. Root should supply persisted suggestions again after loading, including decisions for suggestions originally supplied in job results.

`onSubmitJob` receives `ReviewRequest`. Native storage must freeze and validate snapshots, durably save before acknowledgement, limit one active provider attempt, enforce context/output/time limits, and never substitute newer revisions. It returns `ReviewJob`. `onRefreshJobs` updates the controlled `jobs` prop. `onCancelJob(id)` requests durable cancellation; it must update props when the native terminal state is known. `onRetryJob(id)` uses identical payload/idempotency key and an incremented attempt; editing scope creates a new request. Recovery marks running attempts interrupted. The UI has no provider access beyond these callbacks.

`ProviderStatus` must reflect an actually verified PC-local LanguageTool or LLM provider. `ready` also requires explicit installed language support. Do not report a model as working merely because a runtime binary exists. Expose model license/hash/runtime identity when available. There is no cloud fallback and no automatic acquisition. LanguageTool may suppress exact project exceptions with `filterProofreadingExceptions`; do not hide unrelated issues in the same sentence.

All suggestions must pass `validateSuggestion(suggestion, immutableJobScope)` before becoming native results. Structured LLM arrays can use `parseModelSuggestions`. Replacement suggestions require exactly one matching quote/context and source anchor. Evidence-free results can only be review questions; facts require evidence and a separate author confirmation. JSON strings render as React text, never HTML.

`planning/toContinuityWorld` maps confirmed planning records into the explicit normalized `ContinuityWorld`; registry assumptions are displayed alongside rule toggles. Unknown bounds, beliefs, missing learning notes, ordinary locations without explicit exclusivity, and optional/payoff-open setups are not fabricated contradictions.

## Language-pack import format

A pack is bounded JSON (`LanguagePack`, schemaVersion 1), at most 2 MB and 10,000 entries. It records identity, name, language/variant, immutable version, source, license, attribution, supported operations, entries grouped by sense and part of speech, and checksum. Synonyms, related words, and translations have distinct fields. The imported license is a declaration supplied by the pack author, not a legal approval by the app.

`checksum` is lowercase SHA-256 of the UTF-8 canonical JSON of the **entire pack except checksum**. Canonical JSON sorts object keys lexically, preserves array order, and uses JSON encoding for strings/primitives. Use `canonicalJson` / `packChecksum` when preparing packs. Reject unknown schema/operations, corrupted content, duplicate terms/senses, missing license/source/version, and republishing different content under the same identity/version. User-created glossary entries are generated with their own checksum and can be exported/reimported in the panel. Original text is preserved; optional case folding affects lookup only.

## Observed verification

Targeted Vitest suite: 84 passing tests on October 3, 2026. Full TypeScript type check also passed. Each of six rules has a positive and negative fixture. Tests cover exact multilingual evidence, stale supporting revisions, hallucinated/model-output rejection, payload hash verification, pack metadata integrity, scoped exceptions, invalid job transitions, cancellation, interruption and bounded retries. Native callbacks and actual provider language quality must be verified separately on the host; these unit tests do not claim local model inference, runtime installation, or actual IME coverage.

## Native wrapper and archive payloads

`NativeAssistanceWorkspace` takes projectId, current committed scene, current sources, normalized continuity, optional selectedPassage, native manuscript/fact callbacks, and `onRegisterFlush(flush | null)`. It registers a drain of pending local storage writes only; close does not wait for model inference. CAS failures surface to drain and have an explicit retry control. Unmount unregisters the drain. The wrapper uses the root `workspaceCommand` bridge and does not talk to HTTP services directly.

Project data kinds:

- `languagePacks`: `{schemaVersion:1,packs:LanguagePack[]}`; max 20 packs, aggregate 8 MB, each verified with `importLanguagePack`.
- `settings`: preserve unrelated keys and store `assistance:{schemaVersion:1,providers:ProviderConfiguration[2],exceptions:DictionaryException[],suggestions:Suggestion[]}`. Endpoints must match literal `http://127.0.0.1[:port]` or `http://[::1][:port]` with no path/query/credentials. Both provider kinds occur exactly once. Max 10,000 exceptions and 10,000 uniquely identified decisions; structural historical anchor validation, exact freshness before application. No tokens or credentials belong here.
- `assistanceJobs`: `{schemaVersion:1,jobs:ReviewJob[]}`; max 1,000 retained immutable jobs and aggregate 8 MB, at most one unfinished attempt, globally unique job IDs/idempotency keys. Source text hashes and saved results are revalidated on opening. Older durable records are retained; the UI displays latest 50.

Opening a project validates these payloads and stops the assistance panel on corruption without replacing the saved records. Unfinished attempts reopen as interrupted; no status check, provider launch, or retry happens automatically. Provider config is saved and checked only on explicit action.

Native operations are `provider_status({kind,endpoint,model?})`, `provider_run({request,endpoint,model?,runToken})`, and `provider_cancel({jobId,runToken})`. A run token is `request.id + ':' + attempt`, so cancellation tombstones reject pre-start/late output from the old attempt and an explicit retry gets a new token. Native execution must preserve those tokens through cancellation races. The wrapper additionally ignores terminal/previous-attempt responses. Suggestion identities use a SHA-256 namespace derived from job ID plus original suggestion ID to avoid collisions across model runs.

Submission replays reuse the same durable record only when canonical immutable request bodies match. A changed scope/options/body under the same idempotency key is rejected. Retry preserves the exact request and advances attempt up to 3. Result validation checks origin, pending proposal status, exact evidence/sources, cardinality and distinct IDs before persistence. Exceptions suppress only exact quoted proofreading terms in the requested language.


Canonical fact context is optional and author-selected. `canonicalFactSource(projectId, savedPlanningRevisionId, doc)` produces a `kind:'facts'` source named `planning:<projectId>`. `canonicalFactText` projects only confirmed `status:'fact'` records plus their subject IDs/names, sorted by stable ASCII identity, with fixed ASCII JSON keys. It excludes beliefs, rumors, proposals and private notes. Context is capped at 1,000 facts and 30 KB; oversized/unversioned context is unavailable, never silently truncated. Rust `providers::canonical_fact_text` implements the identical byte projection; both sides share a multilingual SHA-256 fixture. Native preflight must load that exact persisted planning revision and verify both bytes and hash. The LLM prompt treats it as supporting author-confirmed data. It cannot be replaced as manuscript, and newly approved fact proposals must include manuscript evidence. All result suggestions retain every source dependency, including the fact snapshot.

A `ScenePatch.supportingSources` list preserves all reviewed dependencies for the native acceptance boundary. Verify current revision/hash of these sources **after draining pending scene/planning writes and before committing**. Include `planning:<projectId>` in the continuity world's sources so changes to fact/constraint metadata invalidate saved deterministic findings as well. Metadata context references are not scene links.

Desktop pack, planning-recovery and writing-system exports invoke `export_document` with an explicit format/content/suggested filename. The system chooser selects the destination; frontend code does not provide an output path. Browser previews retain Blob downloads, and tests cover both branches and native cancellation/failure.

Selected-passage jobs validate proposed replacements and extracted-fact manuscript evidence against the selected UTF-16 range. Supporting context can explain a question but cannot authorize an edit elsewhere in the scene. Durable result reopening repeats this validation.
