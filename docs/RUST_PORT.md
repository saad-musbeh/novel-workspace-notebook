# Rust desktop port

Updated October 13, 2026. Version 0.4.0. The requested scope includes the interface: the active desktop is a native GTK application written in Rust. It does not embed the React page or launch a Python editor.

## Architecture

| Component | Source | Responsibility |
|---|---|---|
| Native desktop | `apps/native-writer` | GTK notebook windows, left menus, detachable top tabs, manual-save navigation, IME integration, project forms, plot board, recovery and export pages |
| Unicode editor | `crates/editor` | Immutable paragraph blocks, exact Unicode editing, grapheme/word navigation, rich formatting, bounded Undo, guarded paragraph/navigation commands, checkpoints |
| Text layout | `crates/editor/src/layout.rs` | Pango whole-paragraph shaping, bidirectional caret/selection geometry, reflow and Cairo viewport rasterization |
| Local worker | `crates/companion` | Exclusive database ownership, revision-bound requests, recovery, optional local providers and private sync |
| Persistence | `crates/storage` | Schema-3 SQLite, immutable revisions, expected-parent checks, conflicts, archives, branches and project data |
| Word export | `crates/docx` | Saved-snapshot conversion with the existing pinned WordCraft implementation |

The desktop communicates with workers over inherited private stdio. Blocking IO runs outside GTK. Companion replies are matched by request ID, so a long local-provider request does not prevent scene-save replies. Editor operations remain ordered within each scene session. No application network activity is needed for startup, writing or saving.

GTK/Pango/Cairo/Fontconfig remain dynamically linked system libraries. “Rust port” describes the application implementation; it does not mean the Linux graphics stack has been reimplemented in Rust.

## Data compatibility

The app keeps `$XDG_DATA_HOME/local.novel.workspace.spike/editor-spike.sqlite3`, normally `~/.local/share/local.novel.workspace.spike/editor-spike.sqlite3`. It uses the existing scene schema, project-data kinds, revision envelopes and schema-3 storage implementation. This port adds no storage migration and does not normalize manuscript text.

Only one companion may own a store. Close the previous desktop before opening the same database. Importing a complete archive creates a separate project. The port did not open or alter personal writing; tests use disposable stores.

The editor protocol retains scalar-value positions, rich block JSON, layout frames, exact selections and guarded captures from the previous helper. Rust checkpoint files preserve the existing schema-1 shared-block format. Checkpoint resume is implemented/tested in the engine; each GTK window currently keeps at most 16 live scene sessions and does not park or recover them automatically.

## Native workflow coverage

| Workflow | Rust interface |
|---|---|
| Scene writing | Exact multilingual input, rich marks, heading/paragraph language/direction/spacing, grapheme navigation, selection, clipboard, Alt+X, Find/Replace and independent Undo |
| Save safety | Manual Save / Discard / Cancel before leaving, snapshot after queued operations, expected-parent writes, retained failed drafts, per-window closing and saved-project checks across windows |
| Workspace | Left area selector and collapsible menus, searchable hierarchy, chapter opening and empty-chapter Start writing, ready-to-write novel creation, scene/chapter creation and renaming, ordinary page replacement, middle-click windows, draggable/closable/reorderable top tabs and scene Back/Forward |
| Chapter editing (source after 0.4.0) | Revision-checked title and number form, Chapter 0, chapter reordering/renumbering, manual-save navigation and detachable draft ownership |
| Manuscript overview | Chapter/scene navigation in manuscript order; one page per tab, additional windows in place of within-window splits; at most 16 live scene sessions per window |
| Cutting-room floor | Explicit kept selections and plain-text cuts from saved revision differences; text drag/drop or append to a scene draft, followed by manual Save |
| Reading and reminders | Saved read-only manuscript, selected Bored/Confused/Curious reactions, revision-checked passage navigation, unresolved scene notes and edit-save counts |
| Return bookmark | Explicit next-session reminder, scene/cursor/selection capture, exact document-hash check before restoration |
| Story bible | Native structured forms for records, custom fields, linked facts/events, reveals, knowledge and issues; stable identities remain hidden |
| Plot board / chronology | Draggable note cards, visible edges, editable board data; known/unknown/fixed/interval/relative times and explicit constraint findings |
| Library | Shared universe/series catalogue, atomic novel creation, existing novel membership and shared planning notes |
| Recovery | Saved history preview, snapshots, consistent backups, deleted-scene restore and common-point scene/whole-bible recovery with captured-head checks |
| What-if branches | Separate copies, comparison previews and explicit selected scene/whole-bible bring-back |
| Conflicts | Preview saved/current/local versions and choose a resolution. A stale unsaved local draft can be saved explicitly over the captured current head as a new revision; changing heads refuse the action |
| Translation | Manual drafts with immutable source language/revision/text; originals remain the exported manuscript |
| Local review | Explicit configured loopback provider, frozen saved source/hash, read-only result display; author edits remain manual |
| Word / portable exports | Saved snapshot, per-novel export-profile forms, original Unicode/rich marks/language/direction/spacing in DOCX; full project archives |
| Private sync | Session listener start/stop, status and saved-device/peer inspection. Existing authenticated backend APIs remain available |

