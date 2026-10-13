# Rust notebook 0.4.0 — October 13, 2026

Current candidate: `target/rust-bundle/novel-workspace-rust-0.4.0-linux-x86_64.tar.gz` and `novel-workspace-rust-0.4.0-1.x86_64.rpm`. Includes cutting-room scraps, a clean reader and reactions, passages to revisit, and checked return bookmarks. Original source is Apache-2.0; LICENSE, NOTICE and dependency notices accompany the package. The public GitHub release is a Linux x86_64 prerelease.

Publication follows workspace checks, the real GTK workflow, exact-payload packaging checks and the same GTK workflow against packaged editor/storage workers. The user-folder update retains the previous installation, shortcuts and custom icon; a running app receives the change after saving, closing and reopening. No personal store is opened by validation, and no system package install is performed.

Final artifact hashes are published in SHA256SUMS and the external 0.4.0 release manifest to avoid a package hashing itself. Clean-host runtime/physical-input acceptance remains separate.

---

# Rust notebook 0.3.4 novel startup repair — October 10, 2026

The current distributables are `target/rust-bundle/novel-workspace-rust-0.3.4-1.x86_64.rpm` and `novel-workspace-rust-0.3.4-linux-x86_64.tar.gz`. New novel now creates and opens an editable first chapter, and the permanent sidebar window hint is removed. The new create_novel RPC preserves existing bare-project and universe behavior. No dependency or schema migration is added.

Publication requires the expanded GTK workflow with packaged release workers and six exact-payload checks. The user-folder update preserves the Notebook shortcuts and custom icon, retains the previous payload and atomically replaces changed application files. A running app loads the change after saving, closing all Notebook windows and reopening.

Final hashes and observations are outside packaged docs in `novel-workspace-notebook-oct10-0.3.4-release.json` and `work/notebook-0.3.4-installation.json`. Tests use synthetic disposable stores; physical input and clean-host/system installation remain separate.

---

# Rust notebook 0.3.3 chapter writing repair — October 10, 2026

The current distributables are `target/rust-bundle/novel-workspace-rust-0.3.3-1.x86_64.rpm` and `novel-workspace-rust-0.3.3-linux-x86_64.tar.gz`. Chapter clicks now open writing; empty chapters provide an explicit Start writing action. It retains the prior theme/focus fixes and manual saving. No dependency or storage migration is added.

Publication requires the six exact-payload package checks and the expanded GTK workflow with packaged release workers. The user-folder update preserves the existing Notebook shortcuts and custom icon, retains the previous payload and replaces application files atomically. An already running app loads the change after saving, closing and reopening.

Final hashes and observations are recorded outside packaged docs in `novel-workspace-notebook-oct10-0.3.3-release.json` and `work/notebook-0.3.3-installation.json`. Tests use synthetic disposable stores. Physical input and clean-host/system installation remain separate.

---

# Rust notebook 0.3.2 background repair — October 10, 2026

The current distributables are `target/rust-bundle/novel-workspace-rust-0.3.2-1.x86_64.rpm` and `novel-workspace-rust-0.3.2-linux-x86_64.tar.gz`. This release completes the notebook's light surfaces so dark desktop themes cannot supply black menu or tab backgrounds. It retains the 0.3.1 focus-crash repair, manual saving and separate-window behavior. No dependency or storage migration is added.

Publication requires the same six exact-payload package checks. The compiled GTK workflow separately verifies 51 surface/contrast checks across three themes and runs with packaged release workers. The user-folder update keeps the Notebook desktop/app-menu shortcut and custom icon, retains the previous application payload, and replaces changed application files atomically. A running app continues using its existing executable; save, close and reopen it to load the update.

Final hashes and observed checks are outside the packaged docs in `novel-workspace-notebook-oct10-0.3.2-release.json` and `work/notebook-0.3.2-installation.json`. Personal stores, the older app, desktop theme settings and the system RPM remain unchanged. The personal RPM has no publisher signature; physical input and clean-host/system installation remain unqualified.

---

# Rust notebook 0.3.1 repair — October 10, 2026

The corrected distributables are `target/rust-bundle/novel-workspace-rust-0.3.1-1.x86_64.rpm` and `novel-workspace-rust-0.3.1-linux-x86_64.tar.gz`. This release fixes the synchronous active-scene focus panic; see TEST_RESULTS.md. It adds no dependency or storage migration.

