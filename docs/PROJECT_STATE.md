# Unreleased core planning author-transition guards — October 14, 2026

Existing Save, checked Save and author-resolution transactions now refuse newly introduced/increased planning identity collisions, unsupported supplied variants, invalid owned applicability and wrongly typed custom values/links. Field-kind/record-kind changes and removal refuse while incompatible stored values remain. Sparse native omissions and exact unchanged historical partial/stale data remain compatible. Scene-link membership continues through existing scoped source/reference checks; date fields stay textual and choice membership stays at the authoring interface boundary.

Observed: complete locked/offline storage passed **204 tests, 0 failures** in the integrated source. The new **10 authoring regressions** first reproduced duplicate fact IDs, unsupported status and string arrays in link fields accepted by actual Save/checked Save/resolution, then passed with rollback checks for heads/history/projections/cursor/manuscript. Checked legacy archive/recovery and acknowledged resolution cases preserve exact multilingual values; unacknowledged malformed ancestors cannot authorize a new claim. Fixtures now supply stable IDs/owned field definitions and distinguish semantic coordinate/type refusals from scene-ownership refusals. No migration, history cleanup or production sync/save-hook change occurred.

---

# Unreleased confirmed chronology feasibility — October 14, 2026

Chronology and scene references now share a bounded read-only feasibility analysis of authored ranges, signed relative offsets and confirmed ordering constraints. Strict same-day conflicts, positive/strict cycles, unknown intermediary constraints and incompatible fixed bounds expose contributor identities instead of reporting satisfied/applicable. Relative relationships remain meaningful without absolute dates. Duplicate, malformed, incomplete or over-budget evidence stays unresolved; no dates, canon or histories are rewritten.

Arithmetic paths and derived ranges carry uncertainty. Fractional cancellation cannot manufacture an order/contradiction; both confident forward/backward paths never choose Before. Exact strict-zero conflicts and genuine small direct offsets remain supported. References use this same feasibility basis before showing scoped facts/knowledge, while preserving frozen historical values.

Observed: **18 chronology tests**, **8 reference tests**, and real GTK saved-reference/rehearsal/cut workflow **1 test / 2.70 s** passed offline/locked with fatal criticals. Regression fixtures reproduced strict same-day satisfaction and scoped knowledge Applies inside an infeasible component before the fix. Further tests cover negative signed paths, relative cycles, fractional roundtrip, tiny effects beside large offsets, malformed/duplicate identities and work limits. No author data, network, dependency or system change occurred. These are unreleased source results.

---

# Unreleased original/alphabet manuscript export — October 14, 2026

Project → Library → Export alphabet manuscript offers the original saved rich manuscript with immutable mapping metadata, optionally accompanied by a derived author-rule transliteration. The separate `.alphabet-manuscript.json` preserves exact source revisions, scene assignments, historical mapping versions, rich marks/paragraph metadata and original titles. Only nonempty stored scalar transliterations apply, once; unmapped text stays literal. Font/dictionary bytes and full revision history remain outside this manuscript copy. Word export remains separate.

A single checked read transaction captures the manuscript and sole alphabet/translation heads. It refuses scene/deletion/hierarchy/order conflicts, invalid source identities/checksums, foreign assignments, unsupported metadata or historical mapping reinterpretation. Complete retained alphabet verification is bounded to 4,096 revisions/64 MiB. Captured bodies, derived expansion and final serialization are bounded before excessive allocation. Parsing/encoding runs off the GTK thread; closed, changed-project or stale-generation results write nothing. Delivery uses the existing private atomic local-file writer.

Observed: **6 storage transaction tests**, **4 native pure tests** and targeted real GTK **1 test / 1.08 s** passed offline/locked. The action fixture runs actual option/draft decisions, RPC, background encoding and local file delivery with synthetic destinations, including failed writes and stale/closed delivery. The platform chooser itself is not automated. No dependency, model/font download, system change, personal store or network listener was used. Source results do not qualify published packages.

---

# Unreleased recorded formatting sequences — October 14, 2026

Command sequences provides eight named slots, at most 32 allowlisted formatting/spacing/paragraph-language/direction steps, checked settings saves and optional explicitly mapped shortcuts. Recording collects only accepted editor acknowledgements and never saves manuscript edits. Explicit Play and save each step uses normal manual draft decisions, then awaits each command and scene Save before advancing; failure retains earlier saved steps, the current draft and Undo. Logical input gates cover original queued clipboard/IME/tool permits, toolbar/navigation/close/transfer, and ordinary Save/Discard callbacks while allowing only exact author-choice drain and executor Save capabilities.

Cached recordings retain their registered owner for settings and Save/Discard/Cancel decisions. Cancel preserves recording/drafts through close, project switch and new-novel creation; refused clear stops teardown. Recovery refresh preserves a live recording/new draft instead of stopping its worker. Detached panes keep their runtime and Undo. Added shortcut slots default disabled and preserve prior remaps/explicit clears.

