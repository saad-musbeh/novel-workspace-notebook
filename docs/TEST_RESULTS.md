# Native temporary peeks and preference conflicts — October 13, 2026

Locked offline native units passed **64 tests, 0 failures, 1 ignored**. Targeted GTK sections passed: peek **1 test / 2.51 s**, shortcuts including genuine two-store settings conflicts **1 test / 2.33 s**, writing view **1 test / 2.33 s**. Fatal criticals were enabled and actual worker binaries used disposable synthetic stores.

Peek cases verify actual installed key signals, release-before-reply, F6→F7 supersession and wrong-key release, focus/unmap/composition/pending refusal, frozen unsaved rich text, selection/focus/Undo preservation, chosen historical sources and bounded paragraph pagination, detached ownership and competing planning refusal. Preference cases verify explicit built-in fallback after conflicting custom heads, Ctrl+S/Ctrl+B availability, disabled author forms/no Save, exact rich multilingual text/head and no new revisions. Existing real width/mirroring behavior also passed. Logs: `/tmp/novel-native-peek-gui.log`, `/tmp/novel-native-preference-conflict-gui.log`, `/tmp/novel-native-peek-view-gui.log`, `/tmp/novel-native-peek-preference-unit.log`. No external listener or personal store was used.

---

# Native authored knowledge and reader timeline — October 13, 2026

Locked offline native units passed **60 tests, 0 failures, 1 ignored**, including eight knowledge/reader-order pure checks. The targeted real GTK knowledge workflow passed **1 test** in 2.76 seconds with fatal criticals, disposable synthetic stores and actual debug workers. It checks exact frozen rich multilingual fact values, author approval/source/interpretation semantics, scene/event ranges, explicit reader positions, stale planning and deleted-scope/reveal Save refusal retaining drafts, manual Save / Discard / Cancel and current detached ownership. No personal database or external listener was used. Capture `work/notebook-character-knowledge.png` was inspected. Logs: `/tmp/novel-native-knowledge-unit.log`, `/tmp/novel-native-knowledge-gui.log`. Published packages and physical input are not qualified by this source test.

---

# Native continuation integration — October 13, 2026 (unreleased source)

The locked offline Rust workspace excluding the retained Tauri UI passed **296 tests, 0 failures, 2 ignored**: 69 companion, 13 DOCX, 12 editor, 52 native and 150 storage. The complete opt-in GTK workflow separately passed **1 test** in 33.39 seconds with fatal criticals, actual debug workers and disposable synthetic stores. Source generation/contract drift and Git whitespace checks passed; the cached Rust serde fixture compiled and emitted **40 wire shapes**. Node/Vitest/Tauri frontend checks were not rerun because their dependencies/assets remain absent.

Targeted GTK sections also passed for checked reading projections, installed synthetic dictionary lookup, Pango board captions/connections, exact navigation, shortcuts, writing view, chapter/placement recovery, immutable conflict candidates, private-sync controls and ripple preview. The full workflow includes earlier novel creation, chapter, alternatives and notebook tools. Cases cover Save / Discard / Cancel, manual-save inactivity, stale heads/hashes, pending/composition guards, detached current ownership after origin closure, and exact rich multilingual document/revision preservation.

New storage regressions reproduce scene-card/scene-ID collisions and verify actual reference-only remapping through archive/reimport/branch/sync; malformed or foreign links refuse atomically. Atomic body-free reading snapshots refuse conflicting planning/scene metadata, manuscript/deletion/placement conflicts and deferred chapter orders; new unlisted scene additions remain readable. Seven review-source checks cover exact Unicode plaintext hashes, scoped ownership, stale/deleted/conflicted heads, schema/checksum corruption and a deterministic second-store WAL race without writes.

Real GUI attempts initially exposed a writing surface that allocated children twice, continually queuing contradictory width refreshes; a GtkFixed subclass now allocates the final size once. The writing worker also initially opened at 640 px after its allocation event had already fired; opening now refreshes to the actual viewport. Dedicated wide/narrow/RTL tests pass with worker width matching allocated pixels and caret/selection fitting that viewport. The screenshot helper explicitly settles pending GTK resize allocation before manual drawing. A pin fixture was corrected to use the explicit Pin checkbox when selecting the already active combo item (no changed signal), and a recovery assertion uses load_project_scene for chapter membership rather than load_scene's body-only Snapshot. Failed attempts are not counted as passes.

Inspected: `work/notebook-bounded-writing-rtl.png` and `work/native-board-labelled-connection.png`. Missing supplementary/private-use glyphs remain visibly reported while exact Unicode is preserved; no author font was supplied. Logs: `/tmp/novel-current-native-workspace.log`, `/tmp/novel-native-backend-regressions.log`, `/tmp/novel-native-complete-gui.log`, and targeted `/tmp/novel-native-*-gui.log`. Private-sync GUI tests never activate a listener, network probe/pairing or OS-keyring write. Existing synthetic loopback protocol tests remain local transport evidence. Release packages, installation, real private transport and physical IME/accessibility are not qualified by this source run.

---

# Checked scene organization, recovery dependencies and native validation — October 13, 2026

The locked offline storage suite passed **131 tests, 0 failures**. Arrangement fixtures cover ownership, stale scene/title/order/membership/numbering, bounds, tombstones, atomic rollback, reopening, archive and competing sync while preserving rich multilingual writing. Metadata recovery fixtures cover unresolved placements, stale dependency refusal, exact deferred single-head replay and explicit current-membership acknowledgement when a retained chapter order lists a now-foreign scene. Neither placement choice automatically applies chapter order.

Planning tests verify compatibility/author-transition separation, exact snapshots, approved knowledge, explicit scopes/sources, retained historical stale claims, transaction rollback, archive remapping and rejection of unacknowledged-ancestor claims in resolution. Shortcut and view settings tests cover strict bounds, reserved/duplicate chords, composition-safe supported bindings and fractional/unknown settings refusal. Historical branch/time-machine fixtures first author valid knowledge and then withdraw the fact, retaining their original stale-history assertions.

The companion suite passed **69 tests**, with the OS-keyring test ignored; synthetic loopback transport fixtures used disposable credentials and stores. Python contract drift and source-derived RPC shape checks passed; synthetic changed keys fail without overwriting generated output, and borrowed wire fields are refused. The cached Rust serde fixture compiled and emitted **37 wire shapes**, including checked arrangement and metadata snapshots. TypeScript/Vitest is not rerun because its dependencies remain absent. Native desktop controls and the complete GUI workflow are separate ongoing qualification.

---

# Native project-data conflict resolution — October 13, 2026

Native unit discovery passed **7 tests** with the GTK fixture ignored. The explicit full GTK workflow passed **1 test** with fatal criticals, debug workers and disposable synthetic stores. Its new resolver fixture creates genuine planning/writing-system conflicts by revision sync between two stores; checks exact checked previews, stale head refusal, all-window manuscript/form Save/Cancel, immutable resolution rejection without marker/head/payload changes, retained version history, branch-lineage inspection and detached routing after the original closes. No network listener is enabled by this workflow.

This qualifies whole-version project-data resolution only. An explicit combined candidate is still needed where neither whole version satisfies immutable retention constraints. It does not qualify metadata placement/order or deletion conflict controls, real private transport, release binaries or installation. Logs: `/tmp/novel-data-conflicts-{unit,gui}.log`.

---

# Native paragraph alternatives — October 13, 2026 (unreleased source)

- Offline desktop/editor/companion build: passed.
- Editor/native unit suites: **19 passed**, one opt-in GTK check ignored in ordinary discovery.
- Checked project-data concurrent-root regression: **1 passed**.
- Explicit complete GTK workflow: **1 passed**, with fatal criticals and real debug workers.
- RPC contract drift and whitespace checks: passed.

The GTK fixture stores a rich Arabic/Chinese/emoji heading from an explicitly selected paragraph, saves named versions without modifying the scene, applies an unsaved one-step replacement and checks exact Undo. It refuses stale document hashes and saved revisions, rebinds only through an explicit action, retains candidate metadata and undoable writing after a concurrent metadata save, guards navigation/close/composition, and saves detached alternatives after the original window closes. Saved originals retain their exact JSON and untouched text runs retain marks/attributes.

A new editor request accepts an optional checked blockIndex for read-only capture; default caret capture is unchanged. The new checked-data regression ingests two independent root metadata revisions in one batch, confirms no projection exists, refuses an unparented checked save and then resolves every head explicitly. Native source applies source revision dependencies in its capture/rebind/apply commits. No model, external listener, dependency download or personal store was used. The fixture capture is `work/notebook-paragraph-alternatives.png`; logs are `/tmp/novel-alternatives-{build,unit,gui}.log` and `/tmp/novel-checked-data-tests.log`.

---

# Native chapter form — October 13, 2026 (unreleased source)

- Locked offline debug desktop/companion/editor build: passed.
- Rust workspace excluding the retained Tauri UI: **202 passed, 0 failed, 2 ignored**.
- Explicit GTK workflow with `G_DEBUG=fatal-criticals`, temporary synthetic stores and real debug workers: **1 passed**.
- Generated RPC contract `--check` and Git whitespace validation: passed.
- Actual `work/notebook-edit-chapter.png`: inspected; the manual form is light and legible with Arabic/Chinese text.

