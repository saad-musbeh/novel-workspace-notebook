# Saved manuscript statistics — October 14, 2026

Expose saved-text units separately; whitespace tokens and CJK scalars are descriptive measures, never comparable language-aware word counts. Statistics must use the same coherent checked manuscript read as alphabet export and refuse ambiguous metadata/content rather than choose a projection. Read-only refresh keeps author drafts and manual saving independent.

---

# Typed custom field authoring — October 14, 2026

Field definitions and values remain revisioned planning data. Dedicated controls enforce the declared type at the authoring boundary, show unavailable historical choices without reinterpretation and use canonical identities for links. Read one planning/manuscript snapshot before introducing a scene link, then require its saved head during Save. Hide/reorder never deletes values; incompatible definition replacement/removal requires explicit prior value removal.

---

# Native board view and authored grouping — October 14, 2026

Keep zoom, scroll and pan local to the current board page. Convert pointer positions through the exact displayed scale and scroll state before proposing an authored coordinate change; reject stale view/data captures. Named group titles and member labels are author-owned planning metadata, saved through existing registered draft/CAS controls. They never create or replace canonical story links.

---

# Ready-to-write novel creation — October 10, 2026

**ADR-053 — Implemented novel startup:** New novel must produce a usable writing page. Add a create_novel storage/RPC operation that commits the project, first chapter, blank scene, placement and revision/order records together. Reuse the existing atomic universe manuscript initializer while preserving create_project semantics and universe wire responses. There is no schema migration or new dependency. Generate the additive wire contract from Rust.

Check all drafts owned by the current window before creation. Cancel or a failed save creates nothing; block further edits while creation is pending. On success select Write, clear the old outline/search and open the new novel in the same window. Include New novel on the empty welcome page and in the Library. Destroy the temporary naming dialog after its response so it cannot remain open. Remove the sidebar mouse/tab hint requested by the author. Preserve the current full document when an editor preview reply updates layout without repeating text; otherwise a resize or scroll can invalidate save-state and tool captures.

# Chapter writing entry — October 10, 2026

**ADR-052 — Implemented chapter opening:** a chapter is a visible writing entry, not only a scene group. Activate an existing chapter to open its first scene. For an empty chapter, show its title and an explicit Start writing action; create the blank first scene only after that action. Bind creation to the captured project and chapter rather than a later outline selection. Preserve manual-save navigation and the existing chapter/scene storage model. Keep empty title matches in outline search.

Refresh metadata before starting so a stale empty view can open a scene already created elsewhere. Disable repeated Start actions while pending. Ignore delayed metadata or editor navigation after another page, project or window takes over; closed windows also ignore catalogue/index replies. Middle clicks and detached empty-page actions follow the same additional-window model. There is no database migration or dependency change.

# Notebook surface palette — October 10, 2026

**ADR-051 — Implemented theme repair:** the notebook deliberately uses light pages and dark text, so its application CSS must own the backgrounds of popup menus, nested GTK text nodes and notebook stacks as well as root widgets. Use explicit white/pale surfaces and readable selection colors rather than allowing dark-theme backgrounds to mix with notebook text. Verify computed GTK contrast under both dark and light themes without changing desktop configuration.

# GTK callback ownership — October 10, 2026

**ADR-050 — Implemented crash repair:** clone the current editor out of its RefCell before a GTK focus operation. GTK focus callbacks can run synchronously and update active-editor ownership, so an if-let temporary borrow must not span `grab_focus`. Reproduce that callback in the no-focus GTK workflow, using synthetic stores and preserving dirty text/session identity. Deliver the repair through the existing separate Notebook shortcut with a recoverable previous executable; retain the older app and personal storage.

# Rust desktop decision — October 10, 2026

**ADR-048 — Implemented native port:** the user requested Rust for both application and interface. Use the existing installed GTK3/Pango/Cairo/Fontconfig stack through pinned Rust bindings. Move the former Python text model/layout/checkpoint/chooser implementation into `crates/editor`; build the desktop widgets, worker orchestration and dialogs in `apps/native-writer`. Keep the existing revision/CAS/archive/sync/DOCX Rust core and database location. Use cached dependencies; no installation or model download.

The default launcher/build/package now uses this native app. Retain React/Tauri and Python sources for comparison and legacy convenience workflows rather than deleting prior work. The Tauri shell also calls the Rust helper. Package distinct Rust executable names, preserve the old installed selection, and report native coverage gaps explicitly. RUST_PORT.md is the current implementation map; earlier ADRs remain historical where their chosen stack is superseded.

---

# Architecture Decision Register

Original register October 2; current desktop implementation notes October 5, 2026. “Accepted requirement” means selected by the user. “Proposed” means the implementation choice still needs testing.

ADR-039 — October 5, 2026, accepted requirement: replace the LibreOffice editing backend with an application-owned native editor. The new model retains exact Unicode/document JSON, session undo and per-tab ownership; installed Pango/Cairo provide shaping and viewport rendering. Existing SQLite revision/conflict/recovery semantics remain. Scrivener and Scribeist are workflow references, with split drafts as the selected priority; no proprietary backend code is used. Earlier LibreOffice test and latency results are historical and do not qualify the replacement. Implementation and native packaged acceptance are in progress. The author's separate LibreOffice installation is outside this removal.

ADR-040 — October 5, 2026, accepted requirement: add configurable Word-style Alt+X hexadecimal/character conversion across valid Unicode scalars, including private-use characters. Preserve code points without normalization; reject invalid conversions without mutation. Missing glyphs need an appropriate font, and are not grounds to replace stored characters. Existing shortcut remaps survive additive actions.

