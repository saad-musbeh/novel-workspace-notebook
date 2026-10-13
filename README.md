# Novel Workspace Notebook

A local Linux app for writing novels. White pages, a simple left sidebar, manual saving, and detachable tabs. The interface, Unicode editor, storage worker, and Word exporter are written in Rust.

## Download and start

**[Linux test downloads](https://github.com/saad-musbeh/novel-workspace-notebook/releases)** — version **0.4.0** is a testing prerelease for **Linux x86_64**. There is no Android APK.

1. Download the `.tar.gz` archive from the release, then extract it into a folder.
2. Open that folder and run `./run.sh`.
3. Choose **New novel**, enter a name, and click **Create**. Its first chapter opens ready to write.

The `.rpm` is also available for compatible RPM-based systems. The packages require GTK3, Pango ≥ 1.50, Cairo, Fontconfig, and their normal system dependencies. They were built on Fedora; older distributions and other architectures have not been qualified. Rust, Node, and Python are not needed to run the download.

Use **Save / Ctrl+S** to save. Leaving an edited page asks **Save / Discard / Cancel**. Close an older Novel Workspace instance before opening this version: they share the same local database. Keep a complete project archive before testing with important writing. See [setup and data locations](docs/SETUP.md).

## What you can do

- Write multilingual scenes, including Arabic and mixed-direction text, with formatting, Find/Replace, Undo, and exact Unicode preservation.
- Organize chapters, planning notes, plot cards, character knowledge, and reader reveals.
- Use ordinary clicks to replace the current page, middle clicks for additional windows, and drag a top tab out to detach it.
- Recover earlier saved versions, compare conflicts, use what-if branches, and export Word, HTML, Markdown, text, or complete project archives.

Four new writing aids in 0.4.0:

| Aid | Where to find it | What it does |
|---|---|---|
| Cutting-room floor | Write → Keep & return | Keep selected text as a scrap, recover passages removed between saved versions, and drag or append copies into a scene. |
| Read like a stranger | Review → Review | Read a clean saved copy and mark selected passages **Bored**, **Confused**, or **Curious**. |
| Passages to revisit | Review → Review | Collect unresolved reading reactions and scene notes, alongside counts of saved rewrites to existing text. |
| Return bookmark | Write → Keep & return | Leave a short next-session reminder and return to its captured scene position when the document still matches. |

Writing and saving need no model, network connection, or sync listener. Optional local review is separate. Manuscript saving stays manual; clicking a specific action to keep a scrap, mark a reaction, save a note, or leave a bookmark saves that item immediately.

[The notebook guide](docs/NOTEBOOK_DESIGN.md) explains these workflows and their limits. [The Rust port guide](docs/RUST_PORT.md) lists native coverage and remaining differences from the retained React/Tauri interface.

## Build from source

You need the pinned Rust toolchain, a C compiler/linker, pkg-config, and GTK3/Pango/Cairo/Fontconfig development libraries. See [setup](docs/SETUP.md) for details. With the locked Cargo dependencies already cached:

```sh
tools/run-desktop.sh --build
tools/run-desktop.sh
```

The scripts build offline and install nothing. On a new development machine, prepare the prerequisites and Cargo cache first. To create the RPM and relocatable archive:

```sh
tools/run-desktop.sh --package
```

Outputs go to `target/rust-bundle/`. The retained React/Tauri and Python reference sources are outside the Rust runtime package.

[Tests](docs/TEST_SUITE.md), [observed results](docs/TEST_RESULTS.md), and [package validation](docs/PACKAGE_VALIDATION.md) record the checks performed with synthetic writing. Physical IME, accessibility, clean-host installation, and real second-computer/private-network behavior still need qualification. Report bugs through [GitHub Issues](https://github.com/saad-musbeh/novel-workspace-notebook/issues); use synthetic examples rather than private manuscripts.

## License

The original application is licensed under **[Apache-2.0](LICENSE)**. See [NOTICE](NOTICE) and [third-party notices](docs/third-party/NOTICES.txt) for components that retain their own licenses. No model weights, fonts, dictionaries, or personal writing are included.