Storage adds read-only/scoped snapshot and corruption-refusal tests. Existing chapter tests retain stale title/order/numbering, rollback, archives, sync and numbering-conflict coverage. The companion test now captures values through the new snapshot RPC before using the checked edit command. The GTK fixture adds Chapter 0/1 and reordering, no-save inactivity, exact manuscript preservation, invalid title/number refusal, Save/Discard/Cancel, two-window title/order conflicts, stale numbering, failed-close retention, composition barriers and detachable-form saving after the original window closes.

Initial fixture attempts failed because the form helper did not wait for an asynchronous manuscript save, and a corruption test attempted to update an immutable revision without bypassing its fixture-only trigger. Those fixtures were corrected; they are not counted as passes. Full-workspace compilation was blocked by absent retained Tauri frontend assets. The final 202 count excludes that UI member. No Node dependency installation or TypeScript/Vitest rerun was performed. No physical IME, release-worker/package or clean-install qualification is claimed for this slice.

Logs for this run: `/tmp/novel-chapter-build.log`, `/tmp/novel-chapter-tests.log` and `/tmp/novel-chapter-gui.log`. All writing and stores were synthetic. Personal stores and installed launchers were not opened.

---

# Notebook tools 0.4.0 — October 13, 2026

Four native notebook aids are implemented. Validation uses temporary synthetic manuscripts only.

- Locked offline Rust workspace: **242 passed, 0 failed, 2 opt-in tests ignored**.
- The explicit native GTK workflow with fatal criticals enabled: **1 passed with debug workers and 1 passed with packaged release workers**, including all four aids and the previous navigation/theme/chapter/new-novel tests.
- Storage includes **102 passing tests**, with notebook CAS/reopen/history, forged quote/offset and foreign-scene rejection, archive/branch remapping and sync/conflict rollback.
- Generated RPC contract: current; contract tests **16 passed**; TypeScript `--noEmit` passed.
- Dependency notice inventory: **227 packages**, no missing notice files.
- Actual GTK captures inspected: cutting-room floor, saved reader and passages to revisit remain white and legible with multilingual content.

The new GTK fixture verifies exact retained selection, saved-history cuts, unsaved append and Undo, Save/Discard navigation, return reminder persistence/reopening, stale-location refusal, reader cancellation/heading rejection, Unicode marks, detached reader ownership, resolving/adding reminders, pending-capture close durability and failed-save close refusal. Physical dragging/IME, accessibility and installation on a clean host remain separate acceptance checks.

---

# New novel startup and sidebar repair — October 10, 2026

Rust notebook 0.3.4 removes the permanent mouse/tab hint and fixes standalone novel creation. The previous New novel action created an empty bare project. The new create_novel RPC commits a project, Chapter 1, blank scene and revision/placement/order records together, then the interface selects Write and opens that scene. Unsaved pages are checked before creating anything. Opening-page and Library buttons expose the same action.

The locked offline workspace passed **232 tests, 2 ignored**, including two added storage tests for exact standalone edit/reopen/archive behavior, stale-save refusal, validation and injected late-transaction rollback with a clean retry. The generated-contract suite passed **16 tests**, and TypeScript checking passed. The additive NovelCreated wire shape is generated from Rust; create_project and universe wire behavior remain compatible.

The expanded opt-in GTK workflow passed (**1 test**) with fatal criticals, a real initially empty synthetic store and local workers. Added checks cover welcome, Project-menu and Library creation, name dialog closure, naming cancellation/blank validation, Save/Cancel before creation, one-window selection of Write, cleared search/outline, immediate exact multilingual writing/manual save, backend failure without a partial novel, and retained document/save state through layout-only refreshes. Existing chapter, theme, focus, navigation, detached-window, conflict and Word-export checks remain included.

Testing exposed lingering name dialogs and an editor preview that omitted its document: the UI had replaced the full cached frame, invalidating draft comparisons. Temporary name dialogs now close immediately, and layout-only replies preserve the current full document. The GTK draft/conflict check and explicit dirty/clean refresh checks pass with that repair. Screens were visually inspected. Logs: `work/notebook-0.3.4-new-novel-gui.log`, `work/notebook-0.3.4-rust-tests.log`, `work/notebook-0.3.4-contract.log` and `work/notebook-0.3.4-types.log`; previews: `work/notebook-new-novel.png` and `work/notebook-created-novel.png`. Release-worker, package and installed-copy observations are recorded in the external 0.3.4 release manifest. Personal writing and physical input/IME were not used for qualification.

---

# Chapter writing repair — October 10, 2026

Rust notebook 0.3.3 fixes a silent chapter activation: the previous outline handler opened scenes only. An existing chapter now opens its first scene, and an empty chapter presents an explicit Start writing button. Title-matching empty chapters remain visible in search. The first-scene action captures the chapter and project, refreshes metadata, disables repeated activation and ignores replies after a later page or closed window. Ordinary New scene creation now waits for the navigation save decision.

The locked offline workspace passed **230 tests, 2 ignored**. The expanded opt-in GTK workflow passed separately (**1 test**) with fatal criticals enabled and synthetic temporary stores. Its added Prologue checks cover no mutation on opening, explicit chapter targeting after a different sidebar selection, repeated Start clicks, exact multilingual edit/save, Save/Cancel on chapter navigation, an existing first scene, manuscript-overview entry, stale empty views, late replies, real middle-button outline routing, detached empty-page actions, close during loading and Cancel before New scene creation. The close/loading case exposed a late catalogue callback against a destroyed window; catalogue and index handlers now check window lifetime.

The workflow retains the prior focus regression, 51 theme checks and full manual-save/navigation/window/conflict/Word export coverage. Empty/writing screens were visually inspected. Debug logs: `work/notebook-0.3.3-chapter-gui.log` and `work/notebook-0.3.3-rust-tests.log`; previews: `work/notebook-empty-chapter.png` and `work/notebook-prologue-writing.png`. Release-worker, exact-package and installed-copy observations are recorded separately in the external 0.3.3 release manifest. Personal writing was not used for tests; physical input/IME remains separate.

---

# Notebook background and contrast repair — October 10, 2026

The added GTK check reproduced a dark dropdown before the fix: Breeze-Dark supplied an opaque RGB background of approximately (0.111, 0.122, 0.133) behind notebook text (`work/notebook-colors-before-fix.log`). The corrected application CSS sets the palette on menus, popovers, notebook stacks, text-view text nodes, number fields and list rows, including selected text and hover states.

The opt-in GTK workflow checks seven surface types in normal and inactive states, plus highlighted menu items, selected rows and selected text, under Breeze-Dark, Adwaita dark and Breeze light: **51 light-background/contrast checks**, requiring an opaque pale background and at least **4.5:1** foreground contrast. Theme changes affect the test process only and are restored before the notebook workflow. Screens for Write, Planning, Review and Project use synthetic stores; windows do not take desktop focus.

The complete workflow also retains the active-scene focus-crash regression and manual-save/navigation/window/conflict/DOCX coverage. Version-specific observations are in `work/notebook-0.3.2-*`; final release/package/installed-copy evidence is in the external 0.3.2 release manifest. Physical input/IME, accessibility and clean-host installation remain separate.

---

# Active-scene crash regression — October 10, 2026

The user report for PID 346105 and its journal identify a RefCell borrow panic at `desk.rs:66` during outline activation of an already-open scene. The extended GTK test reproduced the same abort before the fix (`work/notebook-focus-before-fix.log`). The active-editor clone now leaves its borrow scope before `grab_focus` enters the synchronous GTK callback.

Rust 0.3.1 passed **230 workspace tests, 2 ignored**, and the opt-in notebook GTK workflow passed separately (**1 test**) with fatal GTK criticals enabled, temporary synthetic stores and no desktop keyboard focus. The added check covers direct reopen, real outline row activation and repeated activation of a dirty multilingual draft, retaining its document, current tab and exact editor instance. The complete manual-save/navigation/window/conflict/DOCX workflow remains included. The conflict check waits for its window-owned reloaded editor.

Final package and installed-copy observations are recorded outside the packaged documentation in the 0.3.1 release manifest and `work/notebook-0.3.1-*` / `work/rust-package-checks.json`. The production main startup check and compiled GUI workflow are distinct; no physical keyboard/IME qualification or personal-store access is claimed. Earlier results below retain their stated version scope.

---

# Notebook redesign verification — October 10, 2026

The current Rust 0.3.0 interface uses manual saving and the notebook workflow in [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md). All checks use synthetic writing and disposable stores.

| Check | Observed result |
|---|---|
| Locked offline full Rust workspace | **230 passed, 2 ignored** |
| Explicit GTK notebook workflow | **1 passed**, with `G_DEBUG=fatal-criticals` and no keyboard focus |
| GTK notebook with packaged release workers | **1 passed**, including saved A4 Word styles and actual DOCX conversion |
| Release RPM/archive/main/workers | **6 check groups passed**: executable/library/doc bytes, notices, RPM equality/digests, editor, companion and isolated release-main startup |
| Rust/reference editor comparison | **147 operations matched**: exact document, selection, Undo and layout geometry |
| Dependency notices | **227 packages, zero missing notice files**; lock identity regenerated for 0.3.0 |