| ID | Status | Decision | Rationale / consequence |
|---|---|---|---|
| ADR-001 | Accepted requirement | One user, Fedora KDE + Android | No collaboration, subscriptions, or hosted account system. |
| ADR-002 | Accepted requirement | Both devices store editable projects locally | Writing never requires the PC or internet. |
| ADR-003 | Accepted requirement | One chronology, main plot and subplots | Plot lanes are not alternative universes. |
| ADR-004 | Accepted requirement | English, Arabic, Chinese, invented alphabets | Actual-device editor tests precede the full UI. |
| ADR-005 | Accepted requirement | PC-only LLM with Android remote requests | No Android model and no cloud inference. |
| ADR-006 | Accepted requirement | Tailscale for away-from-home connection | Networking metadata exists; no cloud manuscript storage is intended. |
| ADR-007 | Superseded desktop proposal | Originally Tauri/React/Tiptap/React Flow; current application-owned editor/custom board | See ADR-039/031; physical input remains an acceptance gate. |
| ADR-008 | Implemented desktop | Shared Rust domain/storage core | Reuse invariants on desktop and Android; avoid raw SQL from the editor UI. |
| ADR-009 | Implemented desktop | Companion owns PC databases | UI and remote jobs share one persistence/migration owner. |
| ADR-010 | Implemented desktop | Immutable per-entity revisions with parent IDs | Detect concurrency without trusting device clocks. |
| ADR-011 | Implemented desktop | Manual same-scene conflict resolution initially | Preserve writing instead of hiding risky merge behavior. |
| ADR-012 | Implemented desktop | Separate event, scene, plot point, board card | Support many-to-many links without copying canonical content. |
| ADR-013 | Implemented desktop | Confirmed facts distinct from beliefs and proposals | Reduce false continuity findings and AI contamination of canon. |
| ADR-014 | Implemented desktop | Selected-scope, revision-bound AI jobs | Evidence and freshness are inspectable. |
| ADR-015 | Implemented desktop | No active-database file synchronization | Exchange application records and verified assets. |
| ADR-016 | Proposed | Foreground refresh is the Android delivery guarantee | Background execution can be deferred by the OS. |
| ADR-017 | Proposed | Free components and optional local packs only | No trial/Pro/cloud dependency hidden in the core workflow. |
| ADR-018 | Implemented desktop | Original text preserved; normalization derived | Search convenience must not silently rewrite Arabic/custom text. |
| ADR-019 | Implemented desktop | No automatic tombstone/history compaction in v1 | Simplify safe recovery and long-offline device handling. |
| ADR-020 | Implemented desktop | Local Git plus portable project archives | Source control and manuscript backups serve different purposes. |

## Decisions still requiring measurement, not another product questionnaire

Exact model, quantization, graphics backend, context budget, and language quality; Fedora edition and compatible tool versions; the actual invented-alphabet mapping/direction; Android IME behavior; licensed dictionary-pack selection; practical performance budgets; and installation packaging details.

Record a new decision entry when changing one of these choices. Include evidence, tradeoffs, migration impact, and affected tests. Do not silently replace an accepted requirement with a convenient library limitation.

## October 2 implementation notes

ADR-021 — M0-only browser persistence adapter. The first interactive preview uses IndexedDB to exercise transactions and editor behavior before native prerequisites are available. It is visibly identified in the interface and never silently used as a fallback if Tauri storage fails. This does not replace the planned Rust/SQLite architecture.

ADR-022 — Historical M0 SQLite owner. The initial native spike embedded SQLx in the shell. It was compiled/tested, then superseded by the single-owner stdio companion and recoverable schema 3 migration (ADR-029). Retained prototype tables preserve old data/history; do not use the old shell-owned architecture for new writes.

ADR-023 — October 3, 2026: use installed Vazirmatn for Arabic in the Fedora spike. The user reported incorrect native Arabic rendering and compared four installed fonts in the same WebView; only Vazirmatn looked correct. Remove synthetic character tracking and optional-ligature suppression from editable mixed-script text. This is a presentation change, with no manuscript conversion or migration. Android font availability and input behavior remain unverified; a licensed bundled-font strategy may be needed before Android release.


ADR-024 — October 3, 2026: experimentally embed the already installed LibreOfficeKit on Fedora, following the user's request to use LibreOffice's Arabic behavior. Synthetic WebKit rendering reproduces the kasra-triggered baseline shift; actual LibreOfficeKit retains the baseline and exact text. Keep Vazirmatn. Retain the standard editor and SQLite revision model; no migration or silent text sentinel. Refuse initial scene conversions that cannot round-trip exactly. This uses the experimental tiled editing API, not a claim of a finished portable editor. Android, runtime packaging, performance, accessibility and native IME behavior still require separate validation. Private per-session FODT/profile files are retained in app cache; do not treat them as verified project backups.


ADR-025 — October 3, 2026: retain the original main writing-page appearance while using the compatible LibreOfficeKit engine underneath. The user explicitly rejected the separate LibreOffice presentation. Native desktop defaults to compatible rendering, with standard rendering available. Presentation styles and cropped layout do not change the scene model or insert manuscript characters. Native input, accessibility, resize reflow and performance validation remain open; this is not a claim of a WebKit core fix.


ADR-026 — October 3, 2026: separate interactive LibreOffice previews from confirmed scene checkpoints. Full per-key FODT export and hidden Tiptap synchronization caused avoidable delay in the actual app. Merge only consecutive waiting text (preserving every code point); commands, navigation, pointer boundaries and snapshots keep their order. Keep the scene dirty until its epoch is checkpointed, retain unconfirmed inserted text on failure, and flush before export. Checkpoint after 300 ms idle or a two-second scheduling deadline. Deadline is not a durability guarantee: queued operations, helper failure and app closure can delay confirmation. This does not add a complete crash-recovery implementation. Native synthetic tests measure engine processing separately from preview output and bridge wall time; do not describe a sub-3 ms engine sample as guaranteed total key processing. No migration or stored-text normalization.


