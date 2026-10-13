# Test suite

Run from the repository with existing dependencies. Fixtures must use synthetic documents and disposable stores.

## Native Rust checks

Prepare the three debug executables first. This also prepares the sidecars required by the retained Tauri workspace member:

```sh
tools/run-desktop.sh --build
cargo test --workspace --offline --locked -j 2 -- --test-threads=1
```

The suite covers the real storage/recovery/sync/provider/export APIs, the native editor and chronology. The opt-in OS-keyring test and current-display GTK workflow remain ignored in ordinary discovery.

For the actual GTK workflow, run separately on a graphical desktop:

```sh
G_DEBUG=fatal-criticals cargo test -p novel-native-writer --offline --locked -- --ignored --test-threads=1 --nocapture
```

This opens native windows without taking keyboard focus and uses a temporary database/cache plus real stdio workers. It verifies first-launch novel creation, Project and Library creation, Save/Cancel before creation, naming cancellation/validation, exact immediate writing/saving and draft preservation through screen refreshes, transactional failure without a partial novel, chapter opening, an empty Prologue, title search, chapter-specific first-scene creation, repeated/stale Start actions, delayed/closed-window callbacks, detached chapter actions, cancelled New scene creation, manual-save inactivity, exact multilingual saves and queued input, Save/Discard/Cancel, composition barriers, ordinary page replacement, guarded tab switching, an actual middle-button event, GTK drag-out routing and editor/Undo transfer, detachable forms, same-window tool pages and detached button routing, stale-save retention and explicit conflict recovery, guarded closing/project switching, the shared companion's lifetime and Word output. Actual synthetic screens are written to `work/notebook-new-novel.png`, `work/notebook-created-novel.png`, `work/notebook-empty-chapter.png`, `work/notebook-prologue-writing.png`, `work/notebook-writing.png`, `work/notebook-detached.png` and `work/notebook-planning.png`. Waits are bounded; only test processes/stores are affected.

To run the same native workflow against extracted **release workers**:

```sh
NOVEL_RUST_TEST_BINARIES="$PWD/target/rust-package/usr/bin" \
  G_DEBUG=fatal-criticals cargo test -p novel-native-writer --offline --locked -- --ignored --test-threads=1 --nocapture
```

That uses the compiled GTK test interface with the actual packaged companion/editor executables. It does not by itself establish an automated UI workflow against the release main executable. PACKAGE_VALIDATION.md distinguishes the separate release-main startup check.

After packaging, validate the archive/RPM and actual release main/workers:

```sh
python3 tests/rust_package_test.py
```

This extracts only the generated package into a disposable folder, compares executable/documentation bytes and RPM/archive payloads, runs release-worker Unicode/checkpoint/storage/archive requests, and launches the release main in isolated XDG directories without keyboard focus. It stops only its own process group. The startup check does not replace the GTK workflow or establish physical input.

## Editor comparison

The retained Python helper is an independent behavioral reference for this port:

```sh
python3 tests/rust_editor_parity_test.py
```

It needs the previous system Python GI/Pango/Cairo/regex setup and target/debug/novel-editor. The comparison exercises 147 deterministic/seeded operations and checkpoint resume, checking exact document JSON/text, positions, selections, Undo/format states, dimensions and Pango caret/selection geometry. PNG bytes and error text are not required to match. Python is a developer comparison dependency, not an application runtime requirement.

Editor-only tests:

```sh
cargo test -p novel-editor --offline --locked
```

They cover rich blocks, exact Unicode, graphemes, immutable neighbors/bounded Undo, size-guarded transaction rollback, exact replacement, stale captures, forged/private checkpoints and reflow.

## Optional retained-interface checks

`npm test`, `npm run build`, native_*_test.py and the JavaScript/WebKit release probes target the prior React/Tauri/Python implementation. They retain valuable regression/reference coverage, but are not substitutes for GTK checks and do not qualify the Rust package. Do not point an old probe at the Rust binary.

The retained Tauri shell now uses the Rust editor/chooser helper; a full legacy workspace build still needs its own WebKit/Node prerequisites. Generated RPC contracts remain relevant to its frontend.

## DOCX interoperability and optional services

Existing converter/storage tests remain part of the Rust workspace. A standalone fixture can be checked with existing LibreOffice:

```sh
cargo run -p novel-docx --offline --locked --example export_fixture -- \
  tests/fixtures/docx-project.json /tmp/synthetic-manuscript.docx
python3 tools/check-docx-export.py /tmp/synthetic-manuscript.docx \
  tests/fixtures/docx-project.json --libreoffice --artifacts work/docx-interop
```

That is separate interoperability evidence. Physical keyboard/IME, accessibility, actual custom fonts, clean-host installs, power loss/low disk and second-computer private sync are not established by synthetic tests. Never enable a real listener/provider/keyring fixture merely to inflate the test count.

The test-only GTK executable supports NOVEL_RUST_TEST_BINARIES for choosing workers. This override is absent from the production application.


## Notebook tools (0.4.0)

`crates/storage/tests/notebook_tools.rs` verifies exact Unicode, compare-and-swap refusal, revision history/reopen, source ownership and quote offsets, archive/branch remapping and private-sync rollback/conflicts. Bounded diff unit tests distinguish insertions from removals, including large multilingual rewrites.

The opt-in `notebook_navigation_manual_save_and_multi_window` GTK test includes `notebook_tests.rs`: retained selections, history-derived scraps, append/Undo/manual-save behavior, next-session bookmark persistence, reader cancellation and exact reaction marks, invalid heading selections, detached-reader actions, opening marked passages and resolving/adding reminders. It creates fresh temporary stores and never loads personal writing. Run once with debug workers and once with `NOVEL_RUST_TEST_BINARIES=target/rust-package/usr/bin` to exercise the packaged editor/companion. Physical drag/IME and clean-machine installation remain separate acceptance checks.
