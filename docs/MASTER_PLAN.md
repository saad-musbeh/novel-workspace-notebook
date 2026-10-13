# Accepted notebook redesign — October 10, 2026

The author chose minimal white notebook pages, manual saving, one top tab row and a left area selector with collapsible menus. Ordinary page clicks replace the active tab after Save / Discard / Cancel. Middle clicks create additional windows, and dragging out transfers a tab's existing state. Additional windows replace within-window splits. These decisions supersede earlier autosave/split interface requirements for the active Rust desktop. See NOTEBOOK_DESIGN.md, PROJECT_STATE.md and TEST_RESULTS.md.

# Implementation update — Rust interface, October 10, 2026

The user requested a Rust port including the interface. The default desktop is now native GTK/Rust, with a Rust Unicode/Pango/Cairo editor and the existing Rust SQLite companion/DOCX core. Build/run/package no longer need Node or Python. RUST_PORT.md records actual coverage and remaining specialized-interface parity; PROJECT_STATE.md and TEST_RESULTS.md record current evidence. Existing requirements below remain useful, but references to React/Tauri/Python describe the retained earlier implementation. Android remains deferred.

---

# Offline Novel Workspace — Master Plan

**Version:** 1.3
**Plan date:** October 2, 2026  
**Last revised:** October 6, 2026 — typing priority, scene references, reader reveals, character knowledge and selective project time travel added to desktop scope.
**Owner:** One writer; personal use  
**Development status:** Desktop implementation integrated and tested with synthetic fixtures; final desktop release gates remain open. Android is excluded from the current continuation.
**Working label:** Offline Novel Workspace. The name is a placeholder, not a product commitment.

## Current scope and implementation note — October 5, 2026

The user selected completion of the desktop application and excluded Android from this work. The original cross-device requirements below remain a future specification; Android, phone remote jobs and physical two-device gates are not current deliverables or passed milestones. See PROJECT_STATE.md and TEST_RESULTS.md for actual implementation and evidence.

The desktop now uses React/Tauri, an application-owned Unicode writer with installed Pango/Cairo shaping, a custom React/SVG planning board and a single-owner Rust stdio companion with SQLx/SQLite schema 3. Tiptap and React Flow are not current dependencies. Local writing, planning, recovery, archives, dictionaries/rules and optional loopback assistance are integrated. Private sync is explicitly enabled per session and exchanges revisions/assets only; no remote inference passthrough or persistent daemon is implemented.

Observed exceptions to the original target: permitted asset bytes are bounded verified SQLite BLOBs rather than external hash files; the WebView has no native browser writer fallback; daily backups run at companion startup, not continuously; there is no automatic whole-database restore UI or automatic peer discovery. Client credentials use the OS keyring; an approved synthetic store/read/remove roundtrip passed. Server auth uses an atomic private hashed-verifier file. Unattended keyring unlock remains unverified. A desktop release is not complete until physical input, packaging/install/update, private transport and other open acceptance checks pass.

The local Qwen3-4B evaluation validates evidence integrity but raises false positives on all four clean controls. AI remains experimental author-reviewed questions. Strict 3 ms whole-input processing is not guaranteed. Read MODEL_EVALUATION.md, PROOFREADING_EVALUATION.md and DESKTOP_PERFORMANCE.md before drawing capability or performance conclusions.

## Accepted desktop additions — October 6, 2026

The author selected ideas 1, 2, 3, 4 and 7: prioritize typing over background work; a scene-following pinnable reference pane; separate story occurrence and reader-reveal order; explicitly authored character knowledge by scene; and a project time machine that previews manuscript/bible together and recovers selected content as new revisions. Implementation and usage are in SCENE_TOOLS.md. The author accepts the measured approximately 72 ms processing for a single 54,000-character mixed-script paragraph; that extreme-case performance no longer blocks the desktop plan. Ordinary typing responsiveness, physical input and external private-network acceptance remain separate concerns. Android remains deferred.

## 1. Purpose and definition of success

Build a dependable writing application for a Fedora KDE desktop and a OnePlus 15 Android phone. Both devices keep local projects and support writing without the internet. A private connection synchronizes projects away from home and lets Android request work from an optional language model running on the PC.

The product promise is: **Keep the manuscript, characters, main plot, subplots, and chronology connected; surface possible inconsistencies without taking control away from the writer.**

A successful first personal release lets you write mixed-language scenes offline, organize character sheets and connected plot cards, maintain one chronology with main/subplot lanes, synchronize without silently losing conflicting work, and review local or PC-assisted language and continuity suggestions. The model is an optional assistant, not a requirement for opening, editing, saving, planning, searching, or exporting.

This is a staged software project, not a promise of complete plot-hole detection, equal proofreading quality in every language, or particular AI performance before hardware tests.

## 2. Confirmed requirements

| Area | Decision |
|---|---|
| User | You alone; no multi-user collaboration or commercial service in version 1. |
| Development | You have a CS background and will develop with coding-assistant support. Use ordinary engineering review and testing. |
| Desktop | Fedora Linux with KDE. Detect Fedora edition, release, session type, hardware, and dependencies rather than assuming them. |
| Phone | OnePlus 15 running Android. Detect actual OS, input methods, and WebView version during device testing. |
| Authoring | Write and edit the manuscript inside the application. |
| Desktop tabs and windows | Open scenes and workspace tools in tabs; move a tab into a separate PC window and back, with shared project state and safe saving. |
| Offline | Writing, planning, local search, installed dictionaries, and manual/rule-based continuity tools remain usable offline. |
| Languages | English, Arabic, simplified and traditional Chinese text, and invented alphabets. Writing-language support is separate from interface translation. |
| Plot organization | Freeform cards with labeled connections; one chronology with main plot and simultaneous subplot lanes. |
| AI | Optional local inference on the PC only. No Android model, cloud inference, or silent provider fallback. |
| Remote assistance | Android may explicitly ask the reachable PC to proofread or run its local LLM. |
| Synchronization | Must work away from home, using the accepted Tailscale approach. |
| Budget | No required paid editor features, cloud API, server rental, or app-store distribution. Existing hardware, electricity, connectivity, and development effort are not zero-cost resources. |
| Hardware selection | Include a read-only inventory and benchmark stage before choosing an AI model. |

Runtime network use is limited to project synchronization, requested PC assistance, and the networking infrastructure needed for the private connection. Initial installation, dependencies, language packs, model acquisition, and deliberate maintenance updates can require separate user-authorized downloads. No normal writing feature should start an unannounced download.

## 3. Scope boundaries

### Version 1 includes

A scene-based editor; desktop workspace tabs that can move into separate windows; custom character sheets; minimal place/item records; freeform boards; labeled relationships; a main/subplot chronology; manual issues; selected deterministic continuity checks; project dictionaries; offline synonym lookup where licensed resources are available; desktop proofreading; optional PC-local LLM review; Android remote jobs; two-device sync; recovery history; and portable project export/import.

### Deliberately deferred