The GTK workflow checks manual-save inactivity; queued input and unfinished composition; Save/Discard/Cancel; ordinary active-tab replacement; guarded top-tab switching; a middle-button event; GTK drag-out with the same editor/Undo; detachable forms; sidebar board controls; same-window tool views and moved-page button routing; stale-save retention; explicit local-draft conflict recovery; guarded closing/project switching; the companion surviving its original window; and DOCX output. Screens were visually inspected.

Native checks exposed and fixed selecting an invisible newly added tab, the unchecked GTK create-window callback's reference lifetime, unreadable inherited theme colors, initial board selection and lazy form-load ordering. Test windows stay below other desktop windows and do not take keyboard focus. Logs: `work/notebook-rust-tests.log`, `notebook-native-gui.log`, `notebook-editor-comparison.log`, `notebook-release-workers-gui.log`, `notebook-release-checks.log`; previews: `work/notebook-*.png`. Package-specific evidence is recorded in PACKAGE_VALIDATION.md.

Physical mouse/keyboard/IME, human usability acceptance, accessibility, clean-host installation and real private-network behavior remain separate. No personal store or installed launcher was used. Rustfmt is absent and was not installed.

The earlier Rust-port and retained-interface results below qualify only their stated versions.

---

# Rust port verification — October 10, 2026

All checks below used existing dependencies, synthetic multilingual documents and disposable stores. No personal database or installed shortcut was used.

| Check | Observed result |
|---|---|
| Locked offline debug and release builds | Passed for native interface, editor and companion |
| Full Rust workspace, serial test fixtures | **230 passed, 2 ignored**; includes 11 new native-editor tests and chronology |
| Explicit GTK desktop workflow | **1 passed**; writes/saves multilingual rich text, independent split Undo, Book/shared sessions, planning persistence, DOCX/HTML, Back/Forward, stale-save retention and refused close |
| GTK workflow with packaged release workers | **1 passed**; same native workflow using the package's actual companion and editor |
| Rust/reference editor comparison | **147 operations matched**; exact document JSON, text/selection/Undo states and layout/caret/selection geometry, including checkpoint resume |
| Release archive/RPM/main/worker checks | Passed; six recorded groups covering byte equality, runtime libraries, notices, real release editor/storage and isolated release-main startup |
| Dependency notices | **227 packages, zero missing notice files**; active native Rust runtime/build graph |

Logs are under `work/rust-build-debug.log`, `rust-release-build.log`, `rust-workspace-tests.log`, `rust-native-gui-tests.log` and `rust-editor-parity.log`. The full suite ignores the explicit GTK check and the opt-in real-keyring case. The GTK check was run separately; its one pass is not included in the 230 count.

The native screenshot was inspected. Asynchronous scene opening initially left tab labels/close buttons hidden; explicit label visibility and a native regression fixed that. The GTK test opens without taking keyboard focus, preserving the current desktop's input; an earlier test run that accepted focus was contaminated by unrelated physical input and was not counted. An asynchronous test-fold/save race was fixed by waiting for its save completion before injecting the competing fixture head. The final refused-save/close check verifies retained local text and an unchanged competing database head.

Release payload evidence is recorded separately in PACKAGE_VALIDATION.md and `work/rust-package-checks.json`. The native workflow can also use the actual packaged workers; that remains distinct from a release-main startup check. No physical keyboard/IME, screen-reader, custom author font, clean-host installation, low-disk/power-loss or real private-network acceptance is claimed.

Rustfmt is absent from the existing pinned toolchain. It was not installed; compiler/workspace checks passed. No new timing benchmark was run. Earlier performance and React/WebKit workflow counts below apply to their stated historical runtime only.

---

# Observed desktop test results

Updated October 10, 2026. Earlier LibreOffice results below are historical; the replacement is qualified separately. Tests use synthetic multilingual fixtures and isolated stores. These observations do not certify a signed release or physical keyboard/IME behavior. Android is excluded. Reports with hardware/model timing remain outside Git; the detailed summaries are DESKTOP_PERFORMANCE.md and MODEL_EVALUATION.md.

## Application-owned editor — October 5, 2026

Source verification passed **303 frontend tests in 28 files**, **150 Rust test executions** (one opt-in real-keyring case ignored), **16 document-model**, **8 engine transaction/protocol**, **16 installed-Pango layout** and **31 native editor contract** tests. Model/engine/contracts passed together in 9.624 s before the additive logical-position frame field; its engine cases passed again after that field. Cases cover exact omitted/empty document shapes, rich Unicode, grapheme movement/deletion, format retention, cross-paragraph edits, guarded replacement, scalar conversion/reversal/Undo, bounded cache/history, size rollback, failure rollback, deep-scene viewports, Arabic/kasra baselines and independent sessions.

An exhaustive runtime-letter fixture retained all **141,028 Unicode 16.0 letters** through insertion, exact snapshot, Undo and Redo; the model suite passed in 1.828 s. This does not establish installed glyph coverage.

The replacement debug save-before-close regression passed in 0.647 s, and the 54,000-code-unit arrow/caret/manual-scroll/burst/exact-save regression passed in 50.715 s. Background timer waits are included in workflow duration; it is not keystroke processing latency. The native resize probe failed its applied-width assertion on both normal and forced-X11 runs: the background window remained 1200 pixels wide after requesting 760. The first sample retained text, viewport, controls and caret, but actual compositor resize/full-screen is unqualified. Exact final-package and installed results are recorded in PACKAGE_VALIDATION.md; physical input is separate.

## Rust storage and companion

October 3 integrated baseline: `cargo test --workspace --offline --locked -j2`, **75 passed** (32 companion unit, 1 responsiveness, 5 authentication/HTTP, 6 storage unit, 10 replication, 17 workspace and 4 native file tests). After adding all-head deletion resolution, the independent storage run also passed all 33 tests. Cargo used installed cached dependencies; no download was required.

The October 3 frontend baseline passed **144 tests in 16 files**. The desktop debug build passed with companion staging. That release frontend was 423.27 kB (126.26 kB gzip), with a separate 0.20 kB character-count worker. The isolated native workspace test passed, including corrected startup/navigation and exact selected-file archive delivery. Native close passed; the two real companion process integration tests passed, including process ownership, idempotent save, revision-bound patch, archive and killed-process acknowledged reopen. The current installed-engine regression suite passed **45 executions**. The final RPM also built and passed digest/extracted-content checks; installed release/update behavior remains unverified. Exact artifact hash and boundaries are in PACKAGE_VALIDATION.md.

Coverage includes exact mixed-script rich-text reopen; stale CAS rejection; immutable envelopes and change-marker rollback; recoverable v1/v2 migrations; revision ownership/checksum validation; new-project archive identity/reference remapping; schema/path/size/hash/missing-parent rejection; immutable alphabet/pack versions; scene metadata; search/projection/statistics; snapshots; metadata reorder/rename convergence; multiple heads; tombstones/edit-delete and deleted-scene restore; epoch rejection; duplicate push/save handling; resumable/corrupt asset transfer; pairing replay/expiry/revocation/project authorization and strict HTTP limits.

The HTTP test uses two real temporary SQLite stores through the private loopback listener. It is not a Tailscale or away-from-home test. The responsiveness test launches the built companion, blocks a fake loopback model's HTTP response, then verifies scene save acknowledgement and exact load before releasing the provider. It establishes persistence independence from a blocked inference response.

Fact approval and suggestion application reject changed, conflicted or foreign-project supporting sources in the same transaction as the new revision. This addresses preflight-to-commit races; unsupported or stale patches do not change stored writing.

## Editor and native lifecycle evidence

The earlier native save-before-close test passed against its own debug app and temporary database: pending Arabic/kasra/Chinese/emoji input drained before exit, exact text survived reopen, integrity_check returned ok and foreign-key checks were empty. Earlier native builds and launches passed on Fedora 44 KDE/Wayland with Rust 1.99.0, GTK 3.24.52, WebKitGTK 2.54.0 and LibreOffice 26.2.6.3. These results are separate from any later final package validation.

Synthetic WebKit rendering reproduced the kasra baseline shift with installed Vazirmatn, including software painting. Installed LibreOfficeKit preserved the baseline and exact text; no sentinel text or normalization was inserted. Engine tests exercise conversion, marks, repeated whitespace, hard breaks, blank blocks, undo/redo, ordered rapid input and paragraph properties. Native physical keyboard/IME, candidate windows, clipboard and accessibility have not passed merely because these fixtures do.

The earlier full native input probe used an isolated 20-paragraph scene. Final recorded samples: edit application 1.47–3.07 ms (median 2.06 ms), preview output 7.70–10.50 ms, native request/response 15–21 ms; the burst retained exact content and committed it. **Strict 3 ms whole-input processing is not met or guaranteed.** These are debug synthetic samples, not screen/physical-keyboard latency.

## Scale and actual model evaluation