Publication requires the existing six package checks against this exact release: executable/library/document equality, notices/launcher validation, RPM/archive equality and digests, actual editor operations, companion saves/conflicts/archives, and isolated release-main startup with store integrity. The extended GTK workflow separately uses the delivered release workers and synthetic stores.

The Notebook installation retains its separate desktop/app-menu shortcut and custom notebook-and-pencil icon. The previous executable is retained before atomic replacement. Final artifact hashes, package results, installed executable equality and preserved icon/launcher hashes are recorded in the external `novel-workspace-notebook-oct10-0.3.1-release.json` / installation report, avoiding self-referential packaged hashes. Personal writing and the older app installation are outside this repair. The existing personal RPM is unsigned; physical input and clean-host/system installation remain unqualified.

---

# Rust notebook package — October 10, 2026

The version 0.3.0 distributables are `target/rust-bundle/novel-workspace-rust-0.3.0-1.x86_64.rpm` and `novel-workspace-rust-0.3.0-linux-x86_64.tar.gz`. They contain the native Rust main, companion/editor, icon/launcher, updated repository documentation and collected Rust notices. No Python/JavaScript runtime, model, private font or manuscript is included.

`tests/rust_package_test.py` checks extracted executables, libraries and documentation bytes; RPM/archive equality and digests; notices; real release-editor Unicode/Undo/checkpoint operations; real companion saved/stale/archive operations; and the actual release main's isolated startup and SQLite integrity. The compiled GTK workflow separately uses the packaged release workers with temporary stores. Its checks cover the manual-save notebook and additional windows; release-main startup alone is not a complete UI qualification.

All six package-check groups passed, and the native notebook workflow passed with the packaged release companion/editor. Saved A4 Word styles and actual DOCX output also passed.

Detailed sizes, hashes and executed checks are external in `work/rust-package-checks.json` and the delivered notebook release manifest. This avoids self-referential packaged hashes. No publisher signature is added to this personal Rust RPM. Packaging does not install it or change the shortcut-selected older build. Source coverage and remaining specialized features are in RUST_PORT.md; current usage is in NOTEBOOK_DESIGN.md.

Physical mouse/keyboard/IME, stock chooser interaction, accessibility and clean-host/system installation remain separate. Historical artifacts and their signatures below do not qualify this version.

---

# Desktop package validation

Updated October 10, 2026. Saved Word export styles are published through the main shortcuts. Final RPM is **11,045,448 bytes**, SHA256 `7c010a2ba86f41c7525aded723c46cf833a5c71883cb8f112ce437af71bfd27a`. The shortcut-selected payload is `~/.local/opt/novel-workspace/builds/7c010a2ba86f41c7` via `current`. Publication retained the immediately preceding `117a937a9da1fa55` translation build and earlier DOCX build. System RPM and personal stores were unchanged.

## Saved Word export styles verification — October 10

589 frontend tests in 55 files and 218 locked offline Rust tests passed, with one opt-in OS-keyring case ignored. Contract, TypeScript/Vite, debug and offline production builds passed. Native editor/Pango code was unchanged by this slice; earlier native Python results remain historical.

| Workflow | Extracted package | Shortcut-selected build |
|---|---|---|
| Saved profile controls, stale export refusal, reload and Word export | 22 checks | Exact payload equality; chooser tested in disposable extraction |
| Export with an unsaved second native window | 16 checks | Exact payload equality; chooser tested in disposable extraction |
| Everyday writing, 1,000 scenes | 23 checks | 24 checks |
| Whole-book writing, 1,000 scenes | 16 checks | 17 checks |
| Optional translation lifecycle | Existing translation evidence retained | 22 checks against this exact RPM |

The style probe verifies the specific stale-head refusal, reloads the newer saved profile, confirms exactly one panel, saves custom A4/20 mm/double-spacing typography, then checks actual DOCX ZIP/XML against the profile and saved manuscript. Styles and export leave scene revision heads unchanged; prior style versions remain in history. The UI screenshot was inspected with all four style groups visible and no horizontal overflow. An earlier real desktop run exposed identical style/Recovery React keys; the unique-key fix and regression passed. Failed probe/timing/SQL attempts are not counted as passes.