Alternative-outcome universes and branching realities; real-time collaborative typing; accounts for multiple authors; automatic novel generation; hosted AI; AI inference on Android; social publishing; a plugin marketplace; a font-design application; arbitrary custom shaping engines; polished book typesetting; full DOCX import fidelity; EPUB production; vertical Chinese layout; sophisticated fictional calendars; mandatory background phone delivery; and remote wake-from-power-off.

Later features remain candidates, not implied obligations. The app should remain useful at the safe-editor and planning milestones even if AI is postponed indefinitely.

## 4. Product invariants

**The manuscript is authoritative.** AI results, extracted facts, search indexes, summaries, and board previews must not silently replace it.

**One story object has one identity.** A scene displayed on a board, in a chapter, and in a subplot is still the same scene. A plot point can link to multiple scenes, and a scene can advance multiple plot points.

**Manuscript sequence differs from story time.** Moving a flashback earlier in the book must not silently change its fictional date.

**Subplots share the same story world.** They are lanes, not alternate realities. A shared event has one canonical record with membership in several lanes.

**Unknown is not false.** Missing dates, uncertain identity, incomplete notes, intentional secrets, and character beliefs are not automatically contradictions.

**Saved means locally committed.** Display “Saving,” “Saved locally,” “Waiting to sync,” and “Conflict” separately. A remote acknowledgement is not a substitute for local durability.

**Concurrent writing is preserved.** Never resolve substantive content conflicts solely by device clock, arrival order, or last-write-wins.

**AI suggestions are revision-bound.** They reference the exact text and facts reviewed; changed material makes them stale until reviewed again.

**No networking feature is needed for local writing.** Failure of Tailscale, the companion service, the model, or the other device must not remove local access.

## 5. Primary user journeys

### Write anywhere

Open a local project, choose a scene, type in its assigned language, and see its save status. Edit without network access. Reopen after closing the app and recover the latest committed content. A later synchronization transfers the edits without requiring you to copy manuscript files manually.

### Plan a subplot

Create a plot lane, add a plot-point card, connect it to a cause or prerequisite, and link scenes. Move the card visually without changing chronology. Change its chronology explicitly through event properties. A shared event updates in every lane that references it.

### Track character knowledge

Mark a fact as known by a character from a specified event. Link a later use of that information. A checker evaluates the constraint only when relevant ordering is established. The author can record an earlier off-page explanation or an intentional exception.

### Request PC assistance from Android

Select a passage or review scope; see the included text, scene revisions, and relevant facts; choose a permitted operation; submit to the PC. If unavailable, retain the request locally. Once the PC durably accepts it, it may finish while the phone is disconnected. Retrieve the result later and review a diff before applying edits.

### Resolve an offline conflict

Edit the same scene on both devices before syncing. Display both versions and their common ancestor, when available. Choose one, combine them, or preserve both as drafts. Save an explicit resolution revision that acknowledges the conflicting parents.

## 6. Interface plan

### Desktop workspace

Use a left project navigator, a central editor/board/timeline workspace, and a collapsible contextual inspector. Provide keyboard navigation, a command palette, resizable panes, a distraction-free editor, and a theme setting that can follow the system.

Primary destinations: **Write**, **Plan**, **Story Bible**, **Review**, and **Project Settings**. Sync status and PC-assistance status are different indicators. The review panel distinguishes manual notes, rule-based findings, proofreading findings, and LLM suggestions.

### Desktop tabs and detachable windows — added October 5, 2026

Provide workspace tabs for scenes and tools such as planning, story records, review and project settings. A keyboard-accessible **Open in new window** action moves the selected tab to an independent PC window, allowing writing and reference material to remain visible side by side. **Move back to main window** returns it without duplicating project content. Tab titles identify the scene/tool and project; each window shows its own save/error state. Opening an already-open scene focuses its existing tab/window by default.

Detaching retains the tab's current document, caret/selection, scroll position and undo history. Finish active composition and drain pending saves before handing off its editor session. Remove the source tab only after the destination is ready; a save, conversion or window-opening failure leaves the source usable with a visible error. Moving a view must not create a manuscript revision by itself.

Closing one window saves its pending edits and leaves other windows usable. Quitting the app drains all windows and retains existing refusal-on-composition/save-failure behavior. Different scenes may be edited in separate windows; changes to shared planning/project data retain revision checks and explicit conflict handling. All windows see acknowledged changes without replacing pending drafts.

Implement the tab model and safe session handoff before multi-window polish. Acceptance includes Arabic/Chinese typing and undo, independent cursor/rendering sessions, shared-state/conflict behavior, source/destination failure recovery, window/whole-app close, and long-project responsiveness with multiple windows. **NW-049** implemented this addition in the retained React/Tauri shell: its registry bounds were 8 windows, 64 tabs and 16 editor sessions, with global exact-route reuse, private editor/Undo handoff and rollback on failed or mismatched acknowledgment. The probe counts in the following paragraph belong to that retained shell. The current Rust/GTK notebook instead keeps at most 16 live scene sessions per window, uses ordinary detachable pages and manual Save/Discard/Cancel, and preserves dirty, pending, composing or recording cached sessions. Its actual ownership/close/transfer checks are recorded by slice in PROJECT_STATE.md and TEST_RESULTS.md; retained shell limits or probes do not qualify the GTK interface. Explicit refresh and expected-parent saving must preserve pending drafts.

Automated qualification is partial: 16 pure Rust registry cases, 10 editor-session fixture executions, 26 workspace coordinator cases and 3 route cases passed. Latest isolated debug runs passed 33 default checks and 30 original-window-close checks, including real owned GTK close, continued detached exact saving, composition refusal without extra child/revision and background-view retention. Window-specific native events and JavaScript listeners are scoped to their window label; shared content and whole-app quit cancellation remain broadcasts. The exact final NW-049 RPM passed isolated 33-check multi-window and 30-check original-close runs, plus an offline 1,000-scene/28-check workflow. Dynamic egress instrumentation observed no destinations or resolver inputs on its intercepted paths; this is not a full OS audit. Its detached personal-build signature passed independent public-key verification. Installation, trusted RPM signing, physical IME/fullscreen, supplied-font rendering, three-window latency, real private Tailscale and LLM accuracy gates remain open.

### Remove outer writing padding — accepted October 5, 2026

Remove editor-card margins and page/header/toolbar/footer padding so the draft fills its available pane. Keep the formatting controls, save state, focus indication and editable scene title. Ordinary control hit areas remain usable. Verify both split panes and the single editor at narrow and wide sizes.

### Split editing and compact writing workspace — accepted October 5, 2026

The author selected Scrivener-style split editing as the priority and also requested Scribeist-style writing workflows. Add two independently editable drafts side by side in one desktop window, with a resizable divider and a clear active pane. Open a scene beside the current draft through its context menu; allow returning to one editor without closing or discarding the other tab. Reuse existing native sessions so each scene retains its own caret, scroll, undo and save state. Opening the same scene again must reuse its existing owner rather than create competing writers.

Use a compact manuscript binder and an optional scene inspector for synopsis, status, viewpoint, tags and event links. Load inspector metadata when requested; ordinary writing must not load all project bodies. Keep formatting and save feedback visible while each draft scrolls. Split mode must remain useful at smaller widths, with keyboard-accessible resizing and pane selection. This addition is **NW-050**; implementation and exact packaged verification are in progress. Scrivener and Scribeist are workflow references, not a claim of complete feature parity.