Observed: locked offline workspace excluding retained Tauri passed **431 tests, 0 failures, 4 ignored** (native **163**, storage **172**). Complete real GTK passed **1 test / 59.54 s**; the final sequence section including creation/recovery guards passed **1 test / 6.11 s**. Headless actual companion settings/CAS bridge passed **1 test / 0.09 s**. Sequence settings/storage bounds, rich Undo, failed per-step Save, original queued clipboard/IM/pointer signals, cached tool source guards, manual lifecycle and transfer are exercised. Synthetic input events qualify installed callbacks, not physical IME. Inspected captures: `work/notebook-command-sequences` and `work/notebook-sequence-recording`. No dependency, migration, model/resource download, system change or listener was introduced. Published packages remain separate from this source slice.

---

# Unreleased alphabet, private font and licensed pack controls — October 14, 2026

Alphabets now provides immutable saved mapping versions, explicit scene assignment, literal symbol insertion, glossary/notes and manual draft choices. Ordinary committed typing alone uses the saved scalar mapping; paste, drop, palette insertion, replacement, stored writing and Undo remain literal. New writing sessions capture checked alphabet history and saved scene identity, then reread the manuscript before opening. Cached sessions retain their profile and Undo until explicit close/reopen after manual Save or Discard. Conflicting/unavailable resources retain literal/default-font writing capability with a visible notice.

Licensed fonts & dictionaries previews selected local TTF/OTF or supported pack JSON, requires explicit source/license/transfer permission and Save, and checks schema, identity, SFNT bounds/family and checksums. Font Save stores the licensed asset before publishing a new immutable alphabet version; scene assignment stays explicit. Verified fonts stage only into each private editor directory (0700 directory / 0600 file). Registration failure retries default-font open only before input or explicit Discard. Historical pack IDs/versions are enforced atomically through both save APIs, resolution, archive validation/import and sync; unchanged legacy records and exact repeated versions remain compatible.

Observed: locked offline workspace excluding retained Tauri passed **394 tests, 0 failures, 3 ignored**; storage passed **166 tests** and native units **132 tests**. Complete real GTK passed **1 test / 52.63 s**. Targeted palette, editor mapping/private-font/fallback and licensed imports passed; the additional real-worker headless bridge passed **1 test / 0.56 s**. Synthetic palette and font-preview captures were inspected; non-test build is warning-free. Testing fixed stale fixture response shapes and a Discard completion race by waiting for viewport acknowledgement before the next navigation capture. No dependency/model/font download, system font install, migration, personal-store access or external listener occurred. These are unreleased source results; physical IME, supplied-font shaping and published packages remain separate.

---

# Unreleased review decisions and translation preview — October 14, 2026

Review decisions now retains local-provider proposals and explicit author choices. Apply validates every ordered saved source atomically, checks exact Unicode evidence/scalar boundaries and replaces only the captured paragraph as one unsaved undoable edit. Decision records use checked metadata/source dependencies; failure after Apply visibly retains the draft/Undo and states that the decision was not accepted. Proposals never become manuscript or canon automatically. Existing Local review and New translation draft routes use the dedicated controls.

Translation preview captures immutable exact saved sources, supports literal manual draft editing and optional explicitly requested local generation, and preserves old source revisions. Optional review/translation work does not count as saving or block normal editing/draft confirmation. Accepted release cancels it; cancelled departures preserve it, and detached pages receive callbacks through their current owner after origin closure. Choice dialogs now hide/destroy after a response and honor the parent focus policy; testing reproduced lingering old dialogs before this fix.

Observed: **85 native pure tests passed** with the desktop test ignored; the complete real GTK workflow passed **1 test / 45.23 s** and final targeted review host passed **1 test / 3.97 s** with fatal criticals and disposable synthetic stores. The host uses test-only delayed provider replies and actual stdio/editor workers; this qualifies cancellation/context/author decisions, not an installed model or provider accuracy. No service, model, dependency or personal writing was accessed. These remain unreleased source changes.

---

# Unreleased checked local asset read — October 13, 2026

The private stdio companion now exposes `get_asset {projectId, assetId}` through the existing bounded checksum-verifying single-asset storage read. It returns one project-scoped asset or null without writing history. This supplies the missing read bridge for forthcoming private font staging; alphabet/font UI integration remains separately qualified. The focused companion regression passed **1 test**, covering exact bytes, foreign/missing scope, missing arguments and unchanged revision cursor. Generated contracts/check and whitespace pass. No new dependency, listener or asset is added to Git.

---

# Unreleased authored fictional-time validation — October 13, 2026

