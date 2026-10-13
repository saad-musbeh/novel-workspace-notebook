# Rust port backlog update — October 10, 2026

The native Rust interface/editor, local save/recovery core and distributable build are implemented. See RUST_PORT.md for the completed coverage map and specialized controls still needing native equivalents. Preserve their data/backend while implementing those controls; previous React completion evidence is not native GTK acceptance evidence.

Remaining native convenience work includes paragraph alternatives, character cuts, reader rehearsal/ripple, custom shortcuts/formatting sequences/desk preferences, automatic translation and review decisions, complete peer setup/sync controls and hierarchy/custom-asset tooling. Physical IME/accessibility, clean-host install/update and actual private-network acceptance remain separate gates.

---

# Ordered Development Backlog

Version 1.0 · October 2, 2026. The table records the original backlog; current partial completion evidence is maintained in PROJECT_STATE.md. Dependencies identify minimum predecessors, not permission to skip the milestone gate. Priorities: P0 = foundation/release blocker; P1 = required feature; P2 = polish that can be deferred with an explicit decision.

| ID | Milestone | Priority | Work item | Depends on | Completion evidence |
|---|---|---|---|---|---|
| NW-001 | M0 | P0 | Inventory PC and record phone software/input methods | — | Actual environment report and explicit unknowns |
| NW-002 | M0 | P0 | Verify prerequisites and initialize local repository/toolchain | NW-001 | Recorded versions, lockfiles, clean setup notes |
| NW-003 | M0 | P0 | Build minimal Tauri shell on Linux and real Android device | NW-002 | Both devices launch the shell |
| NW-004 | M0 | P0 | Spike English/Arabic/Chinese editing and direction controls | NW-003 | IME, selection, undo, and mixed-direction test results |
| NW-005 | M0 | P0 | Spike local SQLite persistence through Rust on both devices | NW-003 | Save/read/restart fixtures pass |
| NW-006 | M0 | P0 | Test supplied invented-alphabet mapping/font round trip | NW-004, NW-005 | Document actual supported encoding/direction behavior |
| NW-007 | M0 | P0 | Record stack go/no-go and toolchain baseline | NW-004, NW-005, NW-006 | Written decision and unresolved defects |
| NW-008 | M1 | P0 | Implement project/chapter/scene schema and migration owner | NW-007 | Schema fixtures, foreign-key checks, upgrade test |
| NW-009 | M1 | P0 | Implement revision envelope and transactional change markers | NW-008 | Crash/dedup tests and invariant checks |
| NW-010 | M1 | P0 | Create/open/edit scene and explicit save state | NW-009 | End-to-end local writing works |
| NW-011 | M1 | P0 | Add recovery history and safe undo/snapshot workflows | NW-010 | Recovery to last acknowledged commit |
| NW-012 | M1 | P1 | Add navigation, exact search, and language-aware statistics | NW-010 | Mixed-script query and count fixtures |
| NW-013 | M1 | P0 | Add complete project archive export/import | NW-011 | New-project restore verifies hashes/content |
| NW-014 | M1 | P1 | Add text/Markdown/HTML manuscript exports | NW-010 | Direction/format loss documented |
| NW-015 | M1 | P0 | Execute safe-writer release gate | NW-011, NW-012, NW-013, NW-014 | No known loss of acknowledged data |
| NW-016 | M2 | P1 | Add story records and custom-field definitions | NW-015 | Character/location/item templates preserve values |
| NW-017 | M2 | P1 | Add character/event-dependent state and fact approval | NW-016 | Facts, beliefs, rumors, and proposals remain distinct |
| NW-018 | M2 | P1 | Add freeform cards, groups, and typed edges | NW-016 | Canonical links survive moving/removing cards |
| NW-019 | M2 | P1 | Add main/subplot lanes and shared event memberships | NW-018 | One event appears in multiple lanes without copying |
| NW-020 | M2 | P1 | Add chronology constraints and manuscript-order separation | NW-019 | Flashback, unknown-time, relative-time fixtures |
| NW-021 | M2 | P1 | Add manual continuity issue workflow | NW-017, NW-020 | Evidence and statuses persist offline |
| NW-022 | M2 | P1 | Add mobile board list and touch connection workflow | NW-018 | Works without hover/precision drag |
| NW-023 | M3 | P0 | Package companion lifecycle and narrow local IPC | NW-015 | Single DB owner; safe restart and migration lock |
| NW-024 | M3 | P0 | Add pairing, device tokens, project authorization, revocation | NW-023 | Unpaired/revoked requests rejected |
| NW-025 | M3 | P0 | Implement revision push/pull and cursor protocol | NW-009, NW-024 | Retry, dedup, missing-parent, and epoch fixtures |
| NW-026 | M3 | P0 | Implement conflict inspection and explicit resolution | NW-025 | Concurrent scene edits preserve all heads |
| NW-027 | M3 | P0 | Implement verified/resumable asset transfer | NW-025, NW-006 | Corrupt/missing/incomplete assets remain visible |
| NW-028 | M3 | P0 | Add tombstones, edit/delete conflict, restore/re-pair handling | NW-026, NW-027 | No silent resurrection or stale-cursor data loss |
| NW-029 | M3 | P0 | Configure private Tailscale Serve access and native HTTP adapter | NW-024 | Private authenticated service; no raw model exposure |
| NW-030 | M3 | P0 | Test away-from-home sync over mobile data | NW-028, NW-029 | Real-device interrupted/retried/conflict run |
| NW-031 | M4 | P1 | Add offline language-pack manifest/import and glossary | NW-015 | Licenses/checksums/versioning and missing-pack UI |
| NW-032 | M4 | P1 | Add dictionary/synonym lookup and replacement preview | NW-031 | Sense distinctions and undo verified |
| NW-033 | M4 | P1 | Implement deterministic continuity rule registry | NW-021 | Positive and negative fixtures for each rule |
| NW-034 | M4 | P1 | Integrate PC-local LanguageTool adapter | NW-023 | Explicit language, local endpoint, capability tests |
| NW-035 | M4 | P1 | Build unified suggestion/evidence/staleness UI | NW-032, NW-033, NW-034 | Accept/reject/undo and stale anchors work |
| NW-036 | M5 | P1 | Benchmark candidate PC-local models by language | NW-001, NW-035 | Model/license/hash/resources/quality report |
| NW-037 | M5 | P1 | Integrate local runtime adapter and bounded inference jobs | NW-036 | No cloud fallback; cancellation/error handling |
| NW-038 | M5 | P1 | Implement source-scoped prompts and evidence validation | NW-037 | Invalid quotes/IDs cannot become verified findings |
| NW-039 | M5 | P1 | Add proposed fact extraction and explicit approval | NW-017, NW-038 | AI never silently changes canon |
| NW-040 | M6 | P1 | Add Android pending requests and immutable input snapshots | NW-030, NW-035 | Unsynced phone text is not replaced by old PC text |
| NW-041 | M6 | P0 | Add durable remote jobs, idempotency, restart, and results | NW-037, NW-040 | Disconnect/retry/restart state-machine tests |
| NW-042 | M6 | P1 | Add phone PC-proofreading and PC-LLM task UI | NW-034, NW-041 | Clear scope/status/provider; phone has no inference |
| NW-043 | M6 | P0 | Test stale results and concurrent edit application | NW-042 | No stale patch silently overwrites newer writing |
| NW-044 | M7 | P0 | Audit egress, import sanitization, credentials, backup settings | NW-043 | Offline run and allowed-destination audit |
| NW-045 | M7 | P0 | Produce signed personal builds and protect signing material | NW-044 | Install/update without data loss on actual devices |
| NW-046 | M7 | P1 | Profile large fixtures and fix responsiveness regressions | NW-045 | Hardware-specific measurements, not guesses |
| NW-047 | M7 | P0 | Run full backup/restore, migration, and two-device drill | NW-045, NW-046 | Reproducible release-gate report |
| NW-048 | M7 | P1 | Publish personal v1 notes and next-priority decision | NW-047, NW-049 | Known limits, verified versions, clean project state |
| NW-049 | M7 desktop | P1 | Add workspace tabs with Open in new window and Move back to main window | NW-010, NW-023 | Shared companion; isolated editor sessions; exact Arabic/Chinese text, undo and view state survive handoff; safe per-window/app close; conflicts and multi-window responsiveness verified |

