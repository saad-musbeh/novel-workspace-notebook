# 0.3.2 — Consistent light notebook surfaces, October 10, 2026

- Fixed dark dropdown menus, text areas and empty tab backgrounds inherited from desktop themes.
- Apply the notebook palette to GTK menus/popovers, notebook content, nested text nodes, number fields and list rows; keep selections pale and readable.
- Give scrollbars and tooltips matching light backgrounds.
- Extended the real GTK workflow with opaque-background and 4.5:1 text-contrast checks under Breeze-Dark, Adwaita dark and Breeze light, including inactive and selected states.
- Capture Write, Planning, Review and Project views using synthetic data. Desktop theme settings and saved writing are unchanged.

# 0.3.1 — Active-scene crash repair, October 10, 2026

- Fixed the reproduced Rust borrow panic when an already-open scene receives keyboard focus through navigation or outline activation.
- Release the active-editor borrow before entering GTK; preserve the same draft, Undo session and manual-save state.
- Extended the real GTK workflow to deliver synchronous focus events without taking desktop focus, including repeated activation with unsaved text.
- The conflict test waits for the actual reloaded editor owned by its window, rather than an earlier retained context.
- Updated the Notebook build and documentation; retain its separate shortcut and generated icon. No storage migration or dependency addition.

# 0.3.0 — Notebook redesign, October 10, 2026

- White notebook pages, plain text controls, left area selector and collapsible menus.
- Manual saving with Save / Discard / Cancel before leaving an edited page, tab or window.
- Ordinary navigation replaces the active tab; middle click opens another window.
- Top tabs drag out with their actual draft, scroll/cursor and Undo state.
- Tool pages use the current tab; moved-page actions follow their current window.
- Additional windows replace within-window splits; each window closes independently.
- Sidebar Find/Replace, paragraph and plot-board controls; initial boards load automatically.
- Saved-project operations check all windows for that novel; stale drafts retain explicit conflict recovery.
- Updated setup, notebook, Rust-port and test documentation; existing schema and dependencies retained.

# Native Rust desktop 0.2.0 — October 10, 2026

- Added a Rust GTK interface, native windows/outline/scene tabs, split and detached drafts, expandable Book editing, structured project forms, draggable plot board and chronology.
- Ported the Unicode model, Pango/Cairo layout, bounded Undo, guarded editor protocol, private checkpoints and chooser from Python into Rust.
- Retained the existing immutable revision/CAS/conflict/recovery/archive/local-provider/private-sync/DOCX core and schema-3 data location.
- Added native shared-universe/series/novel tools, manual translations, saved-version/branch/time-machine previews and explicit scene-conflict selection.
- Kept stale drafts after refused saves and refused window closure until saving succeeds. Matched companion replies by ID so provider requests cannot block save replies.
- Switched default build/run/package to Rust; created RPM and relocatable Linux payloads with distinct names and Rust-only dependency notices.
- Updated setup, architecture, tests, state and packaging documentation. Kept earlier React/Tauri sources/evidence as reference.

See RUST_PORT.md for outstanding specialized-interface parity. No installed shortcut or personal writing was changed.

---

# Desktop build 0.1.0 — 4 October 2026

Desktop writing and planning are integrated. Android is excluded.

- Replaced the alternate web editor with the Arabic-compatible LibreOfficeKit writing surface.
- Added projects/chapters/scenes, scene details, paragraph controls, history, reviewed exact replacement and complete project archives.
- Added story records, plot boards, chronology lanes, facts/beliefs and manual/deterministic continuity checks.
- Added versioned alphabets and permitted font assets, lexical packs and safe optional local review.
- Added private scoped sync, concurrent-version handling, explicit edit/delete resolution, consistent snapshots and retained deleted scenes.
- Fixed standalone builds opening an absent development server; added visible startup/error recovery states.
- Removed repeated page banners, sample labels and filler; shortened action labels and moved secondary tools into keyboard-accessible disclosures. First launch starts with a blank draft.
- Fixed dark-theme navigation/header contrast and light-canvas labels; native computed-style regression covers the theme palettes.
- Fixed long-draft caret painting and scrolling at page boundaries, rejected stale viewport responses and removed printed-page shadows. Character counting now runs in a worker so whole-scene segmentation does not block the interface.
- Kept formatting controls and save status visible while scrolling the draft. Resize/full-screen changes now refresh the settled viewport, preserve the document position and limit painting to visible content. Fixed narrow layout after a hidden editor remount and avoided restarting autosave on unchanged geometry acknowledgements.
- Corrected reversed Up/Down native key mappings, including selection. Fixed the reproduced LibreOffice global autocorrect destructor crash during helper exit: explicit document/Kit cleanup and response flushing still run before the disposable helper exits. Save-before-close remains enforced by the desktop.
- Reused review hashes for unchanged scenes, reducing digest work on each saved scene in large projects.
- Added bounded board/list/connection rendering, keyboard navigation and reveal, focus retention and drag cancellation. Card search follows current canonical plot notes.
- Made manual and pre-migration snapshots private, validated and durable before publishing their final names. Added archive size/history preflight and interrupted-process/SQLite-capacity recovery checks.
- Kept local writing available when optional recovery or sync metadata is damaged.
- Included upstream dependency notices and a reproducible installed-dependency inventory in desktop resources.