ADR-027 — October 3, 2026: guard native close with an explicit editor/storage drain. Browser beforeunload is not the persistence acknowledgment. Wait for in-flight and newer saves; freeze fresh input during the close drain; refuse to close on active composition, renderer checkpoint failure or storage failure. Keep pending edits and resume the same window after failure. Install the local close-event listener before enabling the backend guard. Debug lifecycle probes use separate synthetic databases and close their own windows. No schema migration or claim of recovery for input that never reached a checkpoint.

## Integrated desktop decisions — October 3, 2026

ADR-028 — User-authorized desktop scope. Complete the desktop implementation; exclude Android setup, UI, phone jobs and APK delivery. Original cross-device requirements remain a future specification. Desktop synthetic evidence does not mark original physical-device milestones complete.

ADR-029 — Single database owner and immutable graph. Tauri launches a Rust companion over bounded inherited stdio. An exclusive process lock owns migrations/storage; schema 3 retains prototype data/history and consistent pre-upgrade SQLite snapshots. Scene/project-data CAS, immutable hash/parent/origin/change envelopes, tombstones and explicit multiparent conflicts replace the spike-only revision format. Rename/reorder metadata snapshots carry complete title/order, and unresolved metadata writes reject. Runtime IPC version checking fails visibly on mismatch.

ADR-030 — Superseded by ADR-039. Installed LibreOfficeKit was the October 3 desktop writer. Remove Tiptap from runtime dependencies and keep browser drafts read-only. Paragraph language/direction/style operations preserve rich runs and use the engine's ordered undo history. Private project fonts register inside the helper process only after permission, path, format, family and checksum checks; no system font installation. Actual author-font/IME/accessibility evidence remains open. The measured approximately 2 ms engine path is not a strict 3 ms total-processing guarantee.

ADR-031 — Custom board with canonical planning document. React/SVG/list rendering keeps story records, plot points, chronology, facts and issues in an immutable CAS project-data stream. Visual card movement does not change manuscript order or story time; shared events are not copied between lanes. Generic conflicts retain complete versions for explicit author resolution rather than merging board/field data by arrival order.

ADR-032 — Bounded SQLite BLOB assets, explicit deviation. Current permitted assets and resumable chunks live in SQLite with size/hash/path/media checks. This simplifies atomic commit, backup and archive fidelity but differs from the planned external content-hash files and increases whole-database/archive memory and I/O. Do not claim external asset storage complete. A future migration must preserve hashes/references and transactional recovery before changing the authoritative location.

ADR-033 — Explicit private session sync. No listener on startup. Local UI-approved one-use challenges bind immutable device identity/epoch and project allowlist; tokens are high-entropy, expiring/revocable, hashed at rest in an atomic mode-0600 server file. Public DTOs omit token verifiers. Narrow bounded HTTP exposes only sync/verified assets; loopback or verified local Tailscale bind only, no shell or inference forwarding. Private Serve HTTPS remains deployment work. Keyring helpers have no plaintext fallback, but file verifiers are not keyring credentials and actual native keyring unlock/store is unverified.

ADR-034 — Source-bound author decisions. Assistance jobs freeze saved scenes and canonical approved-fact JSON. Trusted code derives exact unique evidence anchors; model offsets/identities cannot authorize a patch. Scene edits and fact approvals compare all same-project unconflicted supporting heads inside the commit transaction. Extraction is a proposal and consistency output asks questions; neither becomes canon automatically. Cancellation/timeouts do not block independent writing saves.

ADR-035 — Actual model remains experimental. The pinned Qwen3-4B production adapter accepted eight exact-evidence results without changing writing, yet all four clean controls received false-positive questions. Preserve the benchmark/hash/resource report and do not qualify reliable semantic continuity or Chinese variant fidelity. Deterministic configured rules remain independently available.

ADR-036 — Consistent recovery without silent pruning. Create a whole-database VACUUM snapshot on the first companion start of a UTC day and on explicit manual request; retain older snapshots and expose errors. No running-database raw copy or automatic in-place restore. Complete project archive import creates a separate project; whole-store restoration requires stopped ownership and epoch/re-pair protections. Same-disk snapshots do not replace a separately chosen portable backup.

ADR-037 — Package configuration is not release proof. RPM sidecar/helper staging, runtime checks and dependency declarations are implemented. Exact release artifacts require extraction/independent run, notice review, signing and actual install/update/recovery evidence. No external service, firewall change or Android delivery follows automatically from a build.

ADR-038 — Actual optional proofreading pilot. Authorized LanguageTool 6.6 is retained with component notices outside source/package inputs and starts only on explicit request. Native loopback tests plus frontend evidence validation found English/Arabic seeds, no suggestions on matching controls and unchanged writing. Simplified Chinese has only a clean-control observation; Traditional Chinese fails visibly as unsupported without fallback. Do not infer equal language coverage or broad grammar quality from this small diagnostic.

## Desktop continuation — October 4, 2026

ADR-042 — Incremental review sources and bounded board rendering. Reuse digests only for unchanged exact saved revision/text pairs; new revisions re-read text even if the document object is reused. Keep scene snapshots revision-bound across overlapping asynchronous refreshes. Board navigation reveals retained focus, cards/SVG are indexed to the viewport, and lists/connection controls page at 50 items. Full-project loading remains unchanged; no scene-storage migration.