## Writing-aid boundaries

The cutting-room floor reconstructs removed plain text from saved parent/child revisions and stores explicitly kept selections separately. Unsaved deletions remain in Undo until the author keeps a selection or saves an edit. Large changed spans are conservatively retained as the old region, which can include unchanged text. The view scans current manuscript scenes; deleted scenes use the existing recovery page. Insertion copies plain text into a normal unsaved draft and never removes its source scrap.

The reader displays saved plain text, with chapter/scene headings and scalar-value selection anchors. It does not live-update or preserve the editor's rich layout. Reactions bind to the selected quote and saved scene revision. Passage restoration refuses changed revisions, dirty drafts and mismatching quotes. Passages to revisit contains explicit unresolved reactions/notes plus scenes with at least two saved rewrites of existing text; it does not infer difficult prose or track typing behavior.

A return bookmark is an explicit action, not a close-time prompt. A bookmark per scene holds a reminder and an editor navigation capture; the latest saved bookmark is the novel’s return location on reopen. Exact document-hash validation guards restoration; changed text leaves the reminder intact and refuses the old position. This dedicated return action does not add exact restoration to general Back/Forward navigation.

Scraps, reactions, notes and bookmarks use the existing revisioned project-data mechanism, validation, archive/branch remapping and expected-parent checks. Their explicit save buttons do not change the manuscript's manual-save policy. No new schema migration, service or dependency is required.

## Interface differences and outstanding parity

The Rust interface is a new native implementation, not a pixel-for-pixel copy. Some specialized controls from the React workspace still need native counterparts: character-cut navigation, hold-to-peek, exact bookmarked Back/Forward restoration, paragraph-alternative management/swapping, reader rehearsal and ripple/rehearsal previews, shortcut remapping, recorded formatting sequences, saved writing-width/interface-direction preferences, scene reordering/moving between chapters, language-pack/font import, generated translation review and review Apply/Reject/job-history controls.

Full peer pairing, challenge entry, revocation and manual synchronization controls remain in the retained interface/backend. The GTK listener controls alone do not complete a remote pairing workflow. Project-data conflicts beyond scenes retain storage enforcement but do not yet have a complete GTK resolution panel.

Existing data for those features is preserved by storage and complete archives. Do not claim full convenience-feature parity or use the earlier webview workflow probes to qualify this interface. Earlier guides describe the retained React/Tauri implementation where marked.

## Build and packaging

`tools/run-desktop.sh` now builds and runs the Rust desktop by default. `--package` builds the three Rust executables and creates an RPM plus a relocatable Linux archive. The package uses distinct filenames:

- `novel-workspace-rust`: native application.
- `novel-companion-rust`: local companion.
- `novel-editor-rust`: text/layout worker and optional native chooser mode.

Development workers retain their Cargo names, `novel-companion` and `novel-editor`; the app accepts those sibling names when package names are absent. The retained Tauri shell also launches the Rust editor/chooser helper; its bundled Python resources/runtime requirements were removed. The React sources and Python model/layout/checkpoint files are kept for comparison and development, outside the Rust package.

The package includes the application Apache-2.0 LICENSE/NOTICE, documentation and the collected locked Rust notices. Third-party components retain their own terms. It includes no Node modules, web assets, Python sources, model weights, private fonts, manuscripts or probe hooks. It does not replace the previously selected user-folder build. See SETUP.md, DEPENDENCIES.md and PACKAGE_VALIDATION.md.

## Verification boundaries

Native editor tests exercise immutable blocks, exact rich-text edits, grapheme deletion, bounded Undo, failed-batch rollback, stale captures, checkpoint validation/privacy and viewport reflow. A comparison runs 147 operations against the prior helper and checks text/document JSON, selection, Undo state, layout dimensions/caret/selection geometry and checkpoint resume. It does not require PNG bytes or every error message to be identical.

The GTK workflow runs actual native widgets and worker processes with synthetic stores. It checks the notebook layout, manual saving and prompts, top-tab switching, middle clicks, scene/form/tool-page detach, independent windows, retained stale drafts and explicit conflict recovery. See [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md) and TEST_SUITE.md. It does not establish physical IME or screen-reader accessibility.

No new timing guarantee is claimed. The previous giant-paragraph measurements belong to the prior runtime and need a separate Rust benchmark. Custom font registration is implemented in the engine, but no permitted author font or physical qualification has been supplied.