New or changed explicit event times now validate supported variants, finite numbers, ordered interval/bounds ranges and distinct retained relative anchors. Negative and fractional fictional dates remain valid. Unchanged historical malformed or absent times remain exact; acknowledged recovery versions retain the existing compatibility policy. This fixes the reproduced Save of a reversed interval that native chronology displayed as Days 10–1. It does not infer chronology or add complete planning core-shape/feasibility enforcement.

Observed: the regression failed before the fix; **all 156 storage tests passed** offline and locked afterward. It checks new/changed refusals without revisions/projection changes, valid numeric dates, unchanged historical times, acknowledged/unacknowledged recovery and retained relative-dependent deletion refusal. No migration, dependency or UI behavior change was needed. Published binaries remain separate.

---

# Unreleased scene writing and deletion recovery — October 13, 2026

Project → History & recovery → Scene writing and deletion recovery lists the entire novel including deleted scenes. Checked previews retain exact rich writing, every writing/deletion leaf and placement dependencies. The author explicitly reviews Keep writing or Keep deleted after all-window draft decisions; stale or incomplete captures refuse the choice. A sole deleted version can be explicitly restored. Concurrent deletions now resolve using actual leaves while retaining projected-head CAS. Every earlier revision remains in history and archives.

After recovery, only clean idle scene caches are evicted for explicit reopening. New typing after draft approval retains its old parent, exact draft and Undo with a changed-version notice; recovery never reloads over it. Detached recovery pages continue after origin closure.

Observed: the locked offline Rust workspace excluding retained Tauri passed **316 tests, 0 failures, 2 ignored**. The complete real GTK/worker workflow passed **1 test / 41.03 s**, and the targeted scene recovery workflow passed **1 test / 2.45 s**. Five storage recovery regressions and one companion RPC regression passed, including archive/history retention, corruption/rollback, all-head/placement guards and explicit null deletion. Generated contract drift and whitespace checks passed. No dependency install, migration, personal-store access or external listener occurred. These remain unreleased source changes; published packages and physical/private-transport qualification are separate.

---

# Unreleased temporary peeks and safe preference fallback — October 13, 2026

On a writing page, hold F6 for saved authored references or F7 for a read-only paragraph comparison of the frozen acknowledged editor draft against a chosen saved version. Release, focus loss, composition, navigation and ownership transfer cancel the hold and reject late replies. Comparisons show unsaved status explicitly and never save or apply writing. An ordinary page chooses the saved comparison source; excerpts and paragraph pages are bounded.

Optional shortcuts/writing-view settings now load through unambiguous project-data reads. Conflicting or unavailable settings visibly fall back to built-in runtime defaults so editing remains usable; their author forms refuse Save until recovery. Initial/no-project shortcuts work. Normal status updates retain the fallback notice.

Observed: **64 native pure tests passed** with the desktop test ignored. Targeted real GTK peek, shortcuts/conflicted-preferences and writing-view workflows each passed **1 test** using actual debug workers and synthetic stores. No manuscript revision was created by holding, choosing a comparison or reading conflicted settings. This is unreleased source qualification; published binaries and physical input remain separate.

---

# Unreleased authored knowledge and reader timeline — October 13, 2026

Story now exposes dedicated Character knowledge and Reader learns timeline pages. Knowledge captures exact frozen facts, author sources, explicit interpretations and scene/event scopes; approved knows requires approved canon. Reader reveals retain authored scene position independently of occurrence dates. Both pages use atomic saved planning/index/details reads, manual draft decisions, checked dependencies and detached ownership. Missing or deleted newly referenced scenes refuse Save while preserving the form; unchanged historical claims remain retained.

Observed: all **60 native pure tests passed** with the desktop test ignored. The real GTK knowledge section passed **1 test** with fatal criticals and actual workers, covering valid authoring, exact multilingual snapshots, stale canon/scopes, explicit reader ordering, concurrent Save refusal, deleted source dependencies and detached ownership. Character knowledge capture was visually inspected. This is unreleased source qualification; published binaries, physical input and private transport remain separate.

---

# Unreleased native writing and planning continuation — October 13, 2026

The native notebook now exposes checked scene movement/reordering; reader rehearsal; canonical character-cut navigation; pinned/following saved scene references; local dictionary/sense/synonym lookup; labelled multilingual board connections; exact Back/Forward reading locations; saved writing width/interface direction and keyboard shortcuts; explicit fact/event-time ripple preview; and private-peer pairing, approval, revocation and manual-sync controls. Each uses existing immutable project history and manual author decisions. Reading projections bind planning, scene details and manuscript order in one body-free transaction and refuse conflicting or deferred order/data sources. Character cuts use authored links, never manuscript names or inferred presence.