ADR-043 — Atomic private recovery publication. Stage VACUUM snapshots in a mode0700 directory, verify integrity/references, flush a mode0600 file and publish without overwriting another destination. No automatic removal of abruptly interrupted staging or old history. Archive SQL count/byte preflight refuses definitely oversized history before bulk allocation. SQLite-capacity and killed-child tests establish the tested rollback boundaries, not power-loss immunity.

ADR-044 — Native peer progress follows durable content. Keyring credentials remain outside the WebView and project archives. Persist a cloned progress record only after validated ingestion/finalization; failed publication leaves the previous in-memory cursor. Project/remote-epoch receipts and an immutable local anchor reject peer metadata ahead of restored writing. Resumable assets use 256 KiB SQL chunks and a 4 MiB manual-pass budget; only verified finalized bytes permit page acknowledgement. Optional recovery/auth/peer setup errors remain separate from local save availability. Real keyring/private transport acceptance is separate from fake-credential loopback tests.

ADR-045 — Verify exact packaged resources independently. Ship collected pinned upstream notices with lockfile inventory. Extract the RPM outside the checkout, use temporary XDG directories and external test-only WebKit instrumentation, then run the owned app with networking disabled. No production test bypass is added. These checks establish extracted resource paths, offline editing and acknowledged reopen, not OS installation/update, physical IME or an uninstrumented frame benchmark.

ADR-046 — Approved native private HTTPS. Add Reqwest Rustls with OS platform certificate trust, exact Tailscale Serve hostname/443 validation and bounded DNS resolution. Require every answer to be a Tailscale address and pin the validated answer for the entire pass, retaining hostname/SNI verification. No proxy, redirect, certificate bypass, production trust override or startup lookup. Ephemeral CA/loopback overrides exist only in tests. Real Tailscale configuration and away-from-home transport remain separate from the passed TLS fixtures. The approved native synthetic keyring store/read/remove test passed without reading existing credentials.

ADR-047 — Preserve drafts at manual sync reconciliation. The current explicit pass pauses input; it is not background synchronization. Drain again after transport settles because a local provider may finish during the pass. Prefetch projections before remounting, retain the primary transport error after partial receipt and refresh scene-detail CAS state. A failed drain leaves pending controllers mounted. Future background sync needs staged receipt/apply; merely removing the input barrier can discard drafts or strand stale saves.


## October 6, 2026 — authored references and recorded time travel

Reader reveals and character knowledge are optional additions to planning schema1; loading/saving an older document does not manufacture those arrays. A reveal links an event/fact to a scene and explicit reader position. Knowledge freezes a fact snapshot and an author-declared scope; uncertainty, stale canon and competing chronology do not become truth automatically. Reference views derive from those explicit links, never manuscript inference.

Project time travel uses local change-marker sequence plus database epoch and exact point revision identity. Historical branch/hierarchy reconstruction does not use current mutable placement as a substitute. New scenes record their initial placement transactionally; older missing history is labelled and not backfilled from personal data. Recovery copies one saved scene or an explicitly selected whole story bible as a new revision, with exact current-head comparison and conflict rejection. It does not replace the database.

Typing batching preserves semantic Undo boundaries. Intermediate layout can be deferred only for already queued ordinary input, with final full-paragraph shaping at the next barrier. Background metadata and hashing cooperate with typing; no general3ms promise follows from aggregate burst throughput.


## October 6, 2026 — giant-paragraph performance accepted

After the benchmark was explained as one 54,000-character mixed-script paragraph with approximately 72 ms processing (roughly 8,000–10,000 English words without paragraph breaks), the author replied “I am OK with this.” That extreme-case performance is accepted and no longer blocks desktop completion. Further incremental bidi/layout optimization is optional. Measurements are unchanged; no general 3 ms guarantee or physical-input qualification is inferred. Ordinary typing responsiveness remains a design goal. Android stays deferred.

## October 6 — explicit story workflows and bounded continuous editing

The author requested all six proposed desktop additions. Whole-book and character-cut editing reuse the native Pango/Cairo renderer, avoiding a second WebView text renderer. Scene bodies load lazily; offscreen helper processes park into validated, private transient JSON checkpoints retaining bounded independent Undo/Redo. Save/IME/checkpoint failures retain mounted drafts. A timer fallback prevents hidden windows from stalling layout barriers.

Branches copy bounded portable history into a separate project with durable lineage. Bring-back selects one scene or the whole bible and creates a new revision after atomic source/branch checks; it never automatically merges order or deletes source scenes. Paragraph versions are durable project data, whereas Undo checkpoints are session cache. Strict saved-revision/document/block anchors protect swaps; immutable originals survive every alternative edit. Ripple previews stage semantic changes before autosave, and rehearsal follows explicitly authored reveal progress without inferring prose knowledge.

The author's accepted giant-paragraph processing result remains unchanged. These features do not make editing depend on a model, network or service.

## October 6 — Writing desk UX

The author approved eight additions: peek references/outline, a same-window navigation trail, whole-book scene folding, pane placement by tab/binder drag, writing width with overlay docks, saved-revision paragraph comparison, formatting sequences and independent interface mirroring. The additions passed exact-package and installed user-folder probes; physical input acceptance remains separate.

Keep transient peeks read-only and preserve editor focus. Bind draft trail caret/selection to exact text; restore book reading/filter state without claiming native caret restoration. Save before parking folded editors, retaining Undo and refusing failed drains. Pane placement reuses canonical scene/tab identity and does not change manuscript order. Compare saved paragraphs by position, page 12 results and bound excerpts to 2,000 Unicode characters; paragraph insertion can shift later comparisons.