The independent multilingual fixture retained 19 exact ordered paragraphs through LibreOffice DOCX import/export. Its 117,972-byte, two-page A4 PDF was rendered and both pages inspected. Arabic/Chinese, bold/italic/underline, heading hierarchy and explicit chapter break remain visible. Installed fonts omit fixture emoji/private-use glyphs while exact Unicode survives the DOCX roundtrip. Fonts are referenced, never bundled or embedded; Microsoft Word and stock GTK chooser interaction remain unqualified.

RPM digests, source/companion/helper/chooser/notice equality, main-binary bundle marker, executable modes, icon/launcher and packaged long-draft helper checks passed. All WordCraft notices remain; inventory is 371 packages with no missing notice files. Public-key-only detached signature verification passed with fingerprint `3CD4327251AF53BDF697E31741D20465D16DCEBC`. This is a personal-build identity, not external publisher trust. Scoped dynamic egress observation passed in an offline namespace; it does not constitute a complete syscall audit.

Only the chooser in each disposable extraction is substituted; the original helper hash and unchanged remaining files are recorded. No release probe hook is packaged. User-folder publication verifies gates and exact bytes, selects atomically, runs isolated installed-copy checks and rolls back on failure. Personal windows were not stopped. Reports are under `work/export-styles-*`. The signed distributable is `outputs/novel-workspace-export-styles-oct10-0.1.0.x86_64.rpm` in the parent output directory. Usage and limits are in DOCX_EXPORT.md.

## Previous WordCraft export slice — October 10

Updated October 10, 2026. The first WordCraft slice adds **Project → Export Word (.docx)**. Final RPM is **10,945,238 bytes**, SHA256 `c86f66d94e59c5e3bfe1956937446ed32d7a8648da3a6d85ab81173383660e20`. The main shortcuts select `~/.local/opt/novel-workspace/builds/c86f66d94e59c5e3` through `current`; the prior `1a77ae4328f00945` build remains available. System RPM and personal writing stores were not changed.

## October 10 verification

565 frontend tests in 50 files and 192 Rust tests passed, with one opt-in keyring test ignored. Contract, TypeScript/Vite, debug and offline production builds passed. The native Unicode/Pango editor source was unchanged; its prior Python results are historical evidence.

| Workflow | Extracted package | Shortcut-selected build |
|---|---|---|
| Word export with pending multilingual input | 15 checks | Exact payload equality; export command tested in disposable extraction |
| Word export with an unsaved second native window | 16 checks | Exact payload equality; export command tested in disposable extraction |
| Everyday writing, 1,000 scenes | 23 checks | 24 checks |
| Whole-book writing, 1,000 scenes | 16 checks | 17 checks |

Both packaged export probes verify actual DOCX bytes against the saved project snapshot, persisted revision heads and database integrity. The single-window probe also retains all prior archive history. The two-window probe establishes an unsaved peer draft before export and requires its exact multilingual text in the exported snapshot. Only the chooser in each disposable extraction is substituted with a fixed synthetic destination; its original SHA and unchanged other payload files are recorded. Stock GTK chooser interaction is not qualified.

Independent OOXML and LibreOffice checks retained 19 exact ordered fixture paragraphs, Chapter 0, language/direction, marks, spacing and empty/hard-break paragraphs. Two rendered pages were inspected. Installed fonts omit the fixture emoji/private-use glyphs, though exact Unicode survives the DOCX roundtrip. Microsoft Word itself and physical keyboard/IME remain untested.

RPM digests, source/companion/helper/chooser/notice equality, main-binary bundle marker, executable modes, icon/launcher and packaged long-draft helper checks passed. All three vendored WordCraft crates retain their notices; inventory is 371 packages with no missing notice files. No upstream fonts, renderer, dictionaries, model, manuscript, secret or test hook is bundled. LibreOffice is a test tool and is not a runtime dependency.

Detached signature verification passed using a public-key-only temporary profile, fingerprint `3CD4327251AF53BDF697E31741D20465D16DCEBC`. This is a personal-build identity, not an RPM header signature or external publisher trust. Scoped dynamic egress observation passed in an offline namespace; it is not a complete syscall/network audit.

The user-folder publication verified all gates, atomically selected the new build and ran isolated installed-copy workflows with automatic rollback on failure. Personal windows were not closed and personal writing was not opened. Reopen through the usual shortcut to use the update. Reports are under `work/docx-*`; the signed distributable is `outputs/novel-workspace-docx-oct10-0.1.0.x86_64.rpm` in the parent task output directory. Source usage/limits are in DOCX_EXPORT.md.