Recovery adds checked chapter/scene placement previews, placement-first guidance, explicit deferred order replay and current-membership acknowledgement. Project-data recovery can explicitly prepare and review a combined candidate retaining all immutable paragraph versions/originals, translation sources and alphabet definitions, with mutable fields chosen from a retained base. It saves only after review against the original full head set. Other kinds retain checked whole-version choices; branch lineage remains inspection only.

Desktop testing found and fixed a double-allocation resize loop and a missed initial editor reflow. The actual drawing area now gets one final pixel allocation and the worker is refreshed after opening. View changes preserve exact rich writing, paragraph direction, selection and scene revisions. Board validation/remapping now distinguishes card identities from their scene references, including colliding IDs, archives, branches and sync. The additive atomic review-source checker verifies all saved source revisions/hashes without writes; author review/translation decision pages are still being integrated separately.

Observed: locked offline workspace excluding the retained Tauri member passed **296 tests, 0 failures, 2 opt-in tests ignored**; the complete real GTK/worker workflow passed **1 test**, with fatal criticals and disposable synthetic stores. Storage alone passed **150 tests**. Forty cached Rust serde wire shapes, generated contract drift and whitespace checks passed. Actual bounded RTL writing and labelled multilingual board captures were inspected. The SHA-256 library is an additional direct native dependency on the same already pinned/cached version used by the workers; no dependency was installed. No migration, model download, persistent service, personal-store access or external private listener activation occurred.

These are source changes after 0.4.0. Published binaries and physical/clean-host/private-Tailscale qualification remain separate. Formatting sequences remain the next integration slice; their delegated pure tests do not qualify the running notebook.

---

# Unreleased checked organization and native data validation — October 13, 2026

Storage now exposes a transaction-captured scene arrangement snapshot and a checked move/reorder operation. It compares scene/content, chapter memberships/orders and metadata heads before writing; saved scene text and revisions remain unchanged. Replication applies related placement/order projections in recorded sequence, deferring chapter orders with unresolved placement conflicts. New metadata recovery snapshots capture all heads and ordering dependencies. Explicit resolution can replay a retained chapter order or acknowledge the exact captured current membership; it never silently chooses placement or replays an order after another decision.

Native settings validate bounded shortcut bindings and interface direction/writing width. New or changed authored knowledge/reveal entries are checked against retained characters, facts, scopes and exact fact snapshots; approved knowledge requires approved canon. Compatibility reads/imports preserve unchanged stale and partial historical entries. Conflict candidates can reuse only verified acknowledged-head claims; a new merged claim must satisfy authoring rules. No database migration, dependency installation or external service was added.

Observed: all **131 storage tests passed** offline and locked, including six checked scene arrangement tests, three metadata recovery tests, native settings tests and planning transaction/archive/conflict regressions. All **69 companion tests passed**, with the opt-in OS-keyring test ignored. Generated contract drift, synthetic RPC drift/borrowed-field refusal and cached Rust serde fixtures passed. Native interfaces for these backend capabilities are being qualified separately; these checks do not qualify release packages or physical input.

---

# Unreleased project-data conflict page — October 13, 2026

Project → History & recovery → Project data conflicts shows retained heads and checked full-version previews for all supported project-data kinds. The author explicitly chooses a saved payload against the complete captured head set. Resolution checks unsaved pages across the novel's windows first; stale previews refuse the decision. Every earlier revision remains in history, and detached pages route to their current window.

Immutable-field guards remain enforced: choosing a whole version that drops required immutable definitions/versions can be refused. This page does not automatically merge them; a combined author resolution remains a separate control gap. Branch lineage is inspection only. Scene placement/chapter metadata conflicts are also separate from this project-data page.

Observed: native unit discovery passed **7 tests, 1 ignored**, and the explicit complete GTK workflow passed **1 test** using real debug workers and synthetic two-store revision sync. New cases cover exact multilingual checked previews, stale full-head refusal, dirty manuscript/settings Cancel and Save, retained history, immutable choice refusal, detachment and continued resolution after the original window closes. No network listener or personal store was used. Published packages and physical/external qualification remain separate.

---

# Unreleased paragraph alternatives — October 13, 2026

Write → Other pages → Paragraph alternatives keeps exact rich originals and named saved versions. The author chooses a paragraph number, captures it explicitly, edits a version and saves it as a new immutable alternative. Comparing and selecting versions never changes writing. Apply replaces exactly the captured paragraph as one undoable unsaved scene edit; manuscript Save remains manual. A changed document hash or saved scene revision refuses the operation. Explicit rebinding after saving the current scene updates only the group's guarded anchor; its original and saved versions remain intact.

Alternative forms use normal Save / Discard / Cancel, composition and detachable-page ownership. Failed metadata writes retain the candidate and, if Apply already changed writing, explicitly label the retained scene draft and available Undo. Capture/rebind/apply metadata writes check the saved source revision atomically as a dependency. Checked first project-data writes now refuse retained concurrent roots even when no projection exists; a synthetic two-store sync regression reproduced and covers that case.