DESKTOP_PERFORMANCE.md records 1,000 scenes/750,000 Chinese characters, 500 records, 2,000 events and a 1,000-card board. Save/search/archive/new-project import and acknowledged reopen checks passed. Observed save median 4.08 ms; archive export/import about 1.14/1.25 seconds. SSR board culling is not an interactive board/input test. Memory and exact binary hashes are in that report.

MODEL_EVALUATION.md records the actual local llama.cpp b11146 / Qwen3-4B Q4_K_M production adapter run. All eight results had validated exact quotations and unchanged manuscript revisions. Four seeded contradictions were detected, but **all four clean controls produced false positives**. Mean latency 19.6 seconds and peak server RSS 5.24 GiB. Semantic quality, Chinese script fidelity and reliable continuity detection are not qualified. Earlier numeric-offset model output failed evidence validation and was never applied.

## Actual proofreading pilot

PROOFREADING_EVALUATION.md records the authorized LanguageTool 6.6 pilot through the native companion and real frontend evidence validator. English and Arabic seeded errors each produced one suggestion, and their clean controls produced none. A Simplified Chinese clean request produced none; Traditional Chinese returned an explicit unsupported-variant error without fallback. All five successful responses had valid evidence and all six documents/revisions remained unchanged. Loopback-only binding was checked, and the owned temporary server stopped afterwards. This small pilot does not establish broad grammar quality or Chinese error detection.

## Reproduce without personal data

```sh
npm test
npm run build
cargo test --offline --locked -p novel-storage -p novel-companion -j 2
python3 tests/native_text_model_test.py
python3 tests/native_text_layout_test.py
python3 tests/native_editor_engine_test.py
python3 tests/native_editor_contract_test.py
python3 tests/companion_integration_test.py
python3 tests/gui/native_close_test.py
python3 tests/gui/native_workspace_test.py
python3 tests/gui/native_long_writing_test.py
python3 tests/gui/native_resize_test.py
python3 tests/extracted_release_test.py --offline
python3 tests/desktop_scale_test.py --binary target/debug/novel-companion --report work/desktop-scale-results.json
```

The native scripts require a compatible built debug executable and local display/runtime. A command listed here is not evidence that its latest revision has run; the observations above apply to the tested build, and packaging must be verified separately.

## Unverified release gates

Physical IME/font/accessibility; actual Tailscale setup and away-from-home synchronization; unattended keyring unlock; full attempted-egress audit; broad provider language quality beyond the small pilot; cold/slow/physical-low-disk/power-loss drills; uninstrumented release input/frame/resource measurements; signed host install/update and clean-machine backup restore. Native TLS, the approved synthetic real keyring roundtrip, SQLite capacity and owned-child migration/import/export/snapshot interruption are now executed; see the October 4 section. Android and its original cross-device acceptance gates are outside current scope.

## Native theme regression

After the reported unreadable dark-theme labels, the native integrated desktop fixture passed again (3.413 s). It checks actual WebKit computed foreground/background colors, including transparent-background composition, and requires sampled visible text controls to reach 4.5:1 in light, dark and current system modes across Write, Plan, Review, Alphabets and Project tools. Sidebar selection no longer animates between backgrounds with incompatible text colors. This is a synthetic computed-style regression, not a claim to have manually inspected every UI state.

## Interface cleanup — October 3, 2026

After the anti-slop edit, all 144 frontend tests passed, and debug and RPM builds succeeded. The integrated native fixture passed in 5.478 s across all five destinations at 1280, 900 and 760 logical pixels. It drains saves, checks light/dark/system text contrast with disclosures expanded, verifies the compact page title and round-trips the native project archive. The close regression passed in 1.064 s. Browser inspection of read-only synthetic native DOM snapshots covered Write, Project with Scene details expanded, Planning and Review; this does not verify physical typing, every error state or installed-system behavior. Review caught hidden scene-detail save feedback; status now stays in the summary and errors stay outside the disclosure. Alphabet pending saves no longer report Saved.

The refreshed extracted RPM started its packaged companion with no app log output; its standalone packaged helper preserved the multilingual document on open and snapshot. Keyboard input reached that interactive synthetic window, so this run did not establish an unchanged startup revision. RPM payload/digest/signature checks passed; no system installation was performed.

## Long writing correction — October 3, 2026

The installed-engine suite passed all 42 executions in 28.493 s. It now includes long single-paragraph navigation, Arabic/kasra/Chinese/emoji edits, exact checkpoints, manual viewport positioning and refusal of invalid viewport heights before editing. The isolated native long-writing fixture passed in 5.473 s: 54,000 initial UTF-16 code units, no engine or viewport failures, exact rapid-input checkpoint and persisted text, correct 51,655 final grapheme count, and no manual-scroll reversal or stale bitmap after an in-flight navigation response. The native close regression passed in 1.724 s and the integrated workspace/theme/archive fixture passed in 6.171 s at all three widths.

The final release build and 144 frontend tests passed after the worker-start failure guard. Package verification separately exercised the extracted LibreOffice helper against the same 54,000-code-unit draft, navigation and Arabic/kasra input; exact snapshot text and caret containment passed. No private scene was read by these fixtures. Long-paragraph engine processing remains above 3 ms; DESKTOP_PERFORMANCE.md records the measured limitation.

## Resize and scrolling correction — October 3, 2026

The final frontend run passed all 144 tests, and debug/release builds passed. The isolated native resize fixture passed in 8.983 s: eight window-size/maximize/full-screen transitions, preserved document anchors with valid end clamping, full bitmap coverage, visible controls, bounded paint area, visible caret after Arabic/kasra/Chinese input, no idle editor requests, exact checkpoints and exact persisted text. Eighteen rapid size changes produced three editor requests and retained viewport coverage. The debug-only resize commands refuse production use and operate only the owned synthetic window.

The native long-writing race fixture passed in 5.624 s and save-before-close in 1.971 s. The integrated workspace/theme/archive fixture passed in 8.129 s, including keeping formatting controls visible after document/inner scrolling at 1280, 900 and 760 logical pixels and a hidden editor remount followed by return from Alphabets. A read-only browser rendering of that fixture's captured DOM was inspected for layout; it is separate from the actual WebKit geometry assertions.

An additional 20-paragraph Arabic fixture passed the same resize/input/coverage checks and exact saving. Its native processing samples were 0.75–4.49 ms, with 19–40 ms input/response-plus-frame samples and 7 ms maximum extra main-thread heartbeat delay. These synthetic results do not certify physical IME or a 3 ms limit. The long single-paragraph processing limit remains documented in DESKTOP_PERFORMANCE.md. The final RPM payload/digests and extracted long-draft helper check passed; no package installation was performed.

## Arrow navigation and helper shutdown — October 3, 2026

The new visual-direction regression failed before correction and passed after using the installed native constants (Down 1024, Up 1025). The complete installed-engine suite passed 45 executions in 29.820 s, including English/Arabic-kasra/Chinese movement, Shift selection and exact snapshots. Every fixture now requires a successful helper exit instead of merely waiting for it.

Two subprocess shutdown regressions passed in 1.507 s. Ordinary open, Ctrl-End, Enter, undo, redo and snapshot requests followed by stdin EOF reproduced the supplied autocorrect/global-destructor exit stack before the fix. Explicit document/Kit cleanup remains in place, responses are flushed, then the disposable helper skips unsafe process-exit destructors. A separate injected unexpected Python failure still reports its error and exits nonzero. This does not qualify crashes during active editing or native cleanup itself.

The rebuilt debug app's actual keyboard event handler passed Up/Down routing in the 54,000-code-unit native fixture, alongside exact multilingual persistence, caret coverage and manual scrolling (8.790 s). Save-before-close passed separately (3.297 s), preserving queued Arabic/kasra/Chinese/emoji text and database integrity. These owned fixtures used temporary databases; no personal manuscript or crash-core memory was read. They do not certify physical keyboard/IME behavior.

## Desktop continuation — October 4, 2026

Seven incremental-source checks passed, including a 1,000-scene change requiring one new digest, unchanged reload/metadata reuse, exact Unicode/revision binding, in-flight older results and failed-digest retry. The planning slice passed 38 tests in four files, including 14 new regressions. Synthetic 1,000/999 and 10,000/9,999 card/edge boards each mounted 15 freeform cards (~10.9k markup characters), while lists and connection pickers use 50-item pages. These SSR observations do not certify native frame rates or screen-reader behavior.

Storage passed 41 tests (12 library, 12 replication, 17 workspace). New fixtures induce real SQLITE_FULL using a temporary database page cap, kill owned child processes at migration/import/export/snapshot boundaries, and verify acknowledged text, schema rollback, integrity and private snapshot publication. A 20,002-revision history is refused by archive preflight without deletion and remains complete in a consistent backup. This is not a physical filesystem ENOSPC or power-loss test. Project/remote-epoch receipt counts and duplicate receipts are transactional and survive consistent backups.

Three real companion process fixtures passed (0.058 s), including deliberately damaged optional recovery/auth/peer metadata while exact saving and reopening still work. The isolated native workspace/theme/archive fixture passed (7.718 s) and native long-writing/arrow/scroll fixture passed (5.264 s) against the integrated debug snapshot. Later final build checks are recorded separately below.

