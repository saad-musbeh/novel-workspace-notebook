# Current notebook redesign acceptance

Version 0.3.0 uses the minimal notebook workflow in [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md). The native synthetic workflow checks Save / Discard / Cancel, manual-save inactivity, ordinary page replacement, top-tab switching, middle-click windows, draft/Undo/form transfer, moved tool actions, conflicts, per-window closing and the shared companion. User usability acceptance and physical mouse/keyboard/IME remain separate.

The historical tests below describe the retained React/Tauri interface and are not current split/autosave requirements.

---

# Native Rust acceptance — October 10, 2026

The current application has a Rust GTK interface and Rust Unicode/Pango/Cairo editor. Follow TEST_SUITE.md for current automated checks and RUST_PORT.md for coverage differences. The native workflow checks synthetic multilingual input, exact persistence, independent split Undo, shared Book ownership, planning/exports, navigation and retained failed-save drafts/refused closure.

Physical keyboard/IME, accessibility, clean-host install/update, custom author fonts, power-loss/low-disk and real second-computer private sync are not established. Outstanding specialized native controls are documented in RUST_PORT.md. No older webview/package test or checked box below qualifies the Rust interface.

The checklist below is retained historical evidence for the React/Tauri and earlier editor implementations.

---

# Desktop acceptance and regression checklist

Updated October 5, 2026. The current desktop editor uses an application-owned Unicode model and Pango/Cairo shaping/rendering; LibreOffice runtime adapters are removed. Android is excluded. Exact replacement-RPM update and physical-input checks remain pending. Earlier RPM and LibreOffice observations below are historical and do not qualify the replacement editor. Checked boxes cover only the stated execution. TEST_RESULTS.md records observed runs.

## Replacement editor — current source coverage

- [x] 303 frontend tests, 150 Rust executions (one opt-in keyring test ignored), 16 model tests, 8 engine tests, 16 Pango layout tests and 31 native editor contracts passed.
- [x] Exact no-edit JSON, Unicode preservation, selection/formatting, undo/redo, guarded literal replacement and configurable Alt+X scalar conversion have synthetic coverage. Invalid conversion preserves the draft; visible glyphs depend on installed fonts.
- [x] Two different drafts can be edited side by side with independent scroll, undo and save state; divider, compact binder and optional inspector are implemented.
- [x] Visible pane width drives native reflow; writing-area padding and split sizing are implemented. Resize preserves the logical passage and exact document, with rollback on rendering failure.
- [x] Exact replacement RPM: 10,166,588 bytes; SHA256 `4d0a3eb6f94b12d16a958dab23f217bd2172e0895dcbf797df558fb146b7d87f`. Offline 1,000-scene 29 checks passed in 11.936 s; split 32 in 8.622 s; return-transfer 41 in 5.661 s; original GTK close 32 in 4.914 s.
- [x] Packaged source/helper/resources match; 54,000-character fixture, Up/Down, snapshot, undo and EOF checks passed. Egress probe self-test passed: six startup markers, zero intercepted INET attempts. Detached signature verified with the public key; trusted publisher signing is separate.
- [x] Debug long-draft, close and workspace/computed-theme checks passed in 50.715 s, 0.647 s and 4.479 s.
- [ ] Native outer-window resize qualification failed: requested width 760 remained 1200 under both default and forced X11. Source and packaged pane reflow passed; actual fullscreen/physical resizing remains pending.
- [x] Exact replacement payload installed in `~/.local/opt/novel-workspace/builds/4d0a3eb6f94b12d1`, selected by the `current` symlink and existing desktop shortcut/user-menu override. Payload byte comparison and 30 offline 1,000-scene checks passed in 11.619 s against this user-folder copy; no personal store was opened.
- [ ] System RPM update and recovery deferred while the author is remote. Administrator installation was stopped before DNF began; the system RPM remains the older build. A new-build private synthetic fixture is open for later physical checks.
- [ ] Fresh physical Arabic/mixed-direction keyboard, clipboard, selection, resize and undo/reopen checks on that RPM.
- [ ] Chinese IME, author-supplied permitted font and second-PC/private-network acceptance; required external setup is unavailable.
- [x] Author acceptance of the giant-paragraph stress case: approximately 72 ms processing for one 54,000-character mixed-script paragraph is acceptable (October 6, 2026). This is acceptance of the measured limit, not a passed general 3 ms guarantee or physical typing qualification.

## Writing and storage

