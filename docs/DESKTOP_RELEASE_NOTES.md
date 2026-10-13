> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Novel Workspace 0.1.0 — desktop personal build

Updated October 6, 2026. Chapters can start at 0 for a prologue, with checked name/order/numbering saves and durable numbering across reopening, archives and sync. Line spacing now uses font-size-based leading and adds no extra paragraph gap; Arabic marks retain a stable initial baseline and remain visible beyond compact line boxes. Universe → Series → Novel and shared world notes remain available. The RPM is 10,464,149 bytes, SHA256 `1a77ae4328f009451c4b273529aa95e0efdd54dacaf085944d4ccc79325c9106`. Main shortcuts select the byte-matched user-folder payload at `~/.local/opt/novel-workspace/builds/1a77ae4328f00945`. The preceding `19fff51f1730f6b1` build remains available; the system RPM remains older. Android stays deferred.

The writer now uses an application-owned text model with installed Pango/PangoCairo shaping and Cairo rendering. Runtime requirements are GTK/WebKit, system Python/PyGObject, Pycairo, regex and Fontconfig. No model, font, language pack or service is automatically downloaded or started. Existing system document-editor installations are unaffected.

The model preserves original Unicode and rich document metadata and saves immutable revisions locally. The two-pane writer keeps separate editor sessions, undo and scrolling. Caret, long-draft, split reflow, save/close and package fixtures passed; physical Arabic/IME/clipboard and compositor resize still need fresh checks; a strict 3 ms processing bound is not guaranteed.

Scenes and tools open in tabs. Right-click a tab or press Shift+F10 for Save, Open in new window, Move back and Close. Drag tabs to reorder or move between windows. Moving the final tab closes its empty source window after the destination is ready. Opening an already open scene reuses its existing tab. Moving retains the native editor’s undo, caret, selection and scroll position. Quit Novel Workspace saves all windows first. Other tabs offer Refresh after shared changes, preserving pending drafts until they can be saved.

Choose Open beside from a scene or tab menu to edit two different drafts side by side. Resize their divider with the mouse or keyboard; Single view returns to one editor and keeps both tabs. Each draft has separate scrolling, undo and saves. The primary outline searches chapter/scene titles and directly creates chapters and scenes. Notes opens synopsis, status, viewpoint, tags and event links beside the draft on wide windows. These controls follow the requested Scrivener/Scribeist workflow. Earlier packaged split/Unicode checks passed; final October 6 reruns are recorded in PACKAGE_VALIDATION.md. Outer writing margins and page/toolbar/footer padding are removed; controls retain usable hit areas.

F1 opens keyboard and mouse shortcut settings. Assign, clear or reset Word-style commands; conflicting assignments are refused. The editor supports underline, literal Find Next/Replace Selection and explicit clipboard actions. Format includes saved line spacing, with Ctrl+1, Ctrl+5 and Ctrl+2 defaults for single, one-and-a-half and double spacing. Alt+X toggles a selected or preceding hexadecimal Unicode code and its character; U+ in the toolbar invokes the same action. Invalid conversion reports a status message without stopping writing. Shortcut additions preserve saved remaps. Rendering depends on the chosen font; complete Unicode glyph coverage is not promised, and the reported specific `يس` gap needs a physical recheck.

The interface collapses paragraph settings and optional setup/review diagnostics while keeping writing controls and errors visible. Writing loads a project index and its selected scene instead of all manuscript bodies. Scene/chapter indexes and memoized continuity reduce repeated work in large projects; caret-follow retries respect manual scrolling.

This build adds bounded large-board rendering, keyboard reveal/focus, paged lists and connection controls, canonical plot search, and reuse of unchanged review hashes. Recovery snapshots publish only after private staging and integrity verification. Optional backup or sync setup failures leave writing available. Complete project archives import into a separate project and preserve history, planning and permitted assets.

Native peer connections support manual scoped loopback or verified Tailscale Serve HTTPS pairing and bidirectional revision/asset sync. Tokens remain in the OS keyring; a locked keyring has no plaintext fallback. Interrupted transfers resume in bounded passes. Restoring an older database cannot silently reuse newer sync progress. Native TLS verifies platform trust/hostname and pins only private Tailscale DNS answers. The approved synthetic OS keyring roundtrip passed. Real private-network availability is recorded separately in SETUP.md; no Tailscale configuration or persistent listener was enabled here.

Review tools include local packs, deterministic continuity checks and explicit revision-bound decisions. LanguageTool's English/Arabic pilot passed its seed/clean controls; Traditional Chinese was unavailable. Qwen3 remains experimental because all four clean controls generated false positives. Review output never silently edits writing or approves story facts.

Before updating an installed system, export a complete project archive to a separately chosen destination. Same-disk snapshots do not protect against disk loss. History, backups and private editor-session files are retained without automatic pruning. No whole-database automatic replacement is provided.

The package manifest includes dependency notices and a lockfile inventory. Final package contents and the detached signature are checked against the exact artifact; a personal signature does not establish a trusted publisher. Physical IME/clipboard/accessibility, permitted custom fonts, unattended keyring unlock and actual Tailscale transfer still need their own checks. See ACCEPTANCE_TESTS.md for observed and remaining gates.


