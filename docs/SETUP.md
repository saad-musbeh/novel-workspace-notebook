# Rust desktop setup

Updated October 10, 2026. The default application is the native Rust GTK desktop. Android remains outside the current scope.

## Requirements

Use an existing Rust 1.99.0 toolchain, C compiler/linker, pkg-config and development metadata for GTK3, Pango >= 1.50, PangoCairo, Cairo and Fontconfig. Cargo.lock pins the resolved Rust graph. The current host already has compatible native libraries and cached crates; no packages were installed for this port.

For the packaged app, Rust/Cargo is not required. The Linux runtime needs GTK3/Pango/Cairo/Fontconfig and their normal dependencies. The RPM was built on the current Fedora host; compatibility with older distributions or other architectures is not established. Optional peer credentials use the existing OS keyring/secret-tool from libsecret.

Node, npm, WebKitGTK, Python GI, Pycairo and python-regex are not requirements of the Rust app. They remain useful only for retained React/Tauri development and comparison tests. Python also runs the development notice collector; it is not bundled.

Inspect the host without installing anything:

```sh
tools/check_environment.sh
```

Missing prerequisites or cache entries require a separately authorized installation. The build scripts deliberately fail instead of downloading them.

## Build, run and package

From the repository:

```sh
tools/run-desktop.sh --build
tools/run-desktop.sh
```

The default executable is `target/debug/novel-workspace-rust`; its workers are siblings. If Cargo is absent from PATH, the script uses the existing per-user Cargo installation.

```sh
tools/run-desktop.sh --package
tools/run-desktop.sh --release
```

Release files:

- `target/rust-bundle/novel-workspace-rust-0.4.0-1.x86_64.rpm`
- `target/rust-bundle/novel-workspace-rust-0.4.0-linux-x86_64.tar.gz`

The archive contains `run.sh` and its `usr/` payload. Extract into a chosen folder and run `./run.sh` there. The RPM has a distinct “Novel Workspace (Rust)” launcher and binary names. Neither packaging nor this port installs the RPM, changes the existing shortcuts or enables a service. `--no-focus` launches without taking keyboard focus for isolated desktop checks; normal launching omits it.

The existing npm desktop scripts still forward to the new native build/run scripts, but npm is unnecessary for native work. `npm run dev` and `npm run build` refer to the retained React implementation.

## Writing data and recovery

Data remains in `$XDG_DATA_HOME/local.novel.workspace.spike/` or `~/.local/share/local.novel.workspace.spike/`. The SQLite file is `editor-spike.sqlite3`. Cache uses `$XDG_CACHE_HOME/local.novel.workspace.spike/` or `~/.cache/local.novel.workspace.spike/`.

The same Rust companion exclusively owns the database and preserves schema-3 history/conflicts. Close the older app before using this store in Rust. No migration is added by the interface port. Keep a complete archive before deliberately trying a new interface on important writing. Do not copy or synchronize live WAL database files.

Save / Ctrl+S saves the current editable page. Leaving an edited page, switching tabs or closing asks Save / Discard / Cancel. A save refusal retains the draft and keeps its page open. Middle click Review → Saved versions → Scene conflicts to inspect a retained stale draft in another window and preserve it as an explicitly chosen new revision. See NOTEBOOK_DESIGN.md for the full manual-save workflow.

Complete project archives preserve the project's revisioned data and assets. Import creates a separate project. Backups/history are not automatically pruned and do not protect against disk loss; copy a chosen archive to a separate destination. Word/HTML/Markdown/text export uses original scenes, not optional translation drafts. Markdown/plain text omit paragraph metadata and rich marks.

## Optional local services

Writing and saving start no provider or listener. Review uses explicitly configured literal loopback HTTP endpoints only. The app does not download or start LanguageTool or a model and provides no cloud fallback. Existing local evaluation records remain in MODEL_EVALUATION.md and PROOFREADING_EVALUATION.md; they are not a quality guarantee.

Private sync is disabled at startup. The native sync page can inspect status/devices/peers and explicitly start or stop a session listener. Full pairing/revocation/manual-sync controls are still available only in the retained interface/backend; see RUST_PORT.md. Backend policy still refuses public/LAN/wildcard binding and unapproved hosts, preserves application authentication and uses OS-keyring credentials. Real private-network behavior is unverified. No firewall, SELinux, Tailscale or system service change was made.

## Development verification

Follow TEST_SUITE.md for the native workspace, GTK workflow and optional reference comparison. Use synthetic temporary stores only. Prior React/Tauri probes and physical acceptance gates are recorded separately; see RUST_PORT.md and PACKAGE_VALIDATION.md.