Observed: offline native build passed; editor/native unit suites passed **19 tests**, including two rich Unicode version-edit tests and one explicit paragraph-capture test; checked project-data regression passed **1 test**; the complete opt-in GTK workflow passed **1 test** with debug workers, including alternatives, previous chapter and notebook workflows. GTK checks include exact rich originals, manual-save inactivity, one-step Undo, stale document/revision refusal, explicit rebinding, concurrent metadata failure after Apply, retained failed drafts, composition, detachment and saving after the original window closes. Physical input and release packages remain separate. Parallel work on the other audited gaps is not included in this slice's qualification.

---

# Unreleased native chapter editing — October 13, 2026

The source after 0.4.0 adds Write → Add & arrange → Edit chapter. A selected chapter or scene opens a title/number form with ordinary manual Save / Discard / Cancel, composition guards and detachable draft ownership. Chapter rename uses the same checked form. Saving a number moves the chapter and renumbers the manuscript, including Chapter 0 and returning its first chapter to 1. Scene identities, exact writing and saved scene revisions remain unchanged.

The additive `chapter_edit_snapshot` RPC captures title, order, numbering and metadata heads in one read transaction with project ownership/conflict/checksum checks. Saving reuses the existing atomic `edit_chapter` operation. Concurrent title/order/numbering changes refuse the stale form and retain its entered values. No dependency, schema migration, service or network setting was added.

Observed checks: locked offline native build passed; Rust workspace excluding the retained Tauri UI passed **202 tests, 0 failed, 2 opt-in tests ignored**; the explicit GTK workflow passed **1 test** with fatal criticals enabled and actual debug workers. The expanded fixture covers guarded opening from an unsaved scene, manual inactivity, Chapter 0/1, reordering, invalid fields, Save/Discard/Cancel, competing-window title/order edits, stale numbering, refused close, composition, detached saves and continued saving after the original window closes. Its actual multilingual page capture was inspected. Generated RPC contract drift and Git whitespace checks passed.

The retained Tauri member requires its built frontend assets; they are absent in this checkout. Its tests and TypeScript/contract Vitest checks were not rerun because Node dependencies are absent. Documentation now gives a native-only test command. Physical input/accessibility, release workers, packaging and installation were not qualified for this slice; the published 0.4.0 downloads remain unchanged.

---

# Current Rust desktop state — October 13, 2026

The active Rust desktop is now version 0.4.0 with the requested minimal notebook redesign: white pages, a left Write/Planning/Review/Project selector and collapsible menus, a single detachable tab row, manual saving and Save / Discard / Cancel before leaving edited pages. Ordinary navigation replaces the active tab; middle click opens an independent window. Drag-out transfers the existing draft/Undo. Additional windows replace within-window split editing, and closing one window leaves the others working. Tool pages occupy tabs. See [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md).

Version 0.4.0 adds four optional tools under the existing sidebar. **Cutting-room floor** retains explicit selected-text scraps and recovers removed text from immutable saved scene revisions; plain-text copies can be dragged into writing windows or appended to a chosen scene as an unsaved, undoable edit. **Read like a stranger** makes a clean saved manuscript with Bored/Confused/Curious selection markers. **Passages to revisit** collects unresolved reactions and scene notes, with saved rewrite counts based on changes to existing text. **Return bookmarks** retain a short next-step reminder and checked cursor location; reopening the novel resumes its latest bookmark when that scene remains available.

Notebook aids use new `notebookTools` project data within the existing revision format, with compare-and-swap, conflict-head refusal, scene/revision ownership checks and archive/branch/private-sync remapping. They add no external service. Bookmark and reaction jumps refuse stale coordinates. Explicit note/mark/bookmark actions save those aids; manuscript typing still requires manual Save. Closing or changing novels waits for pending aid saves. Saved scraps are plain text; unsaved cuts remain in Undo, and large rewritten spans may conservatively retain extra old words.

