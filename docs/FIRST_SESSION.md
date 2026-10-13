# First notebook session

Use the Rust 0.3.0 build described in [SETUP.md](SETUP.md).

1. Choose **Project** in the left area selector, then **New novel**.
2. Choose **Write → Add & arrange → New chapter**, then **New scene**.
3. Write on the white page. Use **Save / Ctrl+S** to keep changes.
4. Click a scene or page to open it in the same tab. An edited page asks **Save / Discard / Cancel** before you leave.
5. Choose **Planning**, **Review** or **Project** to change the left menus. The current page stays open until you choose another page.
6. Middle click a page/scene for another window. Drag a top tab out to move its existing draft. **Tabs & windows → New tab** creates an additional tab.

See [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md) for conflicts, saving, additional windows and exports. The historical acceptance-session notes below concern the retained interface.

---

> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Next desktop acceptance session

Updated October 5, 2026. Resume the integrated desktop implementation; do not restart the old editor spike or initialize Android. Read PROJECT_STATE.md and TEST_RESULTS.md first. Use synthetic projects for regression work and preserve acknowledged personal writing.

1. The earlier system RPM remains installed and its isolated offline checks passed. The author closed the “Desktop checks — temporary test project” windows normally; its synthetic store passed integrity checks. Arabic/kasra was reported steady and long-draft typing responsive before/after maximizing. The application-owned Pango/Cairo package passed exact extracted offline, split/Unicode, transfer/close and resource/signature checks. Its user-folder installation and updated shortcuts passed isolated payload/offline checks. The author became remote, so system authentication and physical checks were deferred. Resume the open private “New editor checks — temporary project” fixture for the specific `يس`/mixed-direction defect, adjustable spacing, split panes, browser-style tab movement and remappable shortcuts. Verify acknowledged save/close/reopen; keep personal data separate.
2. Have the author freshly exercise the replacement with real English/Arabic/kasra input, Alt+X, resize, selection, Undo and clipboard. Verify direction, save/close/reopen and accessibility, and record the keyboard/runtime versions and any defect. Chinese IME is unavailable on this PC; its physical gate remains open. Synthetic engine fixtures alone do not pass these physical gates.
3. No invented-alphabet font/sample is currently available. When one is supplied with permission, verify mapping versions, original code points, real glyph coverage/direction/shaping, private process registration, archive import and transliteration losses.
4. The main shortcut currently launches `$HOME/.local/opt/novel-workspace/current/usr/bin/novel-workspace`; a user menu override masks the older system RPM. After a deliberately authenticated system update, either retain this qualified user install or retarget both launchers and the menu override to the newly verified system payload. Repeat the documented exact RPM extraction/offline checks after each rebuild, then perform explicitly authorized install/update/recovery checks. The personal detached signature is an integrity artifact, not a trusted installed-package result. See PACKAGE_VALIDATION.md. Retain a complete project archive separately before migration/release drills.
5. Configure/test real Tailscale Serve private HTTPS and an authorized peer only after deliberate setup. Verified native HTTPS/address policy and a synthetic real keyring roundtrip are already tested; real routing and unattended unlock remain. Exercise interrupted transfer, conflicting edits/deletion/reorder, revoke and epoch/re-pair. No public Funnel, raw model proxy or automatically enabled service.
6. Extend the actual small LanguageTool pilot to broader language/variant quality, and evaluate any chosen local model separately. The current Qwen3-4B benchmark fails clean controls; do not present its questions as proven continuity findings. Writing should remain useful with both providers stopped.
7. Measure release-build interactive large-fixture behavior, memory, cold/slow/low-disk and forced-process recovery. Preserve source hashes/versions and distinguish engine time from total input latency; strict 3 ms is not guaranteed.

Update ACCEPTANCE_TESTS.md only with observed evidence, and keep external unrun gates explicit. Resolve any data-preservation defect before release. Whole-database replacement, installs, services, downloads and networking changes require the existing explicit authorization scope; ordinary source fixes and synthetic local tests can proceed autonomously.