## Stop conditions

Do not advance past an unresolved data-loss defect. Do not substitute browser-only tests for real Arabic/Chinese input on the target devices. Do not install a paid dependency to bypass a missing free feature. Do not start remote AI before authentication and local job persistence are tested.

A working non-AI application is a valid intermediate release. Further model tuning should not hold basic writing or synchronization hostage.

## Added desktop requirements — October 5, 2026

| ID | Status | Work | Acceptance |
|---|---|---|---|
| NW-050 | Implemented; packaged checks passed, physical pending | Scrivener/Scribeist-style split writing, compact binder and optional scene inspector | Two different scenes editable side by side; independent scrolling, undo and saves; keyboard/mouse divider; safe tab/window handoff and close; exact multilingual packaged regression |
| NW-051 | Implemented; physical pending | Browser-style tabs, automatic empty-source closure, configurable Word-style shortcuts and mouse selection, adjustable line spacing | Source closes only after committed last-tab transfer; remaps preserve composition and shortcut conflicts; spacing survives save/reopen; native clipboard and physical input acceptance recorded separately |
| NW-052 | Implemented; packaged/user-folder checks passed, physical pending | Remove LibreOffice backend; implement application-owned Unicode editor with Pango/Cairo shaping/rendering | Exact original JSON on no-edit open; independent undo/session ownership; Arabic/kasra and bidi selection; rich styles and saved spacing; long-draft/resize/close/save/reopen tests; no LibreOffice runtime dependency |
| NW-053 | Implemented; packaged checks passed, physical pending | Configurable Word-style Alt+X Unicode conversion | Unicode scalar validation; hexadecimal/character reversal; extended letters/emoji/private-use/combining marks; invalid inputs remain unchanged; exact undo/save/reopen |