The prior RPM was extracted outside the checkout and exercised through external WebKit instrumentation. No production probe or bypass was added. Its isolated no-edit startup retained exact text and revision; synthetic DOM input saved exact Arabic/kasra/Chinese/emoji/markup text, manuscript markup stayed inert, all destinations mounted, archive restoration created a separate project, and acknowledged content/integrity survived exit. The same fixture passed with the owned app's network disabled through an unprivileged network namespace. This is offline availability evidence, not a complete attempted-egress trace, physical IME test or installed RPM qualification. Final artifact verification must repeat after the new build.

The integrated October 4 source run passed **98 Rust tests** (32 companion unit, 14 native peer, 1 responsiveness, 6 sync service, 41 storage, 4 native file) and **167 frontend tests in 19 files**. Native peer tests exercise actual loopback pair/sync/retry/reopen/conflicts with fake credential storage, 5 MiB assets resumed within a 4 MiB pass budget, failed cursor publication, revoked/old epochs, older backups and missing finalized asset bytes. Real OS keyring usability is separate. Asset lookups validate one scoped BLOB; transfer chunks use bounded SQL slices capped at 256 KiB, and finalize replies contain metadata rather than whole bytes.

After the final list-pagination and strict HTTPS-candidate checks, the full integrated run passed **100 Rust tests** (32 companion unit, 16 native peer, 1 responsiveness, 6 sync service, 41 storage, 4 native file) and **168 frontend tests in 19 files**. TypeScript and the release build passed. The debug native workspace, close and long-writing fixtures passed in 7.464 s, 1.247 s and 4.765 s; three companion process fixtures passed in 0.053 s. The later release board probe checks the final pagination change.

The newly extracted release passed **25 checks with networking disabled**: no-edit startup retained its revision, exact Arabic/kasra/Chinese/emoji input drained into storage, markup remained inert, 1,000 cards/999 typed edges used a bounded SVG and visible cards, final-card keyboard reveal/movement retained focus, manual panning retained focused content, list pagination and arrows stayed on the displayed page, remount preserved the exact planning payload without creating a revision, and complete archive import retained cards/edges in a separate project. All destinations mounted; acknowledged revision, integrity and foreign keys survived release exit. External instrumentation is test-only and alters timing, so this is functional release evidence rather than an uninstrumented performance or physical-IME result.

The initial offline cache lacked HTTPS libraries. After explicit approval, pinned Reqwest/Rustls dependencies were downloaded and verified native HTTPS was integrated. Pure startup validation performs no lookup; explicit operations require exact Tailscale Serve/443 names, bounded DNS answers wholly within Tailscale ranges and pinned addresses. Platform chain/hostname verification is mandatory; proxy/redirect and arbitrary certificate/DNS controls are unavailable in production. Ephemeral TLS fixtures pass real pairing/revision/500 KiB asset exchange and retries; wrong SAN, expired leaf, untrusted CA and redirects reject. Actual Tailscale setup and away-from-home connectivity remain unverified.

The final integrated run passed **111 Rust executions** (37 companion unit, 22 peer, 1 responsiveness, 6 service, 41 storage, 4 native file) plus one ignored opt-in keyring test, and **169 frontend tests in 19 files**. The 37 unit and 22 peer totals each execute the same five private TLS fixtures through their module paths; they are not ten distinct TLS cases. After separate approval, the ignored native keyring test passed (0.12 s): one fresh synthetic token was stored/read/cleared and no existing credentials were inspected. This establishes the current session's adapter roundtrip, not unattended unlock.

The rebuilt debug native workspace passed (8.143 s), including partial peer failure after durable scene/metadata ingestion: the primary error stays visible, scene/details refresh, pending controllers drain before remount and input resumes. Close passed (1.088 s), long-writing/arrow/scroll passed (4.886 s), and three companion process checks passed (0.071 s). The final release includes the shorter sync action labels. Its frontend is 440.49 kB (131.58 kB gzip), with a separate 0.20 kB counting worker.

The final RPM's external offline probe passed all 25 checks again, and its extracted helper passed exact 54,000-code-unit navigation/Arabic-kasra saving and acknowledged zero-exit shutdown. Notice/helper/companion resources match exact sources/builds; RPM digests passed. Both English and Chinese release scale fixtures passed against the final companion; measurements are in DESKTOP_PERFORMANCE.md. A temporary RPM transaction-root attempt was refused at database lock creation with Permission denied, even in an owned precreated directory and unprivileged user namespace. No install transaction or system-policy change occurred; this is an unavailable installation check, not a passed replacement drill.

## October 4 optimization and cleanup

- Frontend: 177 tests in 20 files; TypeScript and production build passed.
- Offline locked Rust workspace: 111 tests passed; the opt-in real keyring test was not repeated.
- Native debug long-draft, close/save guard and workspace/contrast/file-delivery tests passed. The final three-test run took 61.338 s in this background graphical session.
- The final owned resize fixture retained exact persisted text, caret coverage, controls, manual scroll anchors and bounded requests. However, all six numeric native width requests stayed at 1200px; it is **not** a successful actual window resize/full-screen qualification. Nine animation-frame waits used a recorded bounded layout wait. Its timer-delay observation (990 ms) is confounded by background throttling and is not a typing latency result. The default resize test now requires requested numeric widths to have actually applied.
- Independent RPM extraction: 27 production UI/IPC checks passed offline with 1,000 scenes and a 1,000-card/999-edge board. Exact multilingual text/revisions, inert markup, paragraph controls, archive restoration and exit integrity passed. An actual WebKit snapshot was inspected for the compact writing layout.
- Packaged helper: exact 54,000-code-unit Arabic/kasra/Chinese draft, caret coverage, Up/Down and ordinary acknowledged shutdown passed.
- RPM digests and independent detached build signature passed; packaged binaries/helpers/notices match the build inputs.

Current RPM: 9,913,676 bytes, SHA-256 `9ee99ed81e2150cb3d26372603ae38d6d1858c2439b08f082aa94d918df4afe8`. The signature is a personal integrity artifact, not an embedded RPM signature or trusted publisher.

These fixtures use temporary synthetic data. They do not open personal manuscripts or qualify physical IME, screen latency, actual full-screen transitions or strict 3 ms long-paragraph processing. The per-keystroke cached viewport experiment was removed after a regression; layout dimensions remain live. Installation status is recorded separately in PACKAGE_VALIDATION.md.

## October 5 desktop tabs and remote verification

The frontend suite passed **218 tests in 24 files**. The locked offline Rust workspace passed **139 test executions**, with the real-keyring test ignored rather than triggering an unlock while the author was remote. Counts include shared module fixtures executed through more than one target. Production TypeScript/build and deterministic generated-contract drift checks passed. Nine serialized Rust workspace types have actual Rust-to-TypeScript compatibility fixtures; RPC operation signatures and opaque document validation are not generated.

The native legacy workspace test initially stalled. Diagnosis found an alphabet-load callback repeatedly remounting its own editor; unchanged loads no longer remount, and changed legacy assignments reload only once. The corrected workspace/navigation/theme/native-file fixture passed in **6.378 s**. Zero-width hidden layout updates are skipped, hidden snapshots retain finite native coordinates, manual-scroll detection ignores hidden dimensions, and activation restores layout. Three new coordinator cases cover alphabet-load/remount behavior. Three companion-process fixtures passed in **0.051 s**. The final single-window queued-input close regression passed in **1.214 s**; the 54,000-code-unit native long-writing/arrow/scroll regression passed in **52.747 s** in the background session. Total workflow duration includes throttled animation/timer waits and is not a keystroke measurement.

External instrumentation against owned debug windows passed **33 checks in 9.024 s** for transfer/return and **30 checks in 6.198 s** for original-window closing. Both test long Arabic/kasra/CJK text, nonempty selection, exact caret/selection/scroll, preserved undo/redo, no revision from handoff, composition refusal retaining the sole source owner, and view retention through a background tool-tab drain. The ordinary workflow also checks two independent native scene sessions, source-owner edit refusal, one canonical returning tab and all-window quit. The close workflow initiates a real native GTK CloseRequested event in the original fixture window, then acknowledges exact new text in the detached window; sampled companion count stays one and the saved head/SQLite integrity survive final quit. The helper is external, scoped to this temporary process and never packaged.

The close test exposed two event-targeting mistakes: native Window.emit broadcasts, and JavaScript listeners default to an Any target. Per-window native emissions and JavaScript listeners now explicitly bind the owning label. App-wide notifications and whole-app cancellation remain broadcasts. The corrected close workflow passed; no normal app window or personal manuscript was inspected.

These are synthetic UI/native functional checks. External hooks change timing; aggregate process RSS includes shared pages and does not establish private memory or a 3 ms typing budget. Physical keyboard/IME, real compositor full-screen changes, three-window interactive load, installation/update and private deployment remain separate gates. Exact final-RPM results follow in PACKAGE_VALIDATION.md.