The author also requested browser-style tab dragging and automatic closure of a source window after its final tab moves successfully. Window actions belong in tab menus rather than a separate Window button. Add configurable Word-style keyboard actions and mouse selection behavior, including line-spacing controls. Preserve composition guards, explicit conflicts, clipboard boundaries and safe close drains. These additions are **NW-051**; source implementation exists and final release checks are in progress.

Reference workflows: [Scrivener split editor](https://www.literatureandlatte.com/blog/see-more-of-your-project-splitting-the-scrivener-editor) and [Scribeist editor](https://scribeist.com/help/docs/editor/overview/). Existing local-only AI and explicit private-sync constraints continue to apply.

### Independent text engine and Unicode input — accepted October 5, 2026

The author requested removal of the LibreOffice editing backend. **NW-052** replaces it with an application-owned document/editing model, independent session undo, Unicode cursor/selection logic and viewport rendering through installed Pango/Cairo. Scrivener and Scribeist supply workflow references; their proprietary backend implementations are not used. Keep existing SQLite revisions, explicit conflicts, project archives, private fonts, split panes and safe window handoff. A new backend must preserve no-edit document JSON and all original code points, refuse unsupported changes before mutation, and pass exact saving/reopening, Arabic/kasra, mixed-direction selection, keyboard, clipboard, long-draft and resize regressions before installation. Earlier LibreOffice measurements remain historical and do not qualify its replacement. Do not uninstall the author's separate LibreOffice application.

**NW-053** adds configurable Word-style Alt+X input. Convert selected or immediately preceding hexadecimal Unicode numbers into valid scalar characters, and convert characters back to hexadecimal. Include Arabic, supplementary-plane letters, emoji, combining marks and private-use characters. Invalid, surrogate or out-of-range values must leave writing unchanged and show a nonfatal message. Font coverage remains separate from Unicode preservation; never normalize, replace or silently discard an unfamiliar character. Implementation and packaged verification are in progress; Android remains excluded.

### Android workspace

Use a single main pane with bottom navigation or a compact drawer. The editor must remain usable with the software keyboard open. Character fields and event properties use touch-friendly forms. Boards have a synchronized list alternative; connecting two selected cards must be possible without precision dragging. Complex editing actions use explicit controls rather than hover behavior.

### Accessibility and language UI

Support scalable text, keyboard focus, descriptive labels, adequate contrast, and reduced animation. The first interface can be English, with UI strings externalized for later Arabic and Chinese translation. Full multilingual authoring is not postponed until UI translation.

## 7. Manuscript editor

Organize projects into optional parts, chapters, and independently stored scenes. The initial rich-text schema includes paragraphs, modest headings, emphasis, scene breaks, and optional inline language/script metadata. Avoid turning the editor into a page-layout system.

Each scene has a stable identifier, title, synopsis, drafting status, optional viewpoint character, tags, links to story events, and its current revision head or conflict state. Titles and positions are editable metadata, never object identity.

Support undo/redo, local autosave, scene/chapter navigation, find/replace with preview, selected-text actions, and explicit snapshots. Maintain stable block IDs for linking review evidence where practical; destructive restructuring may invalidate anchors and must be handled visibly.

Store a versioned editor document as the source of truth, with a rebuildable plain-text projection for search and review. Never make flattened plain text the only rich-text backup. Schema changes need migrations and fixtures.

Autosave should commit after completed editor transactions using a small debounce, without disrupting active input-method composition. Do not rely on an app-close callback to persist every edit. The UI must distinguish committed text from edits still pending; tests should verify recovery to the last acknowledged commit, not claim recovery of keystrokes that never reached storage.

Statistics include readable word counts for appropriate languages and character/grapheme counts. Chinese should not be measured solely by whitespace splitting. Show the counting method and do not compare unlike measures as if they were identical.

## 8. Multilingual and invented-alphabet requirements

### Arabic

Store paragraph direction independently of visual alignment. Support automatic base direction and explicit LTR/RTL overrides, mixed Arabic/Latin text, numbers, punctuation, diacritics, and selection across direction changes. Follow directional-markup principles rather than attempting to fix Arabic with right alignment alone. [S8]

### Chinese

Test actual Chinese input-method composition, candidate selection, cancellation, punctuation, selection replacement, undo, copy/paste, and app background/resume. Preserve simplified and traditional characters exactly as entered; no automatic conversion. Start with horizontal writing.

### Search and text integrity

Retain original stored text. Any case folding, diacritic-insensitive matching, or normalization belongs in a derived search representation and must be a visible option. Provide exact substring search first; evaluate language-specific segmentation later. Test Arabic combining marks, supplementary characters, mixed scripts, and invented-alphabet text. Do not assume one generic tokenizer adequately handles every language.

### Invented alphabets

Create a versioned `WritingSystem` record containing name, script identifier, encoding/mapping mode, character-to-input mapping, font-asset references, direction preferences, optional transliteration rules, glossary, and explanatory notes.

For an alphabet that simply represents existing underlying text, an appropriate custom-font presentation may be sufficient. For distinct symbols, evaluate a documented private-use Unicode mapping. Private-use character interpretation and operating-system behavior are not universally standardized, so font display alone does not establish arbitrary direction, casing, or shaping behavior. [S9]

The first custom-script milestone supports a tested alphabet through a character palette/input mapping and user-provided font, not every conceivable writing system. An RTL invented script needs explicit cursor, ordering, and round-trip tests before claiming support. A complex shaping requirement may need a separate rendering strategy.

Synchronize the mapping and permitted user-supplied assets. Detect incompatible mapping versions instead of silently reinterpreting old text. Export the original text plus mapping metadata and an optional transliterated copy. A transliteration is not assumed to be reversible or linguistically understood by the LLM.

This plan packet contains no font files. The actual app's font-import feature will use assets you supply and are entitled to use or transfer.

## 9. Story bible and character sheets

Start with configurable templates containing short text, long text, number, date/relative-time reference, single/multiple choice, and links to other story records. Support adding, renaming, reordering, and hiding fields without deleting stored values. Removing data requires an explicit action.

Suggested character sections: identity and aliases; background; goals and motivation; traits; relationships; appearance notes; and scene/event links. These are optional templates, not mandatory story-writing rules.

Separate relatively stable descriptive details from event-dependent state. Character knowledge, possessions, location, occupation, and relationships can change. A state record should specify subject, property, value, applicable event or interval, epistemic status, source, and author confirmation.

Epistemic status distinguishes established story-world facts, a character's beliefs, rumors, and uncertain author notes. An LLM extraction begins as a proposal. The author must approve it before it becomes confirmed canon.

Add simple location and item records because continuity rules can refer to them. Detailed maps, inventories, and world encyclopedias are not required for the first release.

## 10. Freeform board

Provide movable cards for plot points, scenes, characters, events, and notes, with groups, labels, filters, zoom, and pan. The current desktop implements a custom React/SVG board; React Flow was the original candidate, not the story data model. [S4]

A board card contains a reference to its story object plus layout information. Deleting a board representation must not automatically delete the referenced scene or character. Offer separate commands for removing a card and deleting a story record.

Connections have explicit types: reference, causes, happens-before, reveals, depends-on, setup/payoff, or a custom label. Only declared semantic relationships participate in continuity checks. A visual connector must not acquire hidden chronological meaning.

Card positions belong to a shared board layout. Device viewport, zoom, current selection, panel sizes, and recent navigation remain device-local. Do not create sync conflicts over which area of the board you last viewed on the phone.

## 11. Timeline and subplot model

Use one event chronology with main/subplot lanes. Events can belong to several lanes and link to several scenes. Scenes may portray multiple events; no forced one-scene/one-event assumption.

Support exact story dates, intervals, approximate bounds, relative offsets such as “three days later,” ordering constraints, and unknown time. Story time is its own domain: do not convert fictional dates automatically using device time zones. Use UTC timestamps only for real-world save/job metadata, not as a conflict-resolution rule.

Separate strong constraints from informal notes. Detect cycles in strict happens-before relationships. Inconsistent numerical constraints should show the contributing constraints; underdetermined timing should stay unresolved rather than producing a fabricated date.

Changing one event's time should preview affected constraints and linked events. Do not silently reschedule the whole book. A character's participation in overlapping events is a review candidate only when timing and mutually exclusive presence requirements are sufficiently specified.

## 12. Continuity and plot review

### Manual issue tracking

An issue contains category, description, affected records/passages, author-assigned importance, evidence, status, and resolution notes. Statuses: open, investigating, intentional, resolved, dismissed, and stale. Manual issues are useful before AI or automatic rules exist.

### Initial deterministic rules

| Rule | Preconditions | Output |
|---|---|---|
| Strict ordering cycle | Confirmed happens-before edges | Show the cycle and the edges to revise. |
| Missing prerequisite | Confirmed prerequisite plus comparable event order | Link the prerequisite and dependent event. |
| Knowledge used too early | Confirmed learn/use records with established order | Explain the knowledge dependency. |
| Incompatible simultaneous presence | Comparable intervals plus explicit exclusive-presence assumptions | Show overlap and location constraints. |
| Conflicting confirmed state | Overlapping applicability of incompatible confirmed values | Link sources and ask for reconciliation. |
| Unresolved setup | Setup explicitly marked as requiring payoff | List as a review reminder, not a proven plot hole. |

Absence of notes is not proof of a contradiction. Rule configuration must accommodate intentional secrets, off-page events, unreliable narration, and genre exceptions. “Teleportation is impossible” must never be an unstated universal assumption.

### Optional LLM review

Begin with selected scenes and explicit facts, not an unbounded “understand my entire novel” button. Build a pipeline: select scope; fetch relevant canonical facts and source revisions; fit an explicit context budget; run the local model; validate structured output; verify evidence pointers; and present provisional findings.

Do not treat summaries as more authoritative than manuscript passages. Cross-scene and whole-project review can be phased, with a displayed list of included/excluded scenes and an incomplete-coverage warning. If evidence is unavailable, label the result as a question rather than inventing quotations.

A result should say “No issues found in the checked scope,” not “Your novel has no plot holes.” Model confidence numbers should not be presented as calibrated probabilities unless they have actually been calibrated.

## 13. Grammar and synonym tools

### Offline on both devices

Provide the project dictionary, spelling-exception list, language assignment, explicit non-AI rules, and installed lexical lookup. Keep optional packs separate from the core app. Each pack records language/variant, source, version, license, checksum, and supported operations. A missing pack produces an installation/import choice, not an automatic network lookup.

### PC proofreading

Evaluate a self-hosted LanguageTool instance for rule-based proofreading, separate from the LLM. LanguageTool documents a local server but excludes its cloud AI rules from that setup. [S10] Its published language table lists Arabic, Chinese, and English with differing coverage and is dated March 27, 2025; use it as evidence of available language work, not a current quality guarantee. Test the exact installed version. [S11]

Use author-selected or paragraph-level language where possible. A mixed-language paragraph may need segmented checking or explicit selection; do not send all mixed text through one guessed language configuration.

The phone may call the PC's non-AI proofreading service as well as its LLM. Without the PC, the phone retains its own lighter non-AI tools. Do not promise identical grammar correction on both devices while offline.

### Synonym lookup

Use licensed local lexical packs, with Open Multilingual Wordnet as an initial resource candidate. Its downloadable English, Arabic, and Chinese resources have different coverage and licenses. [S12]

Group alternatives by sense and part of speech where supported. Distinguish synonyms, related words, and translations. Preserve inflection or warn that a replacement may need grammatical adjustment. Show the proposed replacement in context. Optional PC-LLM contextual suggestions must be visibly distinct from dictionary entries.

Every writing suggestion requires accept/reject controls and ordinary undo. Dialogue, dialect, intentional fragments, and invented names should not be silently standardized.

## 14. Proposed technology stack and validation gate

| Layer | Initial choice | Reason and limitation |
|---|---|---|
| Application shell | Tauri 2 | Candidate shared Linux/Android shell; validate actual input behavior before committing. |
| User interface | React + TypeScript + Vite | Shared UI, typed view models, and a web-based editor/board. |
| Editor | Installed LibreOfficeKit via native helper | Current Fedora writer; exact conversion and kasra fixtures pass; physical IME/accessibility/runtime packaging still need validation. |
| Board | Custom React/SVG board | Canonical planning records are independent from viewport/card rendering; interaction remains a release measurement. |
| Core/domain | Shared Rust crates | Reuse validation, storage, sync rules, and deterministic checks. |
| Database | SQLite behind typed Rust repository methods | SQLx is the initial adapter candidate; compile/test both targets in M0. |
| PC companion | Rust/Tokio, bounded stdio JSON and narrow explicit HTTP listener | Current single persistence owner; HTTP accepts only authorized revisions/assets, without inference passthrough. |
| Remote connection | Tailscale + private Serve HTTPS | No public Funnel and no hosted manuscript backend. |
| Local LLM runtime | llama.cpp server adapter | PC-only; model selected after hardware and language evaluation. |
| Proofreading | Optional local LanguageTool process | PC-only process, no cloud endpoint. |

Tauri documents Fedora and Android prerequisites; its SQL plugin documents SQLx-backed SQLite support on Linux and Android. This supports feasibility, not a claim that our integration has already been tested. [S1, S2] Tiptap distinguishes its MIT-licensed open-source components from paid extensions; we must audit individual packages and implement our own revision history. [S3]

Pin exact package versions and the chosen Rust/Node/Android SDK/NDK toolchains after the prototype passes. Commit lockfiles. Do not put guessed “latest” version numbers into the architecture as permanent requirements.

**Go/no-go gate:** Arabic editing, Chinese composition, custom-script round trips, and a local database read/write must work on the real PC and phone. A basic browser preview is insufficient. If the editor or WebView path fails, isolate the cause and reassess the affected editor/shell layer before building the full planning UI. Keep the domain and storage format independent enough to survive that decision.

## 15. Runtime architecture

The PC runs a local companion process that is the single owner of its project databases. The desktop UI communicates through a protected local IPC channel. It launches the companion on demand; later installation may configure it as a user-level service. No network listener is required for the first writing milestone.

For desktop multi-window support, keep one application-owned companion/database owner shared by all windows; closing the original window must not terminate it while another window is open. Route native editor commands and responses through explicit tab/session ownership rather than a single global active document. A detach transfers ownership of the existing editor session so undo history survives; separate editing tabs must never receive each other's cursor, bitmap or checkpoint. Coordinate save/close acknowledgements across windows, and stop the companion only after the last window or explicit application quit has safely drained. Open editor sessions on demand and measure their memory cost.

Android embeds the shared core and owns its own local database. It never delegates local editing or saving to the PC. Its native layer manages remote connections and credentials; the WebView does not receive unrestricted database, shell, or token access.

When remote access is enabled, Tailscale Serve proxies private HTTPS to the companion's loopback HTTP endpoint. The companion validates paired-device credentials and allowed operations. It alone talks to the PC's loopback model and proofreading processes. Do not proxy the raw LLM API directly to the phone.

This separates the desktop window, persistence, sync, and inference. The current desktop closes its child companion and session listener when the window exits. A separately configured future companion service could leave remote work available; no such service is installed or enabled here. Sleep, shutdown, lost networking, an expired network credential, or a stopped companion can make the PC unavailable. These states must be shown honestly.

Use a process singleton and controlled startup/migration ownership. Android does not ship or start a companion daemon, model runner, or proofreading JVM.

## 16. Data model and document contract

All mutable domain records have locally generated stable UUIDs. Display order and titles are not identifiers. Revisions are immutable and refer to their parent revision IDs; conflict heads are preserved until explicitly resolved.

| Entity group | Main records |
|---|---|
| Manuscript | Project, Part, Chapter, Scene, SceneDocument |
| Story organization | PlotThread, PlotPoint, StoryEvent, TimeConstraint |
| Story bible | Character, Location, Item, Relationship, FieldDefinition, FieldValue |
| Knowledge/state | StoryFact, CharacterState, KnowledgeEvent, EvidenceAnchor |
| Planning views | Board, BoardNode, BoardEdge, EventThreadLink, SceneEventLink |
| Language | WritingSystem, GlossaryEntry, DictionaryException, Asset |
| Review | ReviewIssue, Suggestion, ReviewRun, AIJob, AIResult |
| Persistence/sync | EntityRevision, RevisionParent, EntityHead, ChangeLog, Device, PeerCursor, Tombstone, SyncConflict |

Avoid implementing every table on day one. Start with Project, Chapter, Scene, revisions, and durable change tracking. Add feature-specific schema through tested migrations. Keep core relationships explicit; use versioned JSON for editor documents and custom field values rather than making all data an unstructured blob.

A revision envelope contains `project_id`, `entity_id`, `entity_type`, `revision_id`, `parent_revision_ids`, `origin_device_id`, `device_epoch`, `local_sequence`, `schema_version`, `payload`, and `payload_hash`. Wall-clock creation time is useful for display, not causality.

A scene document contains schema version, stable block identifiers where feasible, paragraph direction/language attributes, allowed formatting, and script references. Evidence anchors include scene revision, block identifier, an explicitly defined offset unit, and a short matching quote/context. Conversion between JavaScript UTF-16 offsets and native text indices must be tested; ambiguous offsets should not be applied to new text.

The target design places binary assets outside the database under content hashes. **Current desktop deviation:** verified bounded asset bytes and resumable chunks are stored in SQLite BLOBs for transactional recovery; moving to external content-hash storage remains an architectural change, not a completed feature. Asset references, size, media type, mapping version, and checksums are metadata. Search indexes, thumbnails, and model caches are rebuildable and should not be synchronized as authoritative content.

## 17. Synchronization protocol

### Topology

Use a two-device, PC-hub design. Both devices are local-first, but the phone exchanges revisions through the PC companion. No cloud mailbox is included. At some point both devices must be reachable to transfer changes; neither has to remain online while the user is simply writing.

### Transactional write contract

Every accepted local change commits its new revision, head update/conflict state, and outbound change marker in one transaction. A failed transaction must not leave a save marker without corresponding content. User-visible save acknowledgement follows commit.

### Exchange sequence

Authenticate and negotiate protocol/schema versions. Push locally missing revisions with their required ancestors. Deduplicate by stable operation/revision identity and verify payload hashes. Commit received revisions and conflicts before acknowledging them. Pull remote changes using a PC-issued cursor; request missing assets; verify completed asset hashes; and record the applied cursor transactionally. A partially downloaded asset remains visibly pending.

Retries must be safe. An uncertain network outcome triggers a status query or idempotent resend, never a second independent edit. Resume chunked transfers from verified state. Do not mark a project fully synchronized while required assets or unresolved transfer errors remain.

### Conflict policy

A revision descending from the known head can advance it. Concurrent descendants retain multiple heads and create a conflict. Different entities may merge automatically; concurrent edits to the same scene are reviewed in version 1, even when a more elaborate text merger might combine them. This conservative choice prioritizes preservation over convenience.

Treat ordering collections as revisioned records too. Concurrent chapter reordering, edit-versus-delete, custom-script mapping changes, and semantic edge edits must not use hidden last-write-wins behavior. A resolution creates a revision acknowledging the relevant conflicting parents.

### Deletion, restore, and compatibility

Represent deletion with a tombstone, not immediate disappearance from replication history. Avoid history/tombstone compaction in the first release. History can therefore grow; expose its storage usage, skip identical saves, and allow lossless compression/deduplication without removing referenced revisions. Later compaction needs acknowledgement rules or a forced rebootstrap for long-offline devices.

If a PC backup is restored, generate a new hub epoch and re-establish peer cursors; do not trust cursors from the future of the restored database. Preserve revision identities to deduplicate already-known content. Device reinstall/re-pairing creates a new device epoch and fresh credentials.

Reject unsupported protocol/schema versions with an understandable message. An app that cannot understand newer project fields must not silently erase them on its next save. Never synchronize the active SQLite file through a generic file-sync folder.

## 18. Remote PC jobs

### Job lifecycle

`pending_local` means the request exists only on the phone. After the PC durably stores it, its server state becomes `queued`, then `running`, and finally `succeeded`, `failed`, or `cancelled`. A PC interruption marks a running attempt as interrupted; retries are explicit or governed by a documented retry policy. Do not falsely claim an inference resumed from its exact prior token.

Each request includes a unique idempotency key, task type, project ID, exact source revision IDs and hashes, selected text snapshot where necessary, fact/context revision references, language/script metadata, and task options. A retry reuses the same key and payload. Editing a queued request creates a new job.

The server stores input and job state before acknowledgement. It persists results for later retrieval. Streaming output can be an optional convenience, but the durable result is authoritative. Cancel and timeout controls must not delete the manuscript or earlier results.

### Operations

Allow selected-text grammar review, contextual wording/synonyms, scene consistency review, scoped cross-scene review, and proposed fact extraction. The companion chooses the configured local provider appropriate to the requested operation. The phone cannot submit shell commands, arbitrary server file paths, external fetch URLs, or unapproved model-management actions.

### Model management

Benchmark candidate models only after the hardware report. Record model source/license, file hash, runtime version, quantization, context setting, prompt-template version, language results, peak memory, and latency. Begin with one inference job at a time; lower-priority reviews must not make writing unresponsive. Model downloads are deliberate setup operations, never a cloud fallback.

llama.cpp provides a local server and health endpoints that can help distinguish loading from readiness. Our companion still needs its own queue, authentication, revision handling, and crash recovery. [S5]

### Availability and freshness

Phone states: disconnected, PC reachable, assistance unavailable, model loading, ready, busy, and result available. Sync readiness must not depend on model readiness. A model can be unloaded while the companion continues synchronizing.

Before inference, freeze and verify every referenced scene/fact revision in the input manifest. If a required revision is missing, request it or reject the job; never silently substitute the latest local text. A result records model/provider, reviewed scope, revisions, findings, and proposed changes. Any changed source or supporting fact invalidates freshness. Display stale findings for comparison but never apply stale offsets automatically. Acceptance creates normal revisions and participates in ordinary sync/conflict rules.

Android background execution is not guaranteed: Doze and App Standby can defer work and network access. Version 1 guarantees refresh when the app is opened or Sync is requested, not instant delivery while it is suspended. [S13]

## 19. API outline

This is an initial contract sketch, not implemented or frozen API documentation. All business endpoints require device authorization and project access checks.

| Operation | Proposed endpoint | Important behavior |
|---|---|---|
| Service/protocol status | `GET /v1/status` | Minimal information; no manuscript content. |
| Explicit device pairing | `POST /v1/pair/complete` | Short-lived, single-use challenge plus PC approval. |
| Push revisions | `POST /v1/sync/push` | Idempotent batches; atomic acknowledgement. |
| Pull revisions | `GET /v1/sync/pull?cursor=...` | Bounded responses; epoch/version checks. |
| Transfer assets | `PUT/GET /v1/assets/{hash}` | Verified content, project authorization, size limits. |
| Submit review | `POST /v1/jobs` | Immutable input snapshot plus idempotency key. |
| Inspect job/result | `GET /v1/jobs/{id}` | Durable status and result; ownership check. |
| Cancel job | `POST /v1/jobs/{id}/cancel` | Idempotent cancellation semantics. |
| Revoke device | Local PC command | Invalidates application credential; separately remove network access. |

Return structured errors for authentication, unsupported versions, validation failure, missing ancestors/assets, busy capacity, stale preconditions, provider unavailable, and cancellation. Do not expose raw stack traces or local paths to the phone. The target is generated TypeScript contract types from a single schema or Rust contract source with compatibility fixtures. As of October 14, the generator covers 56 selected storage wire structs from actual Rust serde declarations and 62 direct stdio storage RPC signatures from companion source, with deterministic drift checks and real Rust serializer fixtures. It explicitly lists 46 uncovered operations, including provider/peer/sync/asset/backup paths, Statistics and several mutation/export methods; unsupported signature forms fail closed. Opaque DocumentNode validation and detailed JSON schemas remain separately maintained. This is bounded source-derived coverage of the local stdio contract, not full shared runtime-schema generation or acceptance of the draft HTTP route table.

## 20. Privacy and security

Tailscale's current Personal plan is listed at $0 for non-commercial personal use. Use ordinary private networking, not an AI gateway. [S6] Tailscale Serve is private to the tailnet; Funnel exposes services publicly and is excluded. Loopback-only backends reduce bypass risk. [S7]

Tailscale may relay traffic when a direct route is unavailable; its documentation states that DERP relays forward encrypted traffic without decrypting it. [S14] Its privacy policy documents networking/device metadata collection, while distinguishing the encrypted traffic content from content processed by separate gateway products. Our design does not use those gateway products. [S15]

Require explicit pairing, per-device credentials, project authorization, rate/size limits, and revocation. Use OS-protected credential storage through a tested platform adapter. Keep native capabilities narrow; do not grant arbitrary shell or unrestricted file access to the editor. Scope file imports to deliberate user selections.

Bundle UI assets locally. Disable application telemetry, remote fonts, remote analytics, automatic update checks, and cloud proofreading. Block automatic remote image loading in pasted/imported content. Sanitize imported HTML and model-rendered output; treat manuscript text as untrusted data, not instructions with authority to invoke tools.

Do not disable SELinux or the firewall to make a prototype work. Identify and fix specific packaging/network permissions. Do not create a public router port-forward to the model server.

Transport encryption is not at-rest database encryption. The initial storage design relies on the device's actual disk encryption, lock screen, permissions, and Android application isolation; verify those properties rather than assuming them. Raw exported project archives are not password-encrypted unless an explicit encryption feature is implemented. Warn before exporting sensitive material to shared storage. Do not invent custom cryptography.

On Android, review backup/data-extraction configuration so application-managed manuscript files are not silently sent to a platform cloud backup. External keyboards, OS services, screenshots, and user-chosen external backups have their own behavior outside the app's network boundary; document this limitation instead of claiming control over them.

Credentials, manuscript samples, model files, personal reports, and signing keys do not belong in Git. Disable content logging by default; useful diagnostic logs contain operation IDs and redacted errors. Treat a compromised unlocked device as outside the app's protection guarantee.

## 21. Recovery, export, and update safety

Provide local recovery snapshots and an independent portable project archive. A suggested adjustable policy is recent-session recovery plus daily backups; the exact retention budget is chosen after real project sizes are measured. Never erase unresolved conflict revisions to satisfy a retention quota.

Use a database-consistent snapshot operation; SQLite documents its backup API and related snapshot mechanisms. Do not copy only a live database file and assume associated state is captured. [S16]

A project archive contains a manifest, versioned content/metadata, asset references and included permitted assets, writing-system definitions, and checksums. Exclude device credentials and LLM binaries. Validate archive paths, sizes, schemas, and hashes before import. Restore into a new project by default; restoring over a synced project requires the explicit recovery flow and hub-epoch handling.

Initial human-readable exports: UTF-8 text, Markdown where representable, and self-contained HTML with explicit language/direction metadata. Explain losses such as formatting in plain text or custom-script rendering without matching assets. Full-project export is the fidelity-preserving format; manuscript export alone is not a complete backup.

Before migrations, create a recoverable snapshot. A failed migration must not launch normal editing against half-upgraded data. An older app must not open a newer schema for writing. Publish local changelog and compatibility notes with personal builds.

## 22. Repository and engineering workflow

Suggested future structure:

```text
novel-workspace/
  apps/writer/                  # Shared React UI + Tauri desktop/Android adapter
  crates/domain/                # Story types and invariants
  crates/storage/               # SQLite repository and migrations
  crates/sync/                  # Revision protocol and conflict rules
  crates/review/                # Deterministic checks and result validation
  crates/companion/             # PC-only persistence/service/job host
  packages/contracts/          # Generated schemas and TypeScript types
  tests/fixtures/               # Synthetic multilingual and sync data
  tests/scenarios/              # Recovery, conflict, and review scenarios
  tools/                       # Environment and development helpers
  docs/                        # Plan, decisions, test results, project state
```

This is a destination structure, not a reason to scaffold empty abstractions immediately. Start with the shell, a minimal core, and one end-to-end scene save. Split modules when their responsibilities exist.

Use a local Git repository; remote hosting is optional. Work in small branches/commits with a written acceptance condition. Each change should contain its implementation, tests, any migration, and a short note about what was actually run. Do not accept coding-assistant assurances as test results.

Keep `PROJECT_STATE.md` updated with the last working milestone, verified environments, current commit, known issues, and next small task. It lets a later development conversation resume from concrete state without relying on chat memory.

Proposed test layers: Rust unit/integration tests for core behavior; TypeScript tests for adapters and pure UI logic; browser interaction tests for ordinary editor behavior; real-device manual or instrumented tests for input methods, lifecycle, signing, and networking. Browser automation alone cannot certify the native input-method experience.

## 23. Original milestones and release gates

| ID | Deliverable | Required exit evidence |
|---|---|---|
| M0 | Hardware inventory and cross-device technical spike | Actual hardware report; Fedora edition identified; Linux and Android shell builds; Arabic/Chinese/custom-script test results; SQLite read/write; stack decision recorded. |
| M1 | Safe offline editor | Projects/chapters/scenes; autosave and revisions; basic search/export; explicit save status; crash recovery to acknowledged commits; tested full-project round trip. |
| M2 | Story bible and connected planning | Custom character fields; scene links; board nodes/edges; main/subplot lanes; chronology; manual issues; no duplicate canonical event records. |
| M3 | Conflict-safe private sync | Pairing; revision exchange; asset transfer; conflict UI; tombstones; restore epochs; successful away-from-home mobile-data test; interrupted/retried transfer tests. |
| M4 | Non-AI language and continuity tools | Installed lexical packs; dictionary exceptions; initial deterministic rules; local PC proofreading; documented language limitations and false-positive tests. |
| M5 | Local desktop AI | Hardware-appropriate model evaluation; one-scene review; verified evidence anchors; proposal approval; no internet inference; resource/cancellation tests. |
| M6 | Android PC-assisted jobs | Durable queue; explicit scope/snapshots; retries; disconnect/reconnect; stale-result handling; local proofreading and LLM requests over private access. |
| M7 | Personal version 1 | Signed installable builds; migration/restore drill; network/privacy audit; actual-device regression pass; documented limitations; no known data-loss bug. |

The October 5 desktop scope addition NW-049 (workspace tabs and detachable windows) is required before the final desktop M7 release gate. It does not mark any original milestone passed.

For the original cross-device release, the critical path is M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7. The current desktop continuation implements desktop portions across these milestones without claiming the original physical-device gates passed; M6 phone work is excluded. Some documentation and fixtures can be prepared in parallel, but no deadline or development-duration estimate is implied. If M0 exposes an editor problem, address it before expanding scope.

Useful intermediate releases are the offline writer after M1, the planning workspace after M2, and the synchronized two-device writer after M3. The architecture is valuable before any model is installed.

## 24. Testing and measurement plan

The companion `ACCEPTANCE_TESTS.md` is the executable planning checklist. No entry begins as passed.

Test multilingual round trips, mixed-direction editing, IME composition/cancellation, undo, app suspension, process crash, storage exhaustion, export/import, schema mismatch, invalid archives, asset corruption, duplicate sync requests, concurrent edits, delete/edit conflicts, stale cursors, PC restoration, device revocation, model errors, and unsupported model output.

Build synthetic project fixtures with known chronology and knowledge relationships, alongside intentional ambiguity and valid narrative techniques. For each deterministic rule, include both positive and negative examples. For AI, evaluate useful findings, missed seeded problems, false warnings, evidence validity, and unwanted meaning changes separately by language. Do not collapse these into a marketing “accuracy” number.

A provisional performance fixture is 1,000 scenes, 500 character records, 2,000 story events, and a 1,000-card board, plus manuscript samples around 250,000 English words or 750,000 Chinese characters. These sizes are not equivalent linguistic measures. The Chinese-size storage/archive/SSR fixture has run (DESKTOP_PERFORMANCE.md); final English-sized and interactive release observations are recorded there separately. Current writing tabs load a metadata-only project index and one exact scene; review/export request full scene bodies. Index SQL still parses stored JSON titles, so this is a transfer/allocation improvement rather than constant database work. Review hashes reuse unchanged saved revisions. Writing tabs mount no hidden review controllers and do not hash the whole project; review tools load project sources, while the library hashes its selected scene. Library/review tool selectors change exact scene context only after pending drafts drain. Board cards and SVG are viewport-bounded, lists and connection controls page at 50 items, and keyboard navigation reveals retained focus. These bounds do not qualify dense overlapping boards or physical input.

Include multi-window detachment/return, simultaneous editing, failed handoffs, one-window close and whole-app quit fixtures. Measure typing, painting and memory with two or three windows using long Arabic/Chinese scenes; inactive windows must not repeatedly repaint or rehash an unchanged project.

Measure editor input responsiveness, local save latency, reopen time, search latency, memory, sync bytes, and AI memory/latency independently. Adjust practical performance budgets after M0 measurements on the user's devices. Model workload must not block the editor's UI thread or storage commit path.

## 25. Risks and responses

| Risk | Response |
|---|---|
| Multilingual WebView/editor defects | M0 actual-device gate; keep editor schema/core separate from shell. |
| Sync complexity or lost writing | Immutable revisions, conservative conflicts, transactional outbox, failure-injection tests. |
| PC is unavailable remotely | Local phone work continues; clear status and pending requests; no false cloud fallback. |
| Model exceeds hardware or performs poorly | Benchmark after inventory; smaller/scoped tasks or disable AI; no hardware purchase assumption. |
| Grammar/synonym gaps | Per-language evaluation, visible capability labels, custom dictionary/glossary, manual alternatives. |
| Invented script renders differently | Versioned maps, asset checksums, explicit font/direction tests, transliteration export. |
| Hosted networking terms change | Keep the application protocol transport-independent; preserve offline use and archives. |
| Android background restrictions | Durable PC jobs, refresh-on-open, no promise of instant background delivery. |
| Dependency licensing/paywalls | Audit actual packages and data packs; avoid Pro/cloud features; retain attribution. |
| Unbounded scope | Enforce milestones and explicit deferred features; keep a useful non-AI application. |

## 26. First implementation session

Start with the included read-only `tools/check_environment.sh`. It prints the executing machine's OS/hardware summary and detects development-tool presence. It does not install packages, call a network service, read manuscripts, print account names/IP addresses, or change configuration. Optional tools may be missing; that is a reported condition, not a reason for the script to install them.

Review its output before sharing it. Then record the actual Fedora edition and development environment in `PROJECT_STATE.md`, verify the current setup instructions for that edition, install only the agreed prerequisites, and create the smallest cross-device editor prototype.

The first prototype is one local project and one scene, with English/Arabic/Chinese input, explicit direction controls, save/reopen, and undo. A user-supplied custom-font sample joins that test when available. It contains no AI, public server, sync engine, account system, or large planning board.

**First implementation issue: NW-001 — Inventory the real devices and validate the development path.** The next issue is the cross-device editor/database spike. No additional product questionnaire is needed to begin.

## 27. Sources and verification notes

External capability claims below were checked against these official project/vendor/standards pages on October 2, 2026. A live documentation page is not proof of actual compatibility on the user's hardware. Architecture choices and acceptance criteria elsewhere in this document are proposed design decisions.

- **[S1] Tauri, prerequisites:** https://v2.tauri.app/start/prerequisites/ — Fedora and Android build prerequisites.
- **[S2] Tauri, SQL plugin:** https://v2.tauri.app/plugin/sql/ — SQLx-backed SQLite integration and platform support.
- **[S3] Tiptap, editor overview:** https://tiptap.dev/docs/editor/getting-started/overview — Open-source core versus paid extensions.
- **[S4] React Flow, quick start:** https://reactflow.dev/learn — Candidate node-based board library.
- **[S5] llama.cpp, server documentation:** https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md — Local inference interface and health status.
- **[S6] Tailscale, pricing:** https://tailscale.com/pricing — Current Personal plan and non-commercial conditions.
- **[S7] Tailscale, Serve:** https://tailscale.com/docs/features/tailscale-serve — Private HTTPS, localhost proxying, and distinction from Funnel.
- **[S8] W3C, structural markup and RTL text:** https://www.w3.org/International/questions/qa-html-dir — Directional markup guidance.
- **[S9] Unicode, private-use FAQ:** https://www.unicode.org/faq/private_use.html — Private-use mappings, fonts, and property limitations.
- **[S10] LanguageTool, local HTTP server:** https://dev.languagetool.org/http-server — Local rule-based service, excluding cloud AI rules.
- **[S11] LanguageTool, supported languages:** https://dev.languagetool.org/languages — Uneven coverage; displayed table dated March 27, 2025.
- **[S12] Open Multilingual Wordnet, data overview:** https://omwn.org/omw1.html — Downloadable lexical resources and differing licenses.
- **[S13] Android Developers, Doze and App Standby:** https://developer.android.com/training/monitoring-device-state/doze-standby — Background execution/network limitations.
- **[S14] Tailscale, DERP servers:** https://tailscale.com/docs/reference/derp-servers — Relay behavior and encrypted traffic.
- **[S15] Tailscale, privacy policy:** https://tailscale.com/privacy-policy — Networking metadata and separate gateway-data treatment.
- **[S16] SQLite, backup API:** https://www.sqlite.org/backup.html — Consistent database snapshots.

## Desktop story workflows accepted October 6

The author selected all six additions: durable what-if branches; continuous whole-book writing; explicit authored ripple previews; character cuts; reversible rich paragraph alternatives; and reader rehearsal with future reveal boundaries. They use the app-owned native writer and immutable revision/CAS storage. Android remains deferred. Implementation and current verification are tracked in PROJECT_STATE.md and STORY_WORKFLOWS.md; physical/external gates remain separate from feature completion.

## Writing desk UX accepted October 6

The author approved all eight additions after the Writing desk preview: hold-to-peek references/outline; same-window navigation trails; reversible whole-book folding; tab/binder drag into panes with existing detach/return; writing width and overlay docks; saved-revision paragraph comparison; eight bounded formatting-command sequences; and interface mirroring independent of manuscript direction.

Source implementation and exact-package/user-folder qualification are complete. Physical input acceptance remains separate from automated qualification. WRITING_DESK.md records controls and limits. Draft trail restoration is exact-anchor-bound; whole-book restoration covers reading/filter state. Folding drains saves before parking Undo. Comparison is by paragraph position, with insertion-shift limits. Sequences acknowledge/save each step, stop on failure and never record text or clipboard contents. Android and external physical gates remain deferred or unverified as previously recorded.


## Compact writing page accepted October 6

Remove the marked writing header and bottom strips. Keep one formatting toolbar and expose occasional actions through its menu. Hide a singleton tab strip, retain multiple-tab controls, and let the author close and reopen the left outline. Fill pane uses the actual available text width without the old centered side columns or bitmap enlargement. Current-host 1920×1080 release and installed-copy probes verify this behavior; physical interaction remains separate.

## Everyday writer workflow delivered October 6

Following the author’s report that scenes and everyday actions were difficult to find, the outline now leads with project selection, New chapter/New scene and Find scene. Search uses scene/chapter titles without full-manuscript loading, bounds the query to 1,000 characters and shows an empty state. Active-scene reveal scrolls only the tree. Creation drains pending saves, commits durable objects and opens the canonical scene route; failed drains retain the current draft.

The single writing toolbar exposes Outline, trail arrows, Notes, Reference, Split, Find and Focus. Format contains paragraph controls; More holds advanced Desk, Commands and Changes actions. A compact title, neutral palette and zero-height hidden singleton tab strip reduce chrome. Persistent Notes/Reference dock beside text in wide windows and overlay in narrow ones; temporary peeks remain overlays. Existing exact-text, revision, conflict and composition safeguards remain required.

This is a design response to the author’s task report, informed by primary product documentation and explicitly identified as an inference in [Writer workflow](WRITER_WORKFLOW.md). Extracted and installed automated slices passed; package details remain in PACKAGE_VALIDATION.md. No human usability acceptance, physical-input qualification or Android completion follows from those checks. Original milestones and external gates remain unchanged.


## Compact tool pages — October 6

Book, Planning, Review, Project and Alphabets have no separate top bars or centered page margins. Outline and More are integrated into their existing controls. More retains Back/Forward, Desk and Commands; Review/Project retain the scene selector and its save-before-switch guard. Page actions also remain available during loading/error states. Panel and form spacing is reduced; manuscript line spacing is unchanged. Review and Alphabets follow the neutral shell palette. Alphabets uses readable captions, wrapping symbol cells and a single-column narrow form, with the outline beside rather than over that form at 760 px.
Exact package and installed-copy evidence is in PACKAGE_VALIDATION.md. Android and human/physical/external acceptance remain separate.


## Chapters and shared universes — October 6

Use Edit beside a chapter to change its name and manuscript number. Changing the number moves it and renumbers the other chapters; Cancel leaves saved metadata unchanged. Saving drains pending writing first and preserves the mounted draft. Stale metadata is refused; any partially completed legacy name/order operation is reported rather than hidden.

The + beside the novel selector opens Library. Create a universe, a series, then a novel, or add an existing unassigned novel. Universe/series groups also appear in the selector. New novel creation commits its separate project, first chapter/scene and catalogue membership together. The universe/series line opens shared World notes; Reference shows those notes beside writing alongside the novel’s own scene references. Shared records have one saved source and never silently overwrite a novel’s planning or canon. Individual project exports/sync remain separately scoped; unavailable novels remain explicit, and whole-universe export/hierarchy merging is not included.

October 6: completed durable Chapter 0 numbering and atomic checked chapter edits. Corrected writer leading to use font size and removed added paragraph gaps; exact Unicode, Arabic baseline/ink, full-screen/narrow and long-book synthetic checks passed. Android remains deferred.