## Current desktop coverage — October 6, 2026

The writing page uses one toolbar and an actions menu, with no header or bottom action strips. A single draft has no duplicate tab strip. Fill pane uses the available width at the original text size. The eight Writing desk additions remain available. The RPM is 10,380,624 bytes, SHA256 `d98fa740ae7e75683d3b78cd492eb6d7f180a34c071970c7fcc2e5989695e1f7`. Main desktop/menu shortcuts select the byte-matched user-folder payload at `~/.local/opt/novel-workspace/builds/d98fa740ae7e7568`. The preceding `11bd5b1a8c75cdca` build remains available; the system RPM remains older. Android stays deferred. Source verification passed **543 frontend tests in 48 files**, **161 Rust test executions** plus one ignored opt-in keyring test, and **90 Python editor tests**. Generated-contract, TypeScript/Vite and production RPM builds passed. All exact package and installed-copy probes passed; PACKAGE_VALIDATION.md records them. Physical input and second-PC/private transport remain open.

The original 48 IDs are retained for traceability. NW-049 was added on October 5 for required desktop tabs and detachable windows; the final desktop release notes depend on its acceptance. “Completion evidence” describes an acceptance target, not an execution result. Android and phone-only work are excluded from the current continuation. Implemented code and passed synthetic checks do not mark a physical-device or signed-release gate passed.