Final exact-RPM checks passed against SHA-256 `4e05132711580dbf2cea43d2f939141999b129e85d301f45037d68342ea1ff31`: 33 transfer/return checks in 8.250 s, 30 original-window-close checks in 6.438 s, and 28 offline checks with 1,000 scenes in 12.146 s. The latter includes actual computed tab/menu contrast in all three theme modes and the 1,000-card/999-edge board/archive workflow. The packaged helper retained exact 54,000-code-unit text, correct Up/Down, caret painting and acknowledged shutdown. English/Chinese final-companion storage/archive/reopen scale fixtures passed again.

The metadata-only dynamic egress interposer passed its loopback/resolver/exec-inheritance and canary-exclusion self-test. Final application observation recorded six preload markers in five instrumented processes and zero intercepted INET socket/connect/sendto/getaddrinfo calls. This does not cover direct syscalls, static/hidden libc paths, noninterposed send/sendmsg/write, helpers dropping instrumentation or a complete OS audit. No network/service configuration or administrator install was performed. Independent public-key detached signature verification passed.


## Installed RPM and first physical Arabic check — October 5

Fedora administrator authentication completed and the approved local `novel-workspace-0.1.0-1.x86_64` RPM installed with cached dependencies and zero downloaded bytes. `rpm -V` is clean. Exact artifact SHA-256: `4e05132711580dbf2cea43d2f939141999b129e85d301f45037d68342ea1ff31`. Both existing shortcuts now target the installed binary.

The installed payload passed 28 isolated offline workflow checks with 1,000 synthetic scenes in 7.281 s. Installed files were additionally compared with the RPM before the workflow. Exact multilingual persistence, bounded 1,000-card/999-edge board, archive restoration, menu/theme contrast and acknowledged exit integrity passed. External instrumentation means this is functional evidence, not physical typing latency.

A separate uninstrumented installed-app temporary project was opened for the author. After typing `مِمِمِ سلام محمد` with the physical Arabic keyboard, the author reported “Correct and steady”. This establishes joined letters and stable kasra for that sample only. Remaining physical checks are separate. Only this PC is available, no Chinese input method is in use and no invented-alphabet font/sample is available; real two-device routing, Chinese IME and the intended custom font cannot be qualified from these observations.


## Scene tools and typing priority — October 6, 2026

The five selected desktop ideas are implemented: bounded typing priority, pinnable references, authored reader order, character knowledge and selective project time travel. SCENE_TOOLS.md describes the controls. The source verification passed 377 frontend tests/33 files, 158 Rust tests with one opt-in keyring test ignored, and 76 native model/engine/layout/contract checks. Eight storage history checks cover exact point identity after older same-epoch snapshots, chronology/placement, conflicts, deleted-scene revival, CAS and fault rollback; real archives remap reveal/knowledge scene IDs and preserve literal fact snapshots. Knowledge retains the original property/value pair after canon edits and marks it stale.

Exact RPM SHA256 `aa59e984c9e03ac28f0bb60c5ccdbcb637b61e785c3d6edbe2a52af8ef7371df`, 10,225,223bytes. Workflows passed: Scene tools: 23 checks, Offline 1,000-scene workspace: 29 checks, Split drafts / Unicode: 32 checks, Window transfer / return: 41 checks, Close empty original window: 32 checks, Installed user-folder copy: 30 checks. The helper/resource equality, 54,000-character exact navigation/shutdown, scoped egress self-test/zero intercepted INET and independent public-key signature checks passed. Main shortcuts use the verified user-folder payload; no personal database or system update was involved.

The native helper burst comparison improved 260 inputs/260 Undo steps from 309.8 ms to 136.7 ms. Final full-paragraph shaping remained about 72 ms; strict general 3 ms is unmet. Package timings are instrumented functional observations and fresh physical input remains open. Android remains deferred.


## October 6, 2026 — giant-paragraph performance accepted

After the benchmark was explained as one 54,000-character mixed-script paragraph with approximately 72 ms processing (roughly 8,000–10,000 English words without paragraph breaks), the author replied “I am OK with this.” That extreme-case performance is accepted and no longer blocks desktop completion. Further incremental bidi/layout optimization is optional. Measurements are unchanged; no general 3 ms guarantee or physical-input qualification is inferred. Ordinary typing responsiveness remains a design goal. Android stays deferred.


## Six story workflows — October 6, 2026

All six selected desktop additions are implemented; see STORY_WORKFLOWS.md. Source verification passed **460 frontend tests in 41 files**, **170 Rust test executions** plus one ignored opt-in keyring test, and **84 core Python editor tests**. TypeScript/Vite production build and generated-contract drift checks passed. Native checkpoint tests exercise validated data-only rich-block sharing, Undo/Redo, cursor/selection, exact resume and failure preservation. Branch tests cover durable lineage, copy/remap/history, exact CAS bring-back, conflict and fault guards. Hidden-window/layout and bounded character-link indexing regressions passed.

Exact final RPM SHA256 `6a66eecbbeb2274647d85c4e40c9c4c7e43eb19d1d678402c9a11029de2b528d`, 10,363,712 bytes:

| Workflow | Result |
|---|---|
| Story branches / alternatives / ripple / rehearsal | 24 checks; 3.574 s |
| Whole-book writing / character cut (1,000 scenes) | 16 checks; 16.466 s |
| Book filter / window handoff / Undo | 26 checks; 7.710 s |
| Offline 1,000-scene workspace | 29 checks; 11.252 s |
| Split drafts / Unicode | 32 checks; 8.600 s |
| Scene tools | 23 checks; 5.548 s |
| Window transfer / return | 41 checks; 5.715 s |
| Close original window | 32 checks; 4.935 s |
| Installed user-folder copy | 30 checks; 11.757 s |
| Installed whole-book / character cut | 17 checks; 16.598 s |

Packaged helper/source resources, exact 54,000-code-unit navigation/caret/shutdown, scoped egress self-test/zero intercepted INET, and independent public-key detached signature verification passed. The complete payload behind desktop/menu shortcuts matches the RPM; installed-copy tests use only synthetic stores.

An earlier broad source-test invocation accidentally included the legacy debug resize GUI fixture. Its applied-width assertion failed: the compositor retained 1,200 pixels instead of the requested 760; text/caret/viewport preservation passed. It is not counted among the 84 passed core Python tests and does not qualify physical resize/full-screen behavior. Initial packaged whole-book tests exposed stale card-height measurement and test-fixture defects; final hash checks above follow the geometry fix. Book-window diagnostics exposed premature scroll restoration against hidden/loading geometry. Restoration now waits for the active measured viewport, committed native frame and matching rendered rows, then informs the manual-scroll coordinator. Those failed attempts are retained and are not counted as passes.

Physical input/IME, real compositor transitions, clean-host and second-PC/private-sync qualification remain open. Instrumented workflow durations are functional observations, not a 3 ms typing guarantee. The accepted giant-paragraph result is unchanged. Android stays deferred.


## Writing desk and full screen — October 6, 2026

Source verification passed **543 frontend tests in 48 files**, **171 Rust test executions** plus one ignored opt-in keyring test, and **87 core Python editor tests**. Generated-contract, TypeScript/Vite and production RPM builds passed.

Exact RPM SHA256 `11bd5b1a8c75cdca01412aa6c68c7a66b89bd13ef8de999eaac00aa9e7b1da64`, 10,380,187 bytes.

| Workflow | Result |
|---|---|
| Writing desk / full screen / navigation / folding / sequences | 24 checks |
| Full-screen split drafts | 32 checks |
| Whole-book / character cuts, 1,000 scenes | 16 checks |
| Folded Book window handoff / filter / Undo | 32 checks |
| Story workflows | 24 checks |
| Offline workspace, 1,000 scenes | 29 checks |
| Split drafts / Unicode | 32 checks |
| Scene tools | 23 checks |
| Window transfer / return | 41 checks |
| Close original window | 32 checks |
| Installed workspace | 30 checks |
| Installed whole-book | 17 checks |
| Installed Writing desk / full screen | 25 checks |

Resources, giant-paragraph helper, scoped egress and independent public-key signature verification passed. Main shortcuts select the verified user-folder payload.

Early package diagnostics exposed a CSS specificity error in width preferences and wide-screen bitmap enlargement. The final CSS preserves selected card width and caps the native surface at its supported 1200 px so text, caret and pointer geometry remain at 1:1 scale. Fixture corrections wait for save/readiness commits, provide real dragenter/dragend lifecycle events, and seed enough text to require scrolling at full-screen width. Earlier failed attempts are not counted as passes.

Synthetic DOM input does not qualify physical keyboard/IME or mouse dragging. The current-host production full-screen transition passed at 1920×1080; other monitors, physical typing/compositor interaction, accessibility, clean-host/power-loss/low-disk and second-PC/private Tailscale acceptance remain separate. Chinese IME and a permitted custom-alphabet sample are unavailable. The accepted approximately 72 ms giant-paragraph result is unchanged; a general 3 ms processing guarantee is not established. Android remains deferred.


## Compact writing page and full width — October 6, 2026

Source verification passed **543 frontend tests in 48 files**, **161 Rust test executions** plus one ignored opt-in keyring test, and **90 Python editor tests**. Generated-contract, TypeScript/Vite and production RPM builds passed.

