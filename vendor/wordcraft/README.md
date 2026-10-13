# WordCraft DOCX libraries (vendored)

This directory contains production Rust source from WordCraft commit `d7d9702bda1fe626952024dc37fd2624323dad0f`, limited to `crates/geom`, `crates/doc`, and `crates/docx`. The copied source hashes in `SOURCE.json` preserve the upstream SHA-256 values. Crate manifests are adapted for this minimal nested workspace. Its `quick-xml` pin is `0.38.4`, matching the upstream lockfile; the writer source is incompatible with `0.42.0`, which may remain in the main application dependency graph.

One reviewed local compatibility patch is applied in `crates/docx/src/write/props.rs`: it writes complex-script bold and italic properties, east Asian font selection, and language attributes for Arabic and Simplified/Traditional Chinese. `SOURCE.json` records the original and current file hashes and describes this patch. All other copied Rust source remains byte-for-byte identical to the recorded upstream commit.

The docx crate provides DOCX package reading and writing. Geometry and document-model crates are its required foundations. Integration-test folders and fixtures are omitted; unit-test source files inside copied `src/` trees are retained. No fonts, UI, rendering, dictionaries, or brand assets are present.

See `LICENSE-MIT`, `LICENSE-APACHE`, and `NOTICE`.