| IDs | Desktop status and remaining acceptance |
|---|---|
| NW-001–007 | Fedora environment/toolchain/native shell and synthetic multilingual engine/storage checks executed. Actual keyboard/IME, accessibility and supplied custom-font gate remain open; Android excluded. |
| NW-008–014 | Integrated schema 3 migrations, immutable revision/change markers, navigation/search/counts, history/snapshots, complete archives and text/Markdown/HTML exports. Native synthetic file delivery and reopen verified. |
| NW-015 | SQLite capacity failure and killed-process migration/import/export/snapshot drills passed on synthetic stores. Physical input, filesystem ENOSPC/power loss and installed release recovery remain. |
| NW-016–021 | Story bible, typed fields/facts, canonical cards/edges, shared plot events, chronology constraints and manual issues integrated with model/validation fixtures. Physical board/accessibility checks remain. |
| NW-022 | Phone-specific delivery excluded; desktop board list/navigation exists. |
| NW-023–028 | Single-owner companion, local IPC, auth/revocation, bounded push/pull, conflicts, resumable assets, tombstones and epochs implemented; real temporary-store and loopback HTTP fixtures passed. |
| NW-029–030 | Explicit private listener exists. Native loopback peer pair/sync/retry/conflict/asset/older-backup tests passed with fake credentials. Native HTTPS/address/certificate fixtures and the approved real synthetic keyring roundtrip passed. Actual Tailscale configuration/away-from-home/unattended unlock drill remains; phone drill excluded. |
| NW-031–035 | Pack/glossary import, lookup/replacement preview, deterministic continuity rules and optional local LanguageTool adapter integrated. Actual LanguageTool 6.6 seed/clean pilot passed in English/Arabic; Simplified Chinese clean input succeeded and Traditional Chinese was unsupported. Broad language quality and licensed real pack selection remain separate. |
| NW-036–039 | Actual pinned local-model benchmark and bounded revision/evidence pipeline implemented. All four clean controls produced false positives; reliable semantic quality gate failed. Fact proposals require explicit atomic source-bound approval. |
| NW-040–043 | Android/remote phone submission and UI excluded. Desktop durable jobs, retries, cancellation/interruption and stale atomic apply are implemented and tested. No remote inference endpoint. |
| NW-044 | CSP, literal loopback provider validation, bounded import/auth and secret exclusions implemented. Historical RPM dynamic egress probe observed zero intercepted INET socket/connect/sendto/getaddrinfo calls across five instrumented processes and six startup markers. Full OS/unattended-unlock/clean-install audit unverified; Android backup settings excluded. |
| NW-045 | Current October 6 RPM passed all extracted workflow, resource, detached public-key signature and scoped egress checks. User-folder byte equality and offline workspace/whole-book checks passed; main shortcuts select the final payload. System RPM update/recovery and trusted publisher signing remain pending; see PACKAGE_VALIDATION.md. |
| NW-046 | Release English 250,000-word and Chinese 750,000-character storage/archive/reopen fixtures executed. Native extracted 1,000-card/999-edge functional checks passed; physical typing/frame/resource/cold/low-disk gates remain. Strict 3 ms guarantee unmet. |
| NW-049 | Implemented: bounded window/tab/session ownership, exact route reuse, independent application-owned editor sessions and undo, rollback, drain-before-Refresh with CAS, handoff and close guards. Earlier LibreOffice debug/RPM 33/30 checks remain historical. The final October 6 package passed folded book-window (32), return-transfer (41) and original-close (32) checks. Physical multi-window qualification remains pending. |
| NW-047–048 | Synthetic backup/migration/reopen and archive restore evidence exists; final installation/recovery/private transport acceptance remains open. See DESKTOP_RELEASE_NOTES.md for personal build notes. |

SQLite BLOB assets currently deviate from the original external-hash-file architecture. No automatic pruning, cloud fallback, persistent service or Android setup is included. The current priority is to resolve desktop defects, validate the exact release artifact and complete external desktop checks; further model tuning does not block non-AI writing.


## Accepted additions — October 6, 2026

| ID | Status | Work / remaining acceptance |
|---|---|---|
| NW-053 | Implemented, packaged and accepted | Typing priority with bounded batching and cooperative background work. The author accepts about 72 ms for the 54,000-character single-paragraph stress case; further extreme-case optimization is optional. |
| NW-054 | Implemented and packaged | Read-only pinnable per-draft scene reference pane. |
| NW-055 | Implemented and packaged | Independent fictional occurrence and authored reader-reveal timelines. |
| NW-056 | Implemented and packaged | Explicit character knowledge, sources/scopes, frozen fact snapshots and stale/uncertain labels. |
| NW-057 | Implemented and packaged | Common-point manuscript/bible preview and selective new-revision recovery with exact point identity/CAS. |

Current build and detailed acceptance are in PROJECT_STATE.md and PACKAGE_VALIDATION.md; the October 5 build notes above remain historical. Physical/private-network gates remain open and Android stays deferred.

## Selected desktop story workflows — October 6