- [x] Stable project/chapter/scene IDs; rename, reorder and move preserve canonical references in synthetic stores.
- [x] Exact multilingual rich-text save/reopen and two-newline paragraph projection; no stored Unicode normalization.
- [x] Stale scene/project-data writes reject; immutable revisions/markers and injected failed transactions preserve earlier data.
- [x] Native synthetic close drains queued input and commits before exit; active composition/save failure are handled in queue tests.
- [x] Original prototype data/history survive v1/v2 upgrades with consistent pre-migration snapshots; newer unsupported schema rejects.
- [x] Exact search and separate scalar/grapheme/whitespace/CJK statistics; replacement preserves marks and checks exact evidence.
- [x] Archive validates schema/paths/counts/size/checksums/ancestry; new-project import remaps canonical identities and retains all history/verified assets.
- [x] Native synthetic selected-file export/import preserves exact archive bytes; text/Markdown/HTML export tests document losses and language/direction metadata.
- [x] Daily-startup/manual consistent SQLite snapshots and retained deleted-scene/history recovery fixtures.
- [ ] Replacement editor physical Arabic acceptance pending. Historical LibreOffice installed app: physical Arabic keyboard sample `مِمِمِ سلام محمد` has joined letters and stable kasra, confirmed by the author on October 5. This is only that sample’s joining/stability result.
- [ ] Broader physical English/Arabic/Simplified and Traditional Chinese typing, candidate selection/cancellation, selection, copy/paste, undo and reopen on the Fedora desktop. Chinese IME is unavailable on this PC.
- [ ] Mixed-direction caret/punctuation and actual IME close/background behavior, accessibility and resize reflow.
- [x] Synthetic SQLite capacity failure and owned-process interruption at migration/import/export/snapshot boundaries preserve acknowledged content; no partial snapshot is published.
- [ ] Physical filesystem ENOSPC/power loss and installed release recovery at durability boundaries.
- [ ] Clean-install archive/backup recovery drill and separately chosen off-disk backup destination.

## Planning, alphabets and local tools

- [x] Custom fields preserve values; cards/typed edges reference canonical objects; shared event memberships and manuscript/story-order separation have fixtures.
- [x] Unknown dates/beliefs/proposals remain distinct from confirmed facts; manual issues and explicit constraints persist.
- [x] Mapping versions are immutable, original text preserved, canonical scene links remapped in archives; font path/checksum checks are implemented.
- [x] Pack identity/version/license/checksum validation, missing-pack state, dictionary/glossary/synonym distinctions and deterministic rule positive/negative fixtures.
- [ ] Author-supplied permitted custom font: real glyph coverage, direction, shaping, exact reopen/export and transfer on the intended desktop.
- [x] Large-board canonical search, retained focus, indexed viewport/SVG and 50-item list/connection bounds pass automated fixtures.
- [ ] Physical pointer/keyboard/accessibility and dense-overlap chronology workflows in the release app.
- [x] Actual LanguageTool 6.6 English/Arabic seed and clean-control pilot, Simplified Chinese clean request and explicit Traditional Chinese refusal; evidence valid and writing unchanged. See PROOFREADING_EVALUATION.md.
- [ ] Broad real proofreading accuracy, regional English spelling and Chinese error-detection/variant coverage beyond the small pilot.

## Private sync and recovery

- [x] Synthetic pairing replay/wrong identity/expiry, project allowlist, revocation and restored-hub epoch checks.
- [x] Two temporary stores and real loopback HTTP preserve concurrent heads, deduplicate retries and refuse corrupt/missing-parent/reused identities.
- [x] Scene, explicit all-head scene deletion, generic data and metadata conflicts have multiparent resolution; reorder-then-rename converges causally.
- [x] Tombstones/edit-delete conflict and restore are retained; fresh replica bootstrap does not silently resurrect scenes.
- [x] Interrupted assets resume after reopen; offsets/retries/checksums/finalization are bounded and verified.
- [x] Listener requires explicit session enable; wildcard/public/LAN bind is refused. No model/shell passthrough exists.
- [x] Native loopback peer pairing/manual bidirectional sync/retry/conflicts/resumable assets and older-backup refusal pass with isolated fake credentials.
- [x] Native Tailscale Serve HTTPS policy and ephemeral TLS pair/revision/asset roundtrip; wrong SAN, expired/untrusted certificates and redirects reject. No production trust override.
- [ ] Actual Tailscale configuration, private Serve HTTPS and transfer/retry/conflict/re-pair drill away from home.
- [x] Approved real native OS keyring synthetic store/read/remove roundtrip passed without inspecting existing credentials.
- [ ] Unattended unlock/locked-session behavior and real peer credential revoke across desktop sessions; server hashed verifier files are separate.
- [ ] Actual transport/resource/egress audit and independent protocol review, including private deployment permissions and rate limits.

## Optional assistance

- [x] Durable frozen review scope, idempotency/retry/cancel/interruption, timeout/error and no-cloud fallback fixtures.
- [x] Exact unique quotations derive trusted revision/hash/UTF16/grapheme anchors; fabricated/ambiguous/stale evidence rejects.
- [x] Scene patch and approved-fact commit check target and supporting current unconflicted revisions atomically.
- [x] Real companion save/load acknowledges while a fake provider HTTP response is blocked.
- [x] Actual local Qwen3-4B benchmark records exact-evidence validity, unchanged writing, language samples, resources and latency.
- [ ] Reliable semantic continuity quality: currently failed, with false-positive questions on all four clean controls. Do not qualify the model as a contradiction detector.
- [ ] Real provider cancellation/resource behavior and application responsiveness under the chosen release workload; mock tests and one small benchmark do not cover every load.