Exact RPM SHA256 `d98fa740ae7e75683d3b78cd492eb6d7f180a34c071970c7fcc2e5989695e1f7`, 10,380,624 bytes.

| Workflow | Result |
|---|---|
| Minimal single draft / full width / menu | 14 checks |
| Writing desk / full screen / navigation / folding / sequences | 25 checks |
| Full-screen split drafts | 32 checks |
| Whole-book / character cuts, 1,000 scenes | 16 checks |
| Folded Book window handoff / filter / Undo | 32 checks |
| Story workflows | 24 checks |
| Offline workspace, 1,000 scenes | 29 checks |
| Split drafts / Unicode | 32 checks |
| Scene tools | 23 checks |
| Window transfer / return | 41 checks |
| Close original window | 32 checks |
| Installed workspace | 30 checks |
| Installed whole-book | 17 checks |
| Installed Writing desk / full screen | 26 checks |
| Installed minimal single draft | 15 checks |

Resources, giant-paragraph helper, scoped egress and independent public-key signature verification passed. Main shortcuts select the verified user-folder payload.

Early package diagnostics exposed a CSS specificity error in width preferences and wide-screen bitmap enlargement. The final CSS preserves selected card width and removes the centered 1200 px limit and supports native viewports up to 4096 px wide and 2160 px high. Text, caret and pointer geometry retain 1:1 scale; the paragraph is shaped at the actual available width. Fixture corrections wait for save/readiness commits, provide real dragenter/dragend lifecycle events, and seed enough text to require scrolling at full-screen width. Earlier failed attempts are not counted as passes.

Synthetic DOM input does not qualify physical keyboard/IME or mouse dragging. The current-host production full-screen transition passed at 1920×1080; other monitors, physical typing/compositor interaction, accessibility, clean-host/power-loss/low-disk and second-PC/private Tailscale acceptance remain separate. Chinese IME and a permitted custom-alphabet sample are unavailable. The accepted approximately 72 ms giant-paragraph result is unchanged; a general 3 ms processing guarantee is not established. Android remains deferred.


## Test review — October 6, 2026

After cleanup, 543 frontend tests in 48 files and TypeScript passed, as did 161 Rust test executions (one opt-in keyring test remains ignored), 90 Python editor checks and three real companion stdio integration tests. TLS scenarios execute once each. TEST_SUITE.md records removed duplicates and obsolete checks, retained failure paths and explicit GUI/benchmark commands. The four legacy debug GUI diagnostics are outside ordinary discovery; the known resize anchor mismatch remains unresolved and is not counted as passing.

The cleanup changes test code only. Current RPM workflow and installed-copy results above remain valid; personal writing was not inspected.


## Scene navigation and everyday actions — October 6, 2026

Frontend verification passed **550 tests in 48 files**, including seven added creation/filter/navigation regressions. Generated-contract, TypeScript/Vite and production RPM builds passed. The unchanged native/storage baseline remains **161 Rust test executions** plus one ignored opt-in keyring test, **90 Python editor tests** and **three companion stdio integration tests**; those suites passed during the preceding test audit.

Exact RPM SHA256 `ed714cbf9b13a6c09badfbd0d09cd5b217820a68eb91ee2812eb6631e0ef197a`, 10,381,556 bytes.

| Workflow | Result |
|---|---|
| Everyday writing / scene search / creation / notes / split / full width | 23 checks |
| Writing desk / full screen / navigation / folding / sequences | 25 checks |
| Full-screen split drafts | 32 checks |
| Whole-book / character cuts, 1,000 scenes | 16 checks |
| Folded Book window handoff / filter / Undo | 32 checks |
| Story workflows | 24 checks |
| Offline workspace, 1,000 scenes | 29 checks |
| Split drafts / Unicode | 32 checks |
| Scene tools | 23 checks |
| Window transfer / return | 41 checks |
| Close original window | 32 checks |
| Installed workspace | 30 checks |
| Installed whole-book | 17 checks |
| Installed Writing desk / full screen | 26 checks |
| Installed everyday writing, 1,000 scenes | 24 checks |

Resources, giant-paragraph helper, scoped egress and independent public-key signature verification passed. Main shortcuts select the byte-matched user-folder payload. Reports use the `work/novelist-*-oct6` prefix outside the source repository. Screenshots were inspected for full-screen spacing, title size and project-selector readability.

Earlier attempts against an intermediate candidate failed because external probes selected the old numeric navigation order and searched before a mounted Find field accepted input. The final probes select named actions, wait for ready controls and accept the documented wrapped-search status only with an actual native selection. These failed attempts are not counted as passes. Visual inspection also exposed a project-selector contrast defect and CSS specificity errors for the compact title/hidden tab gap; corrected production styles were rebuilt before the final hash checks.

Synthetic DOM input does not qualify physical keyboard/IME or mouse dragging. The current-host production full-screen transition passed at 1920×1080; other monitors, physical typing/compositor interaction, accessibility, clean-host/power-loss/low-disk and second-PC/private Tailscale acceptance remain separate. Chinese IME and a permitted custom-alphabet sample are unavailable. The accepted approximately 72 ms giant-paragraph result is unchanged; a general 3 ms processing guarantee is not established. Android remains deferred.


## Compact pages and integrated controls — October 6, 2026

Frontend verification passed **550 tests in 48 files**. Generated-contract, TypeScript/Vite and production RPM builds passed. The unchanged native/storage baseline remains **161 Rust test executions** plus one ignored opt-in keyring test, **90 Python editor tests** and **three companion stdio integration tests**, from the preceding audit. The new page-control slot retains the existing scene-switch save and failed-drain assertions.

Exact RPM SHA256 `6c336e8a0f5fc516158486393cfe4a8be395cb17576d641901672cf552098406`, 10,381,485 bytes.

| Workflow | Result |
|---|---|
| Everyday writing / scene search / creation / notes / split / full width | 23 checks |
| Writing desk / full screen / navigation / folding / sequences | 25 checks |
| Full-screen split drafts | 32 checks |
| Whole-book / character cuts, 1,000 scenes | 16 checks |
| Folded Book window handoff / filter / Undo | 32 checks |
| Story workflows | 24 checks |
| Offline workspace, 1,000 scenes | 29 checks |
| Split drafts / Unicode | 32 checks |
| Scene tools | 23 checks |
| Window transfer / return | 41 checks |
| Close original window | 32 checks |
| Five-page layout / menus / neutral themes | 80 checks |
| Alphabets, 760 px / publish / assign | 10 checks |
| Installed workspace | 30 checks |
| Installed whole-book | 17 checks |
| Installed Writing desk / full screen | 26 checks |
| Installed everyday writing, 1,000 scenes | 24 checks |
| Installed five-page layout / menus / themes | 81 checks |
| Installed Alphabets, 760 px / publish / assign | 11 checks |

All five page screenshots and the real 760 px alphabet editor were inspected. Package bytes/resources, public-key-only detached signature, scoped egress and installed payload equality passed. Main shortcuts select the final payload. Earlier intermediate screenshots exposed green Review styling, a scene selector forcing a second action row, and the small-window outline covering alphabet fields. Those styles were corrected before the final hash checks. Initial frontend failures came from the shallow hook harness not traversing the new pageActions slot; its traversal was updated while retaining exact review drain/failure assertions. Failed attempts are not counted as passing.

The 760 px observation is an instrumented GTK window with synthetic form input, not physical keyboard/IME qualification. Personal writing was not opened. Human usability, physical/custom-font and second-PC/private-network gates remain separate; Android remains deferred.


## Chapter editing and shared-universe library — October 6, 2026

Frontend verification passed **558 tests in 49 files**. The locked offline Rust workspace passed **167 tests**, with one opt-in keyring test ignored. Generated-contract, TypeScript/Vite and production RPM builds passed. The unchanged native editor/stdio baselines remain 90 Python editor tests and three companion stdio integration tests from the preceding audit.

Exact RPM SHA256 `19fff51f1730f6b18d3289a21a05de4a37902ec0677fb824e544aa14ba2b3708`, 10,437,545 bytes.

| Workflow | Result |
|---|---|
| Chapters / universe / two series / existing and new novels / shared world notes, full screen | 18 checks |
| Same hierarchy and chapter workflow, real 760 px GTK window | 18 checks |
| Everyday writing / scene creation / exact Unicode / split, 1,000 scenes | 23 checks |
| Whole-book / character cuts, 1,000 scenes | 16 checks |
| Branches / paragraph alternatives / ripple / rehearsal | 24 checks |
| Five-page layout / menus / neutral themes | 80 checks |
| Installed hierarchy workflow | 19 checks |
| Installed narrow workflow | 19 checks |
| Installed single workflow | 24 checks |
| Installed book workflow | 17 checks |

Native rollback/CAS/concurrent-membership/sync/archive/recovery tests passed. Library screenshots at 760 px and 1920×1080 were inspected. Package resources, independent public-key-only signature, scoped egress, installed payload equality and archive/checksums passed. Earlier probe attempts failed because instrumentation filled a form before its tab had rendered; explicit UI readiness was added and those failed attempts are not counted. The shallow coordinator tests preserve draft-drain and exact-source assertions. No personal store was opened; no physical/IME or human usability acceptance is claimed. Whole-universe export and hierarchy branch merging remain outside this slice.