## October 6 story workflows

- Project → What-if branches creates a separate project/history copy. Compare saved versions and explicitly bring back one mapped scene or the whole story bible as a new revision. No automatic merge or deletion propagation occurs.
- Book writes scenes in manuscript order with one outer scroll viewport, bounded lazy bodies, independent saves/Undo and offscreen session parking. Its character selector uses authored links and edits the same scenes, without prose-based presence guesses.
- Planning previews fact/event-time ripples before committing them. Review exact before/after values and linked dependencies, then Apply or Cancel. Stale previews and unresolved dependent deletions are refused.
- Write → Paragraph alternatives retains a rich original and saved versions. A swap validates the exact scene/paragraph anchor; one Undo restores the previous passage.
- Planning → Reader rehearsal steps through manuscript progress. Only authored reveals and applicable notes through that point are shown; future values remain hidden. Missing and uncertain links stay explicit.

Reference pin/follow, separate Story time/Reader learns timelines, sourced character knowledge and the Project time machine remain available. Recovery creates new revisions and retains newer history; knowledge keeps its recorded property/value pair when canon changes. Usage and bounds are in SCENE_TOOLS.md and STORY_WORKFLOWS.md.

The preceding story-workflow release passed 460 frontend, 170 Rust and 84 core Python checks. Its 26-check Book-window result is historical. Current Writing desk and full-screen results are below and in PACKAGE_VALIDATION.md. Physical keyboard/IME, permitted custom fonts, accessibility and second-PC/private transport remain unqualified.


## Compact writing page and full width

Frontend verification passed **550 tests in 48 files**, including seven added creation/filter/navigation regressions. Generated-contract, TypeScript/Vite and production RPM builds passed. The unchanged native/storage baseline remains **161 Rust test executions** plus one ignored opt-in keyring test, **90 Python editor tests** and **three companion stdio integration tests**; those suites passed during the preceding test audit.

The eight additions are available; see WRITING_DESK.md. F11 toggles full screen, F6 peeks references/outline, F7 peeks the chosen saved revision, Alt+Left/Right restore previous places, and F1 configures shortcuts and recorded formatting sequences. Fold/Unfold parks native Undo before collapse. Drag tabs or binder scenes onto pane targets; tool tabs can sit beside drafts. Desk sets writing width and interface direction. Fill pane removes side columns. Bounded native rendering now supports 4096 px width and 2160 px height with unchanged font sizes; old parking checkpoints remain compatible. Current-host 1920×1080 full-screen single/split checks passed.

Synthetic DOM input does not qualify physical keyboard/IME or mouse dragging. The current-host production full-screen transition passed at 1920×1080; other monitors, physical typing/compositor interaction, accessibility, clean-host/power-loss/low-disk and second-PC/private Tailscale acceptance remain separate. Chinese IME and a permitted custom-alphabet sample are unavailable. The accepted approximately 72 ms giant-paragraph result is unchanged; a general 3 ms processing guarantee is not established. Android remains deferred.

Test maintenance removed duplicate/obsolete checks and separated explicit GUI diagnostics. See TEST_SUITE.md for current commands and the retained resize diagnostic limitation.


## Scene-first navigation

Find scene filters the outline by title without loading manuscript bodies or moving the draft. Clear returns the full tree. New scene saves pending writing and opens a durable scene in the selected chapter; New chapter keeps the current draft open. The tree scrolls independently. Outline, Notes, Reference, Split, Find and Focus are visible; Format holds paragraph controls and More holds advanced actions. The Split chooser opens a chosen scene directly. Wide Notes/Reference panes dock beside the text; narrow windows use bounded overlays. The neutral palette, compact title and custom project selector passed package contrast/geometry checks. See WRITER_WORKFLOW.md.


## Compact tool pages — October 6

Book, Planning, Review, Project and Alphabets have no separate top bars or centered page margins. Outline and More are integrated into their existing controls. More retains Back/Forward, Desk and Commands; Review/Project retain the scene selector and its save-before-switch guard. Page actions also remain available during loading/error states. Panel and form spacing is reduced; manuscript line spacing is unchanged. Review and Alphabets follow the neutral shell palette. Alphabets uses readable captions, wrapping symbol cells and a single-column narrow form, with the outline beside rather than over that form at 760 px.
Exact package and installed-copy evidence is in PACKAGE_VALIDATION.md. Android and human/physical/external acceptance remain separate.


## Chapters and shared universes — October 6

Use Edit beside a chapter to change its name and manuscript number. Changing the number moves it and renumbers the other chapters; Cancel leaves saved metadata unchanged. Saving drains pending writing first and preserves the mounted draft. Stale metadata is refused; any partially completed legacy name/order operation is reported rather than hidden.

The + beside the novel selector opens Library. Create a universe, a series, then a novel, or add an existing unassigned novel. Universe/series groups also appear in the selector. New novel creation commits its separate project, first chapter/scene and catalogue membership together. The universe/series line opens shared World notes; Reference shows those notes beside writing alongside the novel’s own scene references. Shared records have one saved source and never silently overwrite a novel’s planning or canon. Individual project exports/sync remain separately scoped; unavailable novels remain explicit, and whole-universe export/hierarchy merging is not included.