## Desktop tabs and detachable windows — NW-049

The detailed debug/RPM observations in this section precede the editor replacement; repeat packaged checks for the current engine.

- [x] Pure Rust ownership registry: 16 cases cover bounded 8-window/64-tab ownership, exact identity reuse, transitions and close/handoff failure guards.
- [x] Editor-session fixtures: 10 executions cover bounded 16-session ownership and retained exact text/undo/caret with canonical-binding refusal and rollback.
- [x] Workspace coordinator: 26 cases cover exact routes, drain-before-navigation/Refresh, retained drafts on failure, acknowledgement callbacks, no hidden tool-tab editor, library/review scene selection/history, content-error forwarding and unchanged/changed alphabet-load remount guards. Three separate route cases cover identity and handoff readiness.
- [x] Latest isolated debug default/original-close fixtures passed 33/30 checks, including real owned GTK close, continued detached exact saving, one sampled companion, native composition refusal without extra child/revision and background-view retention. Window-specific events/listeners are label-scoped; shared-content and whole-app quit cancellation remain broadcasts.
- [x] Exact final RPM isolated multi-window 33-check and original-close 30-check runs passed in 8.250 s/6.438 s; five post-close samples showed one companion. This accepts those synthetic slices, not physical input or multi-window latency.
- [ ] Physical keyboard/IME actions, exact Arabic/Chinese composition, selection, scroll and undo through detach/return; moving without edits creates no manuscript revision.
- [ ] Installed release: destination failure/timeout/binding mismatch leaves source usable; retries do not duplicate tabs or edits, and acknowledged titles remain current while closing.
- [ ] Installed release: independent input/cursor/bitmap/save responses with one companion owner; explicit Refresh preserves drafts and retains concurrent shared-data conflicts.
- [ ] Installed release: closing one window leaves others usable, including after the original window closes; Quit drains all windows or refuses composition/save failure safely.
- [ ] Measure input, memory, resize/fullscreen behavior with two or three long Arabic/Chinese windows. Source avoids hidden review controllers and whole-project hashing in writing tabs; this is not a measured resource or latency guarantee.

## Shared contracts

- [x] Eleven storage wire types and 32 direct RPC signatures generate from Rust; serializer compatibility fixtures passed. Custom/dynamic operations retain runtime validation.
- [ ] Generate/check RPC operation signatures and the opaque DocumentNode schema/runtime validator from a shared source.

## Desktop release

The completed RPM observations below describe earlier builds. Current replacement-RPM extracted checks are recorded above; installed update, recovery and physical acceptance remain pending.

- [x] Earlier RPM digests, extracted sidecar/helper content, executable permissions, launcher/icon and declared runtime dependencies checked against that release build. See PACKAGE_VALIDATION.md.
- [x] Final NW-049 RPM built and hashed (10,116,035 bytes); its packaged helper preserves the exact 54,000-code-unit fixture. See PROJECT_STATE.md and PACKAGE_VALIDATION.md.
- [x] Exact final RPM offline 1,000-scene run passed 28 checks in 12.146 s, including tab/window menus, light/dark/system contrast and the 1,000-card/999-edge/archive/exit workflow.
- [x] Dynamic egress probe: five instrumented processes, six startup markers, zero intercepted INET socket/connect/sendto/getaddrinfo calls; no observed destinations/resolver inputs. Direct syscalls, static/hidden libc/noninterposed paths and helpers dropping preload are uncovered.
- [x] Exact final RPM detached personal-build signature independently verified with its public key; protected private key outside source. This is not trusted publisher or embedded RPM signing.
- [ ] Required installed-release checks against that hash, including trusted RPM signing and installation/update/recovery.
- [x] Locked dependency notice inventory contains no missing notice files; exact notice resources are bundled. This is metadata/notice collection, not a formal legal or vulnerability audit.
- [x] Earlier extracted release independently passed no-edit startup, exact input/archive/large-board/exit checks with the owned process networking disabled.
- [ ] Signed install/update on intended Fedora host preserves acknowledged data; protect signing keys outside Git.
- [ ] Full application egress and pasted/imported-content audit; no telemetry/remote fonts/cloud grammar/public provider exposure.
- [ ] Cold/slow-disk and interactive large-fixture typing/save/search/board/native-memory measurements. No strict 3 ms whole-input guarantee is claimed.
- [x] Reproducible personal desktop release notes and observed limits are recorded; no acknowledged-data-loss defect remains in the executed fixtures.
- [ ] Complete the remaining installed-system, physical-input and private-network acceptance gates before claiming a final release.

The original Android input, backup policy, phone job delivery, APK and mobile-data acceptance checks remain future work in MASTER_PLAN.md, not waived or passed.