## Chapter zero and compact line spacing — October 6

RPM SHA256 `1a77ae4328f009451c4b273529aa95e0efdd54dacaf085944d4ccc79325c9106`, 10,464,149 bytes. All 559 frontend / 173 Rust / 95 native Python tests passed; one opt-in keyring test ignored. Production build, exact packaged resources, public-key-only detached-signature verification, scoped offline egress and user-folder payload checks passed.

| Workflow | Extracted package | Shortcut-selected build |
|---|---|---|
| Chapter 0 / back to 1 / new chapters / mixed-script 1.15 / Book / full screen | 12 checks | 13 checks |
| Same workflow, real 760 px GTK window | 12 checks | 13 checks |
| Everyday writing / exact Unicode / split, 1,000 scenes | 23 checks | 24 checks |
| Whole-book / character cuts / park and resume Undo, 1,000 scenes | 16 checks | 17 checks |

Original drafts/heads are preserved by chapter edits. Numbering persists through reopened stores, archive import/export and synced conflicts; stale writes and injected failures roll back atomically. Real Pango tests verify compact leading, zero added paragraph gap, full glyph ink at viewport boundaries, inherited Enter formatting and stable incremental Arabic baselines. Dense marks may overlap at compact leading and mixed fonts retain slight baseline differences. Screenshots of the actual package at 760 px and 1920×1080 were inspected. Personal store/windows and system RPM were untouched. Physical-input/user acceptance and Android are not claimed.


## WordCraft Word manuscript export — October 10, 2026

The frontend suite passed **565 tests in 50 files**. The locked offline Rust workspace passed **192 tests**, with one opt-in keyring test ignored. Generated-contract, TypeScript/Vite, debug desktop and production RPM builds passed. Native editor/Pango code was unchanged; the 95 Python editor baseline is prior evidence, not a rerun for this slice.

Final RPM is **10,945,238 bytes**, SHA256 `c86f66d94e59c5e3bfe1956937446ed32d7a8648da3a6d85ab81173383660e20`.

| Check | Observed result |
|---|---|
| Independent DOCX ZIP/XML contract | 9 package parts, 19 exact ordered paragraphs; styles, Chapter 0, page breaks, language/direction, spacing, marks and empty/hard-break paragraphs verified |
| LibreOffice import/export | Exact paragraph text/order retained; 77,210-byte PDF rendered and both pages inspected |
| Converter boundaries | Exact Unicode/U+FFFC/control handling, unknown metadata refusal, global object budget including titles, read-only source and 250,000-word novel passed |
| Storage export snapshots | Clean/read-only, competing drafts, edit/deletion, two tombstones on deleted scene, resolved branches and unrelated-project conflicts passed |
| Packaged one-window export | 15 checks: pending multilingual edit persisted as a new revision, actual DOCX matched its snapshot and all pre-existing archive revisions remained |
| Packaged two-window export | 16 checks: peer draft was unsaved before export, every owning window acknowledged saving, persisted head and actual DOCX contained the exact edit |
| Everyday writing, 1,000 scenes | 23 extracted-package checks |
| Whole-book writing, 1,000 scenes | 16 extracted-package checks |
| Package resources and notices | Companion/helpers/chooser/notices equal source; main equals built executable except bundle marker; all three vendored WordCraft notices present |
| Signature and scoped egress | Public-key-only detached-signature verification and offline namespace egress observation passed |

Release export probes replace only the chooser in a disposable extracted copy to select a synthetic destination. Both reports record the original helper hash, exact RPM identity and unchanged remaining payload. This exercises production UI/native export, not stock GTK chooser interaction. All writing data is synthetic and isolated. Tests do not establish physical keyboard/IME, Microsoft Word application behavior, clean-host installation or power-loss recovery. The two-page PDF renders Arabic marks, Chinese, heading hierarchy and explicit chapter break; installed fonts omit the emoji and show a private-use tofu glyph, while exact Unicode survives the DOCX roundtrip.

The initial upstream writer compilation exposed quick-xml 0.42 API incompatibility; pinning the upstream 0.38.4 version resolved it without XML-source rewrites. An initial interoperability comparison mistook LibreOffice's relocated page break for a text newline; it now excludes page-type breaks while retaining exact text and ordinary hard-break checks. Early external probe/report-shape attempts failed and are not counted as passes.

Evidence is under `work/docx-*`, outside the tracked source. DOCX_EXPORT.md records usage and limits.

The byte-matched shortcut-selected user-folder copy passed **24 everyday-writing** and **17 whole-book** checks against the same final RPM. Publication retained the previous build, updated the current symlink atomically and did not open a personal store or stop personal windows.


## Optional translation slice — October 10, 2026

The final isolated source build passed **589 frontend tests in 55 files** and **218 locked offline Rust tests**, with one opt-in OS-keyring test ignored. Contract generation checks, TypeScript/Vite and the offline production package build passed. Test fixtures cover editable CAS saves, immutable/foreign/forged sources, archive/branch remapping, incremental sync rollback, source staleness, saved versions, composition/failure drain barriers, manual fallback, bounded local model output, cancellation and intentional model-busy refusal.

The exact RPM passed **21 translation checks** in disposable extraction and **16 whole-book checks with 1,000 scenes**. The extracted user-folder payload and shortcut-selected current payload each passed **22 translation checks**. The probe creates manual translation text, saves/reopens exact Unicode, edits the original through the real native editor, confirms the stale notice and retained wording, checks save-on-close, verifies no extra manuscript revision, and tests stacked compare controls in a narrow writing pane. System-theme screenshot was inspected. All stores were synthetic and network namespaces disabled. Original and translated text are separate; DOCX integration is owned by its separate slice.

RPM `117a937a9da1fa55d5d0d1a1961569b767304d7406bf1965fbd44520be386145` is 11,045,448 bytes. Companion/helper/chooser/notice bytes matched the isolated build; main binary differs only in the three-byte Tauri RPM bundle marker. Public-key-only detached verification passed. Atomic user-folder selection retained the prior payload; system RPM and personal stores were not touched. Source identity was checked immediately before selection. Local model translation language quality, physical input/IME and clean-host installation are not established.

Earlier shallow-hook fixtures failed because they did not traverse the translation component; stricter ID validation exposed a decimal ID mock. Parallel provider fixtures also raced the intentional one-inference gate. These fixtures were corrected with valid deterministic IDs and test-only async serialization, retaining a separate busy-refusal case. An isolated contract test initially lacked cached serde libraries and passed after read-only access to the existing debug cache. Package equality checks were adjusted to verify the exact known bundle marker. Failed attempts are not counted as passes.

## Saved Word export styles — October 10, 2026

The final source suite passed **589 frontend tests in 55 files** and **218 locked offline Rust tests**, with one opt-in OS-keyring case ignored. Generated contracts, TypeScript/Vite, debug and production builds passed. Nine storage profile tests cover strict fields/ranges, read-only defaults, CAS/stale binding, conflicts/resolution, archive/sync/reopen, checksum/invalid payload and projection mismatch. Converter tests verify four styles, complex-script properties, Letter/A4/margins and source-only paragraph spacing; invalid source metadata remains refused even when a profile overrides spacing. UI lifecycle tests retain failed-save drafts, guard old requests, permit clean Recovery drains and confirm discard; a coordinator regression verifies unique sibling keys.

The exact **11,045,448-byte** RPM, SHA256 `7c010a2ba86f41c7525aded723c46cf833a5c71883cb8f112ce437af71bfd27a`, passed **22 profile/native export checks** and **16 two-window dirty-draft export checks**. The profile check exercises visible controls, the precise stale-head error, reload, one mounted panel, unchanged manuscript heads, stored style history and independently checked final DOCX bytes. Initial probe attempts exposed stale DOM capture, readonly invoke instrumentation and an incorrect schema query; these were corrected without weakening profile checks. The real duplicated-panel bug was fixed with a distinct React key.

The synthetic custom A4/20 mm/double-spacing fixture passed independent ZIP/XML and exact 19-paragraph LibreOffice roundtrip. A 117,972-byte two-page PDF was rendered and both pages inspected. Typography, Arabic marks, Chinese, body spacing and chapter break were visible; emoji/private-use glyph coverage remains limited by installed fonts. No QA PDF is offered as a manuscript deliverable.

Extracted/shortcut-selected 1,000-scene writing checks passed **23/24**, and whole-book checks **16/17**. Shortcut-selected optional translations passed **22** checks against this RPM. Package bytes/resources/notices, public-key-only detached signature and scoped offline-namespace egress passed. Atomic publication selected `7c010a2ba86f41c7`, retaining preceding `117a937a9da1fa55` and earlier builds; personal stores/system RPM were unchanged. Evidence is under `work/export-styles-*`, outside tracked source. Stock GTK chooser, Microsoft Word, physical keyboard/IME, clean-host installation and power-loss remain separate qualifications.