Store eight sequences of at most 32 explicitly supported formatting commands as local interface preferences. Exclude manuscript input, clipboard contents and navigation. Replay one acknowledged/saved command at a time on the focused draft, lock normal input and stop visibly on refusal. Earlier accepted steps remain saved; no transactional rollback is implied. Interface layout direction stays independent of paragraph direction and exact Unicode text. No new dependency, service or model is required.


## October 6 — compact writing page and test review

The author's marked screenshot requested removal of the writing headers and bottom action strips, plus the blank side columns. Keep one formatting toolbar, accessible save state and a menu for occasional actions. Hide the duplicate tab strip for a single tab; multiple tabs retain their browser controls. The outline has a close button and reopens through the visible Outline control. Its visibility is saved as a local preference.

Fill pane renders at the actual available width without magnifying the bitmap. Native viewports are bounded to 4096×2160; text, caret and pointer coordinates retain the original scale. Overlay tool controls start below the writing toolbar. Exact Unicode and old parked checkpoints remain compatible.

The test review removes duplicated execution and assertions that only reproduce implementation details. Keep distinct boundary and failure-path regressions. Separate explicit GUI diagnostics from ordinary discovery; preserve the unresolved old resize anchor diagnostic. TEST_SUITE.md records the resulting workflow.

## October 6 — scene finding and everyday writing actions

The author reported that finding scenes and everyday actions made the app unusable. Put project selection, durable New chapter/New scene controls and a searchable manuscript tree before grouped tools. Find scene searches title metadata only, with a bounded query and an explicit empty state. Reveal the active scene inside the tree scroller. Creation drains pending saves before committing objects and uses the canonical scene route; failure retains the draft.

Keep Outline, trail arrows, Notes, Reference, Split, Find and Focus visible in the writing toolbar. Put paragraph settings in Format and advanced Desk/Commands/Changes actions in More. Use a compact title and neutral palette; a singleton tab strip occupies no space. Persistent notes/references dock beside text in wide windows and overlay in narrow ones; transient peeks remain overlays.