| Item | Source state | Acceptance evidence |
|---|---|---|
| What-if branches | Implemented; final packaged workflow passed | Exact bounded project forks, baseline comparison and atomic selected scene/whole-bible bring-back fixtures |
| Whole-book writing | Implemented; final packaged workflow passed | Bounded body loading, native shared scroll viewport, per-scene saves and parked Undo |
| Ripple preview | Implemented; final packaged workflow passed | Zero-save preview/Cancel, exact Apply, stale-head/IME/navigation guards |
| Character cuts | Implemented; final packaged workflow passed | Explicit metadata links, manuscript order, linear scans, failed-drain preservation |
| Paragraph alternatives | Implemented; final packaged workflow passed | Durable rich originals/versions, single Undo, exact anchor/head checks and failed metadata saves |
| Reader rehearsal | Implemented; final packaged workflow passed | Future reveal values absent until authored progress, reordered scenes and missing-link fixtures |

See STORY_WORKFLOWS.md for behavior and limits. The final RPM passed 24 story-workflow, 16 whole-book/character-cut and 26 book-window assertions. The published user-folder payload passed 30 offline workspace and 17 whole-book checks. No inference turns prose into canon or character presence. Android remains deferred.

## Writing desk UX — October 6

| Addition | Source state | Package acceptance |
|---|---|---|
| Hold-to-peek references / outline | Implemented; F6 default, transient read-only pane | Passed exact RPM / user-folder checks |
| Same-window navigation trail | Implemented; Alt+Left/Right, exact draft anchors and book reading/filter state | Passed exact RPM / user-folder checks |
| Whole-book scene folding | Implemented; save/park before collapse, retained Undo/handoff state | Passed exact RPM / user-folder checks |
| Tab / binder pane placement | Implemented; drag and context-menu controls, existing detach/return | Passed exact RPM / user-folder checks |
| Writing width / responsive docks | Implemented; bounded writing width, wide Notes/Reference beside text, narrow overlays; Peek stays transient | Passed exact RPM / user-folder checks |
| Saved-revision comparison | Implemented; F7, 12 paragraphs/page, 2,000-character excerpts, insertion-shift limit | Passed exact RPM / user-folder checks |
| Formatting command sequences | Implemented; eight slots × 32 actions, per-step acknowledged saves, refusal stops replay | Passed exact RPM / user-folder checks |
| Interface mirroring | Implemented; UI direction independent of paragraph direction | Passed exact RPM / user-folder checks |

WRITING_DESK.md describes usage. All eight additions and the compact writing page passed package and user-folder checks. Physical input and external gates remain separate; see PACKAGE_VALIDATION.md.


## Navigation and everyday actions — October 6

Implemented and packaged: metadata-only scene/chapter title search, independent outline scrolling, durable New chapter/New scene, visible Find/Notes/Reference/Split/Focus, direct second-scene selection, compact title and readable theme controls. The exact RPM and installed payload passed the everyday workflow, including Unicode text, saving and Undo. Human usability acceptance remains open; Android remains deferred. See WRITER_WORKFLOW.md and PACKAGE_VALIDATION.md.


## Compact tool pages — October 6

Book, Planning, Review, Project and Alphabets have no separate top bars or centered page margins. Outline and More are integrated into their existing controls. More retains Back/Forward, Desk and Commands; Review/Project retain the scene selector and its save-before-switch guard. Page actions also remain available during loading/error states. Panel and form spacing is reduced; manuscript line spacing is unchanged. Review and Alphabets follow the neutral shell palette. Alphabets uses readable captions, wrapping symbol cells and a single-column narrow form, with the outline beside rather than over that form at 760 px.
Exact package and installed-copy evidence is in PACKAGE_VALIDATION.md. Android and human/physical/external acceptance remain separate.


## Chapters and shared universes — October 6

Use Edit beside a chapter to change its name and manuscript number. Changing the number moves it and renumbers the other chapters; Cancel leaves saved metadata unchanged. Saving drains pending writing first and preserves the mounted draft. Stale metadata is refused; any partially completed legacy name/order operation is reported rather than hidden.

The + beside the novel selector opens Library. Create a universe, a series, then a novel, or add an existing unassigned novel. Universe/series groups also appear in the selector. New novel creation commits its separate project, first chapter/scene and catalogue membership together. The universe/series line opens shared World notes; Reference shows those notes beside writing alongside the novel’s own scene references. Shared records have one saved source and never silently overwrite a novel’s planning or canon. Individual project exports/sync remain separately scoped; unavailable novels remain explicit, and whole-universe export/hierarchy merging is not included.