The public source repository is [saad-musbeh/novel-workspace-notebook](https://github.com/saad-musbeh/novel-workspace-notebook), licensed Apache-2.0 for original application code, with upstream terms retained. The 0.4.0 Linux x86_64 prerelease includes a portable archive and RPM. It is a desktop build, not an Android APK. Observed checks: 242 Rust workspace tests passed (two opt-in tests ignored); explicit GTK workflows passed with debug and packaged release workers; 16 contract tests and TypeScript checking passed. Package checks and published hashes accompany the test release.

The existing schema-3 companion/storage/DOCX format and exact Unicode editor remain in use. No dependency or database migration was added.

Version 0.3.4 removes the permanent sidebar window hint and fixes new-novel startup. Opening-page, Project-menu and Library actions save the novel, first chapter and blank scene in one transaction, then open the Write editor. Unsaved drafts are checked before creation, so Cancel creates nothing. Name prompts close immediately, and project switches clear the previous outline while metadata loads. Layout-only editor replies retain the current document so screen refreshes do not lose draft captures or mislabel saved writing. The additive create_novel RPC reuses the existing universe manuscript initializer; the bare create_project API and database schema are unchanged.

Version 0.3.3 fixes chapter activation: clicking a chapter opens its first scene; an empty Prologue or other chapter shows its name and Start writing, which creates a blank first scene in that chapter. Empty chapter titles remain searchable. The native regression covers exact multilingual manual saving, cancelled navigation/creation, repeated Start actions, stale metadata, late replies, middle clicking and detached chapter pages. Closed windows ignore late catalogue/index callbacks.

Version 0.3.2 owns the light backgrounds of dropdown menus, popovers, notebook content, text-view text nodes, number fields, lists and scrollbars. Selections use pale grey with dark text. This prevents dark desktop themes from appearing behind the notebook's dark text; desktop theme settings are unchanged. The GTK regression reproduces the previous dark dropdown under Breeze-Dark and checks readable contrast under Breeze-Dark, Adwaita dark and Breeze light, including inactive and selected states.

This is a new interface implementation with documented parity gaps. Specialized reader rehearsal/ripple, paragraph alternatives, shortcut/sequence customization, automated translation/review decision controls and complete private-peer setup are not yet exposed natively. Their data/backend remain intact. See [RUST_PORT.md](RUST_PORT.md) for the exact coverage map, [SETUP.md](SETUP.md) for the new commands, and [TEST_RESULTS.md](TEST_RESULTS.md) for this runtime's evidence.

Version 0.3.1 fixes the reported active-scene focus crash by releasing the active-editor borrow before GTK can synchronously call its focus handler. The regression reproduced the original abort and now checks direct reopening, outline activation and repeated activation of an unsaved draft without replacing its editor or text. Locked offline Rust checks passed 230 tests (2 opt-in tests ignored); the extended notebook GTK workflow passed separately. The 0.3.0 editor comparison matched 147 operations; that editor implementation is unchanged. Release payload and publication observations for the current build are in the external 0.3.4 release manifest and work logs. Details are in TEST_RESULTS.md and PACKAGE_VALIDATION.md.

The separate Novel Workspace — Notebook desktop/app-menu shortcut uses the user-folder Rust app at `~/.local/opt/novel-workspace-notebook`, with the generated notebook-and-pencil icon. The older Novel Workspace shortcut remains available. This repair changes application files only; personal stores, the system RPM, models, services and network policy are untouched. Android remains deferred. Engine checkpoint resume is tested, but GTK does not automatically park or recover editor sessions; there are at most 16 live scenes.

The historical state below describes the retained React/Tauri implementation. Its feature-completion claims, timing and installed-payload hashes do not qualify the Rust desktop.

---

# Project state

Updated October 10, 2026. Per-novel Word export styles are available on Project → Word export styles, with Default layout, Standard manuscript and Compact reading presets. Paper, margins, fonts, sizes, bold/italic and body spacing are saved as immutable revisions; export refuses stale or conflicting profiles and preserves original writing. WordCraft DOCX export, Chapter 0, compact leading, shared-universe library and optional translations remain available. The RPM is 11,045,448 bytes, SHA256 `7c010a2ba86f41c7525aded723c46cf833a5c71883cb8f112ce437af71bfd27a`. Main shortcuts select the verified user-folder payload at `~/.local/opt/novel-workspace/builds/7c010a2ba86f41c7`; prior `117a937a9da1fa55` and `c86f66d94e59c5e3` builds remain available. System RPM and personal writing were unchanged. Android stays deferred.

## Done

| Area | Implemented behavior |
|---|---|
| Writer | App-owned exact Unicode/rich-text model, Pango/Cairo full-paragraph Arabic/bidi shaping, viewport-only painting, visual navigation, Alt+X, missing-glyph notices and bounded independent Undo. LibreOffice app backend removed. |
| Typing priority (idea 1) | Bounded exact-input batching with separate Undo steps; queued intermediate layout can yield to final shaping. Background metadata/history and review hashing wait for input/composition. The author accepts the measured giant-paragraph performance; it no longer blocks desktop completion. |
| References (idea 2) | Per-draft, read-only scene references with pin/follow, linked people/places, facts, events, reveals and knowledge. No manuscript inference or editor remount while browsing. |
| Two timelines (idea 3) | Fictional occurrence time separate from explicitly authored reader reveals in manuscript order. Reordering scenes does not change story dates. |
| Character knowledge (idea 4) | Knows/believes/misunderstands with author sources and explicit scene/event scopes; exact fact snapshots and stale-canon labels. Unknown/conflicting timing stays unresolved. |
| Project time machine (idea 7) | Common-point scene/bible preview, historical hierarchy/branches, deleted-scene recovery and selected scene/whole-bible copies as new revisions. Epoch/sequence/revision identity and atomic head checks preserve concurrent/newer history. |
| Desktop workspace | Browser-style tabs, detach/return/empty-source closure, split drafts with independent scroll/Undo/save, compact binder, configurable keyboard/mouse shortcuts and spacing. Outer writing padding removed. |
| Storage / planning | Single-owner SQLite schema3, immutable revisions/CAS/conflicts, recovery snapshots, archives, exact exports, alphabets, dictionaries, linked story records, bounded boards, chronology and manual issues. |
| Assistance / private sync | Optional local revision-bound review with explicit author decisions; authenticated manual bounded revision/asset sync with OS-keyring tokens and private Tailscale HTTPS policy. Editing/saving needs no model or listener. |
| What-if branches | Separate bounded project/history copies, exact baseline comparison and explicit selected scene/whole-bible bring-back as new revisions. |
| Whole-book / character cuts | Native shared scroll in manuscript order, bounded lazy scene loading, independent saves/Undo and drained offscreen parking. Character cuts use explicit links and retain the same scene identities. |
| Ripple preview | Before/after fact and event-time changes with bounded authored dependencies; exact Apply, Cancel and stale/composition/navigation guards. |
| Paragraph alternatives | Immutable rich originals and saved versions; exact paragraph/head anchors, one-step Undo and failed metadata-save barriers. |
| Reader rehearsal | Authored reveals and setup/payoff notes through selected manuscript progress; future values hidden, missing/uncertain links labelled, no inferred reader psychology. |
| Writing desk UX | Packaged and user-folder verified: F6 peek, same-window draft/book trail, save-before-fold with parked Undo, tab/binder pane drag, writing width and responsive notes docks, F7 saved-paragraph comparison, eight × 32 formatting sequences and independent interface mirroring. Current-host full screen also verified; see WRITING_DESK.md. |
| Scene-first navigation | Search scene/chapter titles, create and open scenes from the outline, reveal the active scene in its own scroller, and use visible everyday actions. Wide Notes/Reference docks do not cover writing. Automated workflows passed; user usability acceptance remains separate. |
| Other-page layout | Full-width Book/Planning/Review/Project/Alphabets, compact panels and integrated controls without separate top bars. Neutral Review/Alphabets palette, readable alphabet captions and bounded narrow form/table. Exact packaged and installed workflows passed. |
| Chapters and novel library | Edit chapter names and numbers in the outline. Universe/series catalogue uses immutable revisions and CAS; creation of a novel, first chapter/scene and membership is atomic. Existing novels retain their text/history. Shared universe notes are read beside any novel without cloning or automatically merging canon. |
| Word manuscript export | Saved transaction snapshot, exact Unicode/rich marks, language/direction/spacing, editable title/chapter/scene styles, Chapter 0 and explicit conflict refusal. Real packaged one- and two-window exports and LibreOffice text roundtrip passed; see DOCX_EXPORT.md. |
| Word export styles | Strict per-novel profiles, four style definitions, Letter/A4 and equal margins, body-only spacing override, saved presets, CAS/local draft retention, archive/sync and explicit Recovery. Stale-profile refusal/reload and actual DOCX/profile bytes passed in the final RPM. |
| Packaging | Compact writing page and full-width renderer passed extracted and installed-copy checks. Main shortcuts select the verified user-folder build; system RPM remains older. See PACKAGE_VALIDATION.md. |

Frontend verification passed **589 tests in 55 files**. The locked offline Rust workspace passed **218 tests**, with one opt-in keyring test ignored. Generated contracts, TypeScript/Vite, debug and production builds passed. Final-RPM style export passed 22 checks; two-window dirty-draft export passed 16. The extracted/shortcut-selected 1,000-scene writing workflows passed 23/24 checks and book workflows 16/17. Installed translations passed 22 checks. Independent profile-aware OOXML, exact LibreOffice text roundtrip, both rendered pages, package/resources/notices, public-key-only signature and scoped egress checks passed. The prior native Python editor baseline remains historical evidence. See TEST_RESULTS.md and PACKAGE_VALIDATION.md.

## Remaining external checks

The story workflows and all eight Writing desk additions are implemented in the verified package. The byte-matched published user-folder copy also passed offline workspace and whole-book checks. The author accepts approximately 72 ms processing for the 54,000-character single-paragraph stress case. Further optimization of that extreme case is optional; ordinary typing responsiveness and external/physical acceptance still matter. No physical check is being claimed while the author is remote.

| Gate | State |
|---|---|
| Giant-paragraph processing | Accepted by the author on October 6: about 72 ms for one 54,000-character mixed-script paragraph. The burst improved from about 310 ms to 137 ms with 260 Undo steps. Further optimization is optional, not a desktop release blocker. |
| Fresh Arabic/kasra/يس/mixed-direction/keyboard/selection/Undo/clipboard/full-screen | Needs a fresh physical input check. Automated current-host full screen passed at 1920×1080. Earlier LibreOffice tests are historical. |
| System-wide RPM replacement | Deferred while remote; the system RPM remains older. The byte-matched final user-folder payload is verified and selected by the main shortcuts. |
| Word interoperability | Exact LibreOffice DOCX text roundtrip passed. Microsoft Word and stock GTK chooser interaction were not exercised. Font glyph coverage depends on the receiving application. |
| Chinese IME | No Chinese input method available. Exact synthetic Unicode tests exist. |
| Invented alphabet | No permitted author font/sample supplied. Private font registration is implemented; glyph coverage is not promised. |
| Real second-PC/private Tailscale | Only one PC available. Loopback/TLS/conflict fixtures passed; actual remote routing, interruption and unattended unlock remain unverified. |
| Model quality | Qwen3 remains experimental: false positives on all four clean controls. User declined Qwen3.5 comparison. |
| Proofreading quality | LanguageTool English/Arabic pilot passed; Traditional Chinese unavailable. Broad quality remains unqualified. |
| Clean host, power-loss/low-disk, accessibility | Not established by current-host synthetic checks. |

## Not started / deferred

Android shell, phone input/sync and remote jobs are not started under this scope. No SDK, phone client or phone inference runtime is included.

## Known limits

History points follow local recorded sequence, not synchronized author timestamps. Old missing placement is labelled; no personal-data backfill occurred. Recovery selects one scene or the whole story bible, not individual bible records, global hierarchy rewind or whole-database replacement. Current conflicts must be resolved before recovery. History reconstruction is bounded to 20,000 heads/hierarchy events and 64 MiB per summary/replay; large previews page rows and load bodies lazily.

Assets use bounded verified SQLite BLOBs rather than external hash files. Archives load bounded complete data. History/same-disk backups are not automatically pruned and do not protect against disk loss. Copy portable archives to a separately chosen destination. Session Undo is bounded to 512 operations/about 64 MiB; trimmed entries are reported and persisted revisions remain separate. Manual sync pauses workspace input during its bounded pass. A source-window crash during transfer can lose session-only caret/Undo state; acknowledged SQL text remains preserved.

No cloud fallback, telemetry, automatic model download, persistent service, firewall or SELinux change was added. The separate system LibreOffice installation is untouched.


Universe catalogue references do not authorize or bundle other projects. Each novel retains independent planning, review, alphabets and sync authorization. A single-project archive contains only that project; a catalogue imported/synced before its novels shows unavailable references. Whole-universe export and hierarchy branch merging are not implemented. Universe catalogue conflicts can be reviewed and explicitly resolved through Recovery. Native exact-head catalogue history recovery exists; the current time-machine UI still offers scene/bible recovery.


## Optional translations — October 10, 2026

Write uses one original language. The visible Translate action adds optional, editable English, Arabic or Chinese drafts, with Simplified/Traditional Chinese variants. Only added languages have tabs. Drafts retain their exact saved source, author edits and older versions; changed original text shows “Translation may need updating” and never replaces translated wording. Compare views work in wide and narrow panes. Manual drafting does not need a local model.

The local-model translation route validates project/scene/revision ownership and exact saved text, uses bounded literal-loopback requests, and ignores cancelled/late results. Editing/saving remains local and independent of inference. The sceneTranslations data kind uses CAS and immutable source fields, survives complete archives/branches/sync with remapped source IDs, and rejects forged source text. Manuscript/Word exports still use the original text. See TRANSLATIONS.md for usage and limits.

589 frontend tests in 55 files, 218 locked offline Rust tests (one opt-in keyring test ignored), generated contracts, TypeScript/Vite and the production RPM passed. Packaged translation lifecycle and narrow-pane checks, a 1,000-scene whole-book check, resource equality and public-key-only signature verification passed. The shortcut-selected user-folder payload passed the same isolated translation workflow; no personal writing was opened. Local-model translation quality and physical input/IME remain unqualified.

The translation build is 11,045,448 bytes, SHA256 `117a937a9da1fa55d5d0d1a1961569b767304d7406bf1965fbd44520be386145`. The translation publication selected `$HOME/.local/opt/novel-workspace/builds/117a937a9da1fa55` through `current`; the latest selection is recorded at the top of this document; `$HOME/.local/opt/novel-workspace/builds/c86f66d94e59c5e3` remains available. System RPM was unchanged. Evidence is under `work/translations-*`.