The design inference draws on Scrivener’s optional panels and document inspector, and Dabble’s separation of manuscript structure from story notes: [window panels](https://www.literatureandlatte.com/blog/tame-the-scrivener-window), [inspector](https://www.literatureandlatte.com/blog/get-to-know-the-scrivener-inspector), [planning notes](https://www.dabblewriter.com/docs/planning-story-notes), [story organization](https://www.dabblewriter.com/docs/planning-story-notes/how-dabble-organizes-your-story). These sources do not establish usability of this app. Extracted and installed workflow checks passed; human usability acceptance remains unverified. WRITER_WORKFLOW.md records the delivered controls.


## Compact tool pages — October 6

Book, Planning, Review, Project and Alphabets have no separate top bars or centered page margins. Outline and More are integrated into their existing controls. More retains Back/Forward, Desk and Commands; Review/Project retain the scene selector and its save-before-switch guard. Page actions also remain available during loading/error states. Panel and form spacing is reduced; manuscript line spacing is unchanged. Review and Alphabets follow the neutral shell palette. Alphabets uses readable captions, wrapping symbol cells and a single-column narrow form, with the outline beside rather than over that form at 760 px.
Exact package and installed-copy evidence is in PACKAGE_VALIDATION.md. Android and human/physical/external acceptance remain separate.


## Chapters and shared universes — October 6

Use Edit beside a chapter to change its name and manuscript number. Changing the number moves it and renumbers the other chapters; Cancel leaves saved metadata unchanged. Saving drains pending writing first and preserves the mounted draft. Stale metadata is refused; any partially completed legacy name/order operation is reported rather than hidden.

The + beside the novel selector opens Library. Create a universe, a series, then a novel, or add an existing unassigned novel. Universe/series groups also appear in the selector. New novel creation commits its separate project, first chapter/scene and catalogue membership together. The universe/series line opens shared World notes; Reference shows those notes beside writing alongside the novel’s own scene references. Shared records have one saved source and never silently overwrite a novel’s planning or canon. Individual project exports/sync remain separately scoped; unavailable novels remain explicit, and whole-universe export/hierarchy merging is not included.

## Chapter zero and compact leading — October 6

Display numbering starts at 0 or 1 per novel; storage positions remain nonnegative indices. The strict revisioned chapterNumbering payload is exported/synced with project data. A checked chapter edit commits name, order and numbering together; captured chapter/project heads, chapter order and numbering parent must still match. Old projects/archives default to 1. Competing numbering is resolved explicitly in Recovery.

Pango absolute leading is derived from displayed font size rather than multiplying full fallback-font ascent/descent. Paragraphs add no extra 16px gap. Actual glyph ink drives viewport culling and caret visibility, so Arabic marks remain painted outside tight logical line boxes. Text and stored spacing values are unchanged.


## October 10 — WordCraft manuscript export

Use the pinned WordCraft document/geometry/DOCX libraries for disposable saved-manuscript conversion. Keep the application-owned Unicode model, Pango/Cairo editing/rendering, immutable storage, proofing and conflict handling. Source review found WordCraft's renderer and proofing insufficient to replace the existing Arabic/multilingual paths; no upstream UI, font or dictionary assets are copied. MIT/Apache licenses and source/patch hashes are retained.

Native export asks every owning project window to save, rejects unavailable/busy/transferring tabs, and reads a single transaction snapshot after acknowledgements. Count scene/tombstone DAG leaves in that transaction: more than one leaf refuses export, including deleted projections. A single resolved tombstone remains exportable even when the text-head projection is stale. Chapter-numbering conflicts also refuse conversion. Further typing belongs to later saves; the barrier is not a frozen click-time copy.

Convert supported exact Unicode text, marks, language, direction and spacing into editable OOXML styles. Literal U+FFFC uses WordCraft's opaque-text fallback with a global bounded object budget, including titles. Reject unsupported source shape/metadata and XML-inexpressible controls. Deliver binary DOCX through the existing native chooser and atomic private-file writer. Keep planning/history/fonts in complete project archives. This first slice does not add DOCX import, PDF export or style editing.

Independent XML, LibreOffice roundtrip and native UI tests use synthetic stores only. Release probes substitute only the chooser in a disposable extraction, with original hash and unchanged-payload verification; no release test bypass is shipped. Stock chooser interaction, Microsoft Word and physical keyboard/IME remain separately unqualified. See DOCX_EXPORT.md and TEST_RESULTS.md.

## October 10 — Saved Word export profiles

Store a strict version-1 exportProfile as immutable per-project data, using the existing history, compare-and-swap, archive, private sync and explicit Recovery paths. No database migration or dependency is added. Default profiles are read-only fallbacks only for novels without profile history. Reject unknown fields, unsupported paper/spacing, invalid font names/sizes, corrupt checksums, mismatched projections and competing leaves. Keep ordinary project opening available for Recovery.

Bind the UI-reviewed profile head to native export. After all project-window save acknowledgements, read profile head/leaves/payload and manuscript in the same SQLite transaction. Refuse a changed profile, preserving local pending edits after failed CAS. Clean conflict drains must permit Recovery actions, while native export refuses conflicted profiles. Give the style panel and sibling Recovery distinct React keys; a real desktop probe exposed duplicated DOM controls from the shared key.

Apply export-only Letter/A4 paper, equal margins and four named Word style definitions. Body spacing overrides only source paragraphs; headings retain source/generated spacing. Preserve text, marks, direction and language. Fonts remain references, not bundled assets or verified installed faces. Keep the app-owned editor and its stored formatting unchanged.


## Optional editable translations (October 10, 2026)

Use one original manuscript with optional scene translation drafts. Keep translation text in revisioned sceneTranslations project data, not as additional mandatory manuscript scenes. Source language, target language, original text and scene/revision IDs are immutable per draft; author text is editable through CAS. Re-translation appends a new draft and retains previous wording. Compare current original plain text exactly to the frozen source to flag staleness, without automatic re-generation.

Automatic translation is a separately cancellable local model request with immutable source verification, bounded input/output, timeout and the existing inference semaphore. It creates an editable draft, never a manuscript patch. Close/navigation drains author edits but cancels inference independently, discarding late output. Manual translation remains available without any service. Archive/branch remapping and sync validate exact source ownership/text; incremental sync checks only newly accepted translation documents after ancestors are inserted.


## October 10 — Minimal notebook and manual navigation (ADR-049)

The author chose white notebook pages, very few visible controls, manual saving and an area selector above the left collapsible menus. Replace the Rust desktop's horizontal toolbars, static page tabs, nested scene tabs and split editor with one top tab row and left menus. Ordinary page navigation reuses the active tab after Save / Discard / Cancel. Middle click opens another window; dragging out transfers the actual page and editor state. All windows share one companion, own independent view drafts, and can close separately. Cross-window tab merging is outside this requested slice.

Remove periodic autosave. Wait for queued text before deciding whether a page is dirty, refuse leaving during composition, keep cancelled or failed saves mounted, and reset the confirmed document/Undo only after explicit Discard. Project-dependent saved operations check all windows for that project. Tool views are ordinary page content; only decisions/file selection use dialogs. Route actions in moved tool pages to their current window. A separate conflict preview can explicitly save the retained local draft against captured heads without changing its original window prematurely.

Use a value-based GTK create-window signal callback: the autogenerated unchecked borrowed-pointer callback caused a lifetime failure during native transfer/cleanup testing. Retain the same scene helper on a move, and use separate helpers plus existing CAS on independent opens. Load saved form data before first edits; never allow a late load to overwrite a changed form. No schema migration or dependency is needed. NOTEBOOK_DESIGN.md records behavior and verification boundaries.


## October 13 — Four optional notebook aids and public testing (ADR-054)

Keep the editor quiet; expose cutting-room floor and return bookmarks under Write → Keep & return, and a saved reader plus Passages to revisit under Review. Navigation keeps the existing manual-save decisions and same-window behavior. Tool pages retain the current window owner after detachment.

Use revisioned `notebookTools` project data for explicitly kept scraps, reading reactions, scene notes and per-scene return bookmarks. Explicit named actions save these records; typing a manuscript does not automatically save it. Track pending record writes so closing/project changes wait. Validate scene ownership, immutable reaction source/quote/scalar offset, field limits and unique record IDs; refuse stale heads. Archive/branch import remaps references and sync validates ancestors atomically. No migration or dependency is needed. Bookmarks keep exact document hashes and scalar coordinates; do not guess at changed text. A failed save retains reminder text in the visible error.

Recover automatic scraps by comparing saved scene revisions with their actual parents. Use a bounded Unicode diff, retaining the changed old span conservatively for very large rewrites. Preserve all source history and explicit scraps when inserting plain-text copies into drafts. Undo handles unsaved deletions. The attention list counts only saved edits removing/replacing existing text, excluding append-only writing, formatting, renames and restores; it makes no claim to judge prose quality or time spent writing.

Publish original source under Apache-2.0 with upstream license/NOTICE files retained. Keep private writing, runtime stores, work artifacts, credentials and model/font files out of the public Git snapshot. Publish tested Linux x86_64 archives/RPMs as a GitHub prerelease. Android is outside this desktop release.


## October 13 — Revision-checked native chapter editing

Host chapter title and number edits in a normal notebook page. Reuse the manual draft lifecycle, composition guards and tab ownership transfer so navigation, closing, project switching and detached saves retain the same author decisions. Route chapter rename through this page; do not use the uncaptured rename API from the native interface.

Read the chapter title, order, numbering and expected metadata heads in one SQLite transaction through an additive `chapter_edit_snapshot` RPC. Independent reads could pair an old displayed value with a newer head. Reuse the existing atomic `edit_chapter` mutation and refuse all stale captured boundaries. Keep entered values on failure; loading newer values requires discarding and reopening. Chapter edits change metadata/order only, preserving scene IDs, text and scene revisions. No schema migration or dependency is needed.

Qualify the active GTK runtime with native workspace tests excluding the retained Tauri UI member. That member additionally needs its frontend build and remains a separate reference check. Source qualification does not qualify the published 0.4.0 packages.


## October 13 — Native immutable paragraph alternatives

Keep rich originals and versions in existing revisioned paragraphAlternatives data. Author text changes append immutable versions with scalar-safe prefix/suffix run preservation, unchanged attributes and local inherited marks. Native Apply creates a normal unsaved scene edit and one Undo step; it never changes the manual manuscript-save policy. Keep exact anchor/hash and saved-revision checks, require explicit rebinding after changed writing, and include saved scene dependencies in capture/rebind/apply metadata commits. Retain failed metadata candidates and any already-applied scene draft with clear status; Discard metadata never silently discards scene writing.

Integrate alternative forms with the shared manual draft and detached ownership lifecycle. Reject checked first saves if project-data revision roots already exist without a current projection, preserving every concurrent root for explicit resolution. No migration or dependency is needed.


## October 13 — Explicit project-data conflict choices

Expose checked retained full-version previews and a whole-version author decision against every captured head. Resolve only after all windows for the project complete their manual draft decisions. Route detached actions to the owning window and preserve all revision history. Keep backend immutable retention checks; refuse choices that require a combined candidate rather than silently manufacturing a merge. Branch lineage remains inspection-only.


## October 13 — Checked arrangement, explicit deferred-order recovery and native author validation

Bind scene moves/reorders to one read transaction including content head, placements, chapter orders and metadata boundaries. Apply replicated projections in recorded sequence; preserve unresolved placement histories and defer dependent chapter orders. Recovery binds each decision to a complete snapshot. A retained single-head order can be explicitly replayed or replaced by the exact reviewed current-membership projection, never arbitrary unseen edits. Resolving placement never silently reapplies chapter ordering.

Move retained knowledge/reveal authoring checks into storage without making historical compatibility depend on current canon. Live author transitions check new/modified claims; imports/read/recovery preserve unchanged historical snapshots. Combined resolution may retain claims only from checksum-verified acknowledged heads, and newly authored claims obey current rules. Strict native shortcut/view settings are optional additive fields. No migration or dependency download is needed.


## October 13 — Native authored views and explicit recovery controls

Use one body-free transaction for manuscript order, planning and scene details. Refuse competing retained data, manuscript metadata/content/deletion and deferred chapter orders rather than choosing a canon/disclosure basis. A retained order checks its listed members' relative current order; new unlisted scenes do not make normal creation look conflicted. References and cuts use only explicit saved links and frozen knowledge, while rehearsal renders only reached reveals/payoffs. Optional dictionary lookup checks schema, declared provenance/license and canonical checksum before rendering separate senses.

Expose explicit retained metadata decisions and a review-confirmed immutable union candidate with a chosen mutable base. Preserve every original/version and reject incompatible immutable identities. Validate board-card identity by schema location; only nested references are scene identities during ownership checks and archive remapping.

Capture window navigation from the acknowledged rich document/frame after manual draft decisions, including exact scalar selection/top and saved head. Refuse changed sources before showing a target. Bound the session trail to 256 entries. Allocate the editor surface through one GTK allocation override and refresh it after opening, so actual pixels and worker geometry agree. Interface mirroring changes chrome only. Saved keyboard/view preferences use additive settings with strict validation and expected-parent saves. Ripple commits only an explicitly reviewed frozen candidate, preserving stale historical knowledge rather than rewriting it. Private peer controls remain explicit session actions and retain backend transport/authentication/project authorization policy.


## October 13 — Dedicated authored knowledge and reader order

Use explicit frozen fact capture and author decisions rather than arbitrary snapshot fields. Keep character interpretation, approved knows and scene/event scopes distinct. Reader reveal positions are declared disclosure order, independent of occurrence dates. Capture planning, manuscript order and details atomically; reject newly referenced unavailable scenes before submitting and verify captured live revision dependencies at Save. Preserve unchanged historical rows. Both forms participate in normal manual draft and detached ownership handling.


## October 13 — Temporary frozen peeks and optional preference conflicts

Freeze the acknowledged editor frame at hold time, label unsaved writing and compare only to an explicitly selected immutable saved revision. Do not reintroduce superseded autosave for comparisons. Cancel by token and current widget owner on release, focus/composition/navigation/transfer; late replies cannot reopen a hold. Keep references saved and authored. Read custom view/shortcut preferences unambiguously; use visible built-in runtime defaults on refusal and keep conflicting author forms disabled. Preserve the fallback notice across ordinary status updates.


## October 13 — Complete scene/deletion author recovery

Capture projected rich writing, actual scene/tombstone leaves and placement in one transaction. A deleted projection may retain an ancestor scene for previews; it is not an extra leaf parent. Require the original complete capture and explicit author review for Keep writing/Keep deleted; preserve all immutable history and recoverable archives. Drain project-wide drafts first. After Save, evict only clean idle caches synchronously; preserve pending/composing/new drafts against their original parent with a changed-version notice. Ordinary detached recovery does not require an open scene editor.


## October 13 — Validate newly authored fictional time

Validate only new or changed present event-time values at the author transition, including finite dates, ordered ranges and distinct retained relative anchors. Keep unchanged historical malformed/absent times and acknowledged recovery compatible. Preserve negative/fractional dates and unresolved cycles; validation must not manufacture feasibility or dates. This closes a reproduced backend/native bypass without backfilling legacy rows or broadening core-shape constraints.


## October 13 — Project-scoped private asset read

Expose the existing bounded/checksummed single-asset storage read through inherited private stdio for local font staging. Require explicit project and asset identities, return null for absent scope, and keep it read-only. Do not list/read every project BLOB or introduce a listener to obtain one font.


## October 14 — Revision-bound review and translation decisions

Keep explicit provider proposals separate from author acceptance. Before Apply, atomically verify the ordered complete saved-source manifest, then exact paragraph anchors; record acceptance only with final checked metadata/source CAS. If the record write fails after editor mutation, retain the unsaved draft and Undo and say acceptance failed. Translation sources stay immutable historical captures; optional generation is a cancellable page activity rather than a saving barrier. Resolve callbacks through current widget ownership, cancel only accepted actual release, and preserve generation through transfer. Use weak widget callbacks to avoid retaining closed controls. Choice dialogs hide/destroy after response and follow the parent focus policy.


## October 14 — Immutable alphabets and licensed private resources

Bind alphabet authoring to checked immutable history and explicit live scene sources. Publish mapping corrections as new versions and assign scenes explicitly; cached editor sessions retain their checked profile and Undo until close/reopen after manual draft decisions. Map committed ordinary typing once; literal input and stored text bypass mapping. Capture/reread the saved scene before spawning an editor.

Import only explicitly selected bounded resources with declared source/license and storage/transfer permission. Validate SFNT/pack schemas, family and checksum; stage fonts into the private helper directory, never install them globally. A font asset can remain licensed history after a later metadata CAS failure; retain the exact pending candidate for explicit retry/discard. Fall back visibly to default-font/literal editing without reopening typed content. Complete open/Discard only after viewport acknowledgement so ordinary navigation captures a settled frame.

Enforce pack version identities against checksum/ownership-verified complete retained history inside the same transaction as save/resolution and across archive/sync batches. Preserve unchanged partial legacy records without inventing identities. Bound history memory and scan every row with explicit keyset pagination; refuse corrupt or conflicting history rather than choosing a version.


## October 14 — Explicit recorded formatting sequences

Keep eight named slots with at most 32 allowlisted formatting, spacing and paragraph-language/direction commands. Record only successful acknowledgements against the captured pane; text, clipboard, navigation and Undo are never sequence steps. Save sequence settings against the captured sole settings head while preserving unrelated preferences. Recording never saves manuscript edits. Explicit Play and save each step drains ordinary drafts through Save/Discard/Cancel, then awaits the command and scene Save acknowledgement before advancing. Failure stops later steps and preserves earlier saved steps, the current unsaved draft and Undo.

Acquire a logical window/pane gate before changing sensitivity. Capture original permits before clipboard delivery, IM scheduling, source checks and tool/editor creation; recheck their owner, generation and mutation epoch before mutation. Allow only narrowly captured author-choice draft mutations during the initial drain and the executor's exact step Save; ordinary Save/Discard callbacks remain guarded. Read-only refresh and already dispatched acknowledgements stay authoritative. Cached recordings retain unique registered ownership for settings/leave decisions; physical input and playback require mounting. Ask about recordings before actual close/project teardown, retain them through Cancel and transfer their existing state when a tab moves. Refused clear must stop project switching before changing identity or caches.


## October 14 — Original manuscript and author-rule transliteration export

Use a separate bounded JSON manuscript copy for exact rich originals and immutable alphabet metadata; keep DOCX/complete archives distinct. Capture manuscript/alphabet heads in one read transaction and verify complete retained definitions before interpreting assignments. Do not embed font/pack bytes, guess transliterations, normalize stored text or replace originals. Derive scalar replacements once from the explicitly assigned historical version. Refuse ambiguous, corrupt or unsupported sources without dropping metadata. Bound capture, expansion and serialization incrementally; perform expensive parsing/encoding off GTK and reject obsolete delivery. Use ordinary manual draft decisions and the existing private atomic writer.


## October 14 — Shared bounded confirmed chronology analysis

Analyze authored ranges and signed relative equalities together with confirmed constraints, retaining contributor identities. Do not infer absolute dates or mutate canon. Propagate only qualified order into references; mark infeasible components, duplicate/unsupported evidence and work-limit exhaustion explicitly. Track representable arithmetic uncertainty so cancellation/rounding cannot create a confident order or contradiction; exact strict-zero statements remain meaningful. A path proving both directions never resolves to Before. Expose range uncertainty to future deterministic overlap checks rather than treating rounded endpoints as definite presence.


## October 14 — Core planning author transitions and typed values

Validate new/changed authored identities, supplied variants, owned applicability and custom field values at existing transaction hooks. Retain exact acknowledged historical rows, sparse omissions and legacy unavailable choices/links; do not backfill or rewrite them during load/import/recovery. Compare field ownership/kind against stored values before allowing definition changes/removal. Preserve canonical typed links and existing project-scene membership checks; unrelated literal strings remain text. Increased collisions and new malformed claims cannot inherit permission from an unacknowledged ancestor. Fix synthetic fixtures to express valid owned identities rather than weaken guards.