## Previous verified slice — October 6, 2026

Updated October 6, 2026. Chapters can start at 0 for a prologue, with checked name/order/numbering saves and durable numbering across reopening, archives and sync. Line spacing now uses font-size-based leading and adds no extra paragraph gap; Arabic marks retain a stable initial baseline and remain visible beyond compact line boxes. Universe → Series → Novel and shared world notes remain available. The RPM is 10,464,149 bytes, SHA256 `1a77ae4328f009451c4b273529aa95e0efdd54dacaf085944d4ccc79325c9106`. Main shortcuts select the byte-matched user-folder payload at `~/.local/opt/novel-workspace/builds/1a77ae4328f00945`. The preceding `19fff51f1730f6b1` build remains available; the system RPM remains older. Android stays deferred.

## October 6 verification

559 frontend tests in 49 files, 173 Rust tests (one opt-in OS-keyring fixture ignored), and 95 native Python editor/model/layout/checkpoint/resume tests passed. Generated-contract, TypeScript/Vite and offline production RPM builds passed.

| Workflow | Extracted package | Shortcut-selected build |
|---|---|---|
| Chapter 0 / back to 1 / new chapters / mixed-script 1.15 / Book / full screen | 12 checks | 13 checks |
| Same workflow, real 760 px GTK window | 12 checks | 13 checks |
| Everyday writing / exact Unicode / split, 1,000 scenes | 23 checks | 24 checks |
| Whole-book / character cuts / park and resume Undo, 1,000 scenes | 16 checks | 17 checks |

The chapter workflow exercises the real Edit form with 0, durable numbering, the outline and full project projection, reopening the form, restoring start-at-1, appending another chapter, and the Book heading. Original scene source/revision equality is checked before synthetic typing. Selected short mixed-language paragraphs save exact text and 1.15 spacing. Normal/full-screen and 760 px screenshots were inspected. Installed checks launch the complete shortcut-selected payload against private synthetic stores and verify its bytes against the independently extracted RPM.

Native storage tests cover numbering defaults, persistence, order, invalid inputs, stale metadata/order/numbering heads, sync conflicts, archive roundtrips and rollback after mutation. Chapter positions stay nonnegative; chapterNumbering is strict revisioned project data. Recovery can resolve competing numbering explicitly. Shared universe features remain from the preceding verified build; their full hierarchy/branch/layout probes were not rerun for this hash.

Real Pango checks measure 1.15 baseline advances of 20.70 px for 18 px English and 26.45 px for 23 px Arabic; mixed fallback fonts can add a few pixels. Separate paragraphs add no gap. Tests cover all four spacing choices, Enter inheritance, current-paragraph scope, Undo, incremental kasra/stacked-mark baseline stability and exact whole-Pango pixel equality at viewport edges. A constant font-size guard keeps the first baseline stable. Compact spacing can allow densely stacked marks to overlap; glyphs are not clipped or rewritten.

RPM digests/resources passed. Companion/helpers/file dialog/notices match release/source bytes; the main binary differs only by its RPM marker. No manuscript, secret, model, author font or test hook is packaged. The packaged 54,000-character helper retains exact input, vertical navigation, caret painting and acknowledged shutdown. GTK/WebKit, system Python/PyGObject/Pycairo/regex, Pango/Cairo and Fontconfig remain required; LibreOffice is not required.

Detached signature verification passed independently with only the public key, fingerprint **3CD4327251AF53BDF697E31741D20465D16DCEBC**. This is a personal-build identity, not an RPM header signature or external publisher trust. Scoped egress observation passed in an offline namespace; direct/uninstrumented syscalls are outside that observation.

The selected path is `$HOME/.local/opt/novel-workspace/current`; existing shortcuts use `current/usr/bin/novel-workspace`. No personal store, administrator authentication, system RPM transaction or personal-window shutdown was used. Reopen through the usual shortcut to use the update. Evidence is under `work/chapter-zero-*-oct6`, `work/chapter-numbering-*-oct6` and `work/novelist-line-spacing-*-oct6`, outside the source archive.

Synthetic input does not qualify physical keyboard/IME, dragging, accessibility, clean-host, power-loss or human usability. No new uninstrumented processing benchmark or general 3 ms guarantee is claimed. Whole-universe export/hierarchy merging, unavailable custom-font and second-PC/private Tailscale checks remain separate. Android stays deferred.