This is a personal development release. See PROJECT_STATE.md and ACCEPTANCE_TESTS.md for unverified physical input, trusted package installation, private deployment, resource and quality gates. Qwen3 remains experimental after four clean-control false positives; LanguageTool's actual local English/Arabic pilot passed its small fixtures, with Traditional Chinese unavailable. A strict 3 ms whole-input limit is not guaranteed.

### October 4 optimization and cleanup

Indexed scene/source/chapter lookups, memoized continuity and navigator, and a single memoized review filter reduce repeated work. Paragraph options and optional sync/review diagnostics use disclosures; error, evidence and save controls remain visible. Removed an unused ProseMirror style. Caret-follow retries preserve manual-scroll precedence and survive refresh coalescing. Added current-head/project projection and queued-refresh regressions; 177 frontend tests pass. The exact RPM passed 27 offline UI/IPC checks with 1,000 scenes. Native size requests were not applied in the background session, so full-screen qualification remains open.

### October 5 desktop tabs and remote checks

Added scene/tool tabs, Open in new window, Move back to main window, explicit tab saving and coordinated window/app closing. Each writing tab owns an independent native editor session; a transfer retains its existing process, undo, caret and selection. Refused or timed-out transfers retain the source, and changed canonical scene/alphabet bindings refuse handoff rather than replacing its undo history. All windows share one storage companion.

Other tabs show acknowledged shared changes and offer an explicit Refresh that drains pending work before replacing projections. Writing tabs no longer create hidden review controllers or hash the entire project. Review and Project tabs can choose their scene context. Hidden editor checkpoints preserve finite viewport coordinates, and unchanged alphabet loads no longer trigger repeated editor remounts.

Nine Rust serialized workspace types now generate TypeScript definitions, with drift checks in the build and real serializer fixtures. Operation signatures and opaque rich-document validation remain separate work. Automated results and physical/installed-system limits are recorded in TEST_RESULTS.md and ACCEPTANCE_TESTS.md.

The final October 5 RPM passed 33 transfer/return, 30 original-close and 28 offline 1,000-scene workflow checks, including actual tab/menu contrast in light/dark/system themes. Its scoped dynamic network trace observed no intercepted destinations or resolver inputs; full OS egress and installed-system qualification remain open.


## October 6, 2026

- Added typing-priority background reads and bounded input batches with separate Undo steps and final full-paragraph shaping.
- Added pinnable scene references, separate authored reader reveals and character knowledge with frozen fact snapshots and uncertainty/stale labels.
- Added recorded-point manuscript/bible previews and atomic selective new-revision recovery, including deleted scenes and retained competing/newer history.
- Fixed inactive-tab contrast during dark-theme background transitions and kept recorded knowledge property/value pairs together after canon edits.
- Updated the main shortcuts to the verified user-folder build; Android remains deferred.

## October 10 — Saved Word export styles

- Added per-novel Word export presets and editable paper, margins, fonts, sizes, bold/italic and body spacing.
- Saved profiles use immutable history, CAS, archive/private sync and explicit Recovery; stale profiles refuse export.
- Profile changes leave manuscript revisions and editor formatting unchanged.
- Fixed duplicated style controls on Project refresh with distinct component keys; final packaged/native and installed-copy checks passed.
