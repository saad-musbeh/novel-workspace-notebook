# Dependencies and notices

Updated October 10, 2026 for the native Rust desktop. Cargo.lock and rust-toolchain.toml are the reproducible baseline. No dependencies were installed or fetched for this port; all builds used the existing offline cache.

## Active application graph

| Dependency | Locked/direct version | Purpose |
|---|---|---|
| Rust | 1.99.0 | Compiler/toolchain |
| gtk | 0.18.2 | Native GTK3 widgets, windows, clipboard, IME and chooser |
| glib | 0.18.5 | GTK main context and asynchronous worker replies |
| pango | 0.18.3, v1_50 feature | Whole-paragraph shaping, caret and selection geometry |
| cairo-rs | 0.18.5, png feature | Viewport rendering and PNG frames |
| unicode-segmentation | 1.13.3 | Extended grapheme/word boundaries |
| regex | 1.13.1 | Unicode word/token behavior |
| sha2 | 0.10.9 | Exact document/font/text checksums |
| base64 | 0.22.1 | Bounded viewport frame encoding |
| sqlx | 0.8.6 | Existing SQLite persistence |
| tokio | 1.53.1 | Existing companion asynchronous work |
| reqwest | 0.13.5 | Optional local providers/private authenticated peer client |
| serde / serde_json | 1.0.229 / 1.0.151 | Preserved document/RPC data |
| uuid | 1.27.0 | Stable local identities |
| WordCraft | vendored 0.3.0 | Existing DOCX document/geometry/export modules |

GTK3, Pango >= 1.50, PangoCairo, Cairo and Fontconfig are external system libraries. The native app calls a small checked PangoCairo/Fontconfig FFI surface where bindings do not provide the required operation. Complete bidirectional selection ranges copy both endpoints of every returned range.

The Rust distribution contains no npm runtime packages, Python helpers, WebKit, fonts, dictionaries or model weights. Legacy React/Tauri dependencies remain in package.json/package-lock.json for reference development; they are outside the Rust package graph.

## Collected notices

`tools/dependency-notices.py` collects notices for the locked non-dev runtime/build graph rooted at novel-native-writer, novel-editor and novel-companion, including reachable vendored WordCraft crates. The observed inventory is **227 packages, zero missing notice files**. This is conservative dependency metadata, not an exact linked-binary map, legal approval or vulnerability audit.

Generated files are docs/third-party/NOTICES.txt and inventory.json. Upstream files and checksum-pinned supplemental notices retain their existing attribution. inventory.json records Cargo.lock and retained npm-lock hashes; its package list is Rust-only. Packaging refuses a stale Cargo-lock notice hash. After intentionally changing dependencies:

```sh
python3 tools/dependency-notices.py
tools/run-desktop.sh --package
```

Vendored WordCraft is pinned to `d7d9702bda1fe626952024dc37fd2624323dad0f`, MIT OR Apache-2.0. vendor/wordcraft/SOURCE.json records hashes/manifests and the prior Arabic/Chinese OOXML compatibility patch. Its fonts/UI/renderer/dictionaries are not distributed. Existing zip 3.0.0 and quick-xml 0.38.4 pins retain their notices.

## Optional external material

Project fonts and dictionaries require author-supplied permission/license metadata and checksum/format validation. Engine font registration is process-local; no system font installation is performed. The GTK interface does not yet provide the full font/language-pack import workflow.

The previously authorized llama.cpp/Qwen3 and LanguageTool evaluations remain separate ignored runtime material. Neither is included or automatically started by the Rust app. Model/grammar quality remains subject to the limits in the evaluation documents. LibreOffice is only an optional DOCX interoperability test tool. Tailscale and libsecret are optional external services/tools; no configuration changes are implied.

The personal RPM currently records the application's license as Unspecified, matching the absence of an application license grant. Collected third-party notices do not grant a license to the application itself. Earlier React/Tauri package inventories/counts belong to those historical releases.
