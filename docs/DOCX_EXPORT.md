> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Word manuscript export

Open **Project**, choose **Export Word (.docx)**, then select a local destination. The app saves pending work in every open window for the selected project. A failed save, active composition, window transfer, unresolved scene/deletion conflict or conflicting chapter numbering stops the export with an explanation.

The file contains the novel title, chapters and scene titles in manuscript order, followed by every saved paragraph. Chapter numbering respects start-at-0 or start-at-1. Later chapters begin on a new page. Title, Heading 1, Heading 2 and Normal are editable Word styles; existing manuscript headings retain their level.

Text stays in its original Unicode sequence, including Arabic marks, Simplified/Traditional Chinese, emoji, combining characters and private-use characters. Bold, italic, underline, language, paragraph direction, hard breaks and empty paragraphs are retained. Automatic direction uses the first strong Unicode character; explicit direction takes precedence. Paragraphs have zero extra before/after spacing and retain 1, 1.15, 1.5 or 2 line spacing, defaulting to 1.15.

DOCX is a manuscript copy. Planning, revision history, deleted-scene history, dictionaries, alphabet definitions and custom font assets remain in the app and complete project archive. Fonts are referenced, never embedded. Receiving applications need suitable installed fonts; preserved Unicode does not establish glyph coverage. The checked LibreOffice installation renders Arabic and Chinese but lacks glyphs for the fixture's emoji and private-use character.

The export reads a consistent saved database snapshot after save acknowledgements. Further typing belongs to later saves and exports. Cancel leaves the destination untouched. File delivery uses a private temporary file and atomic replacement, with incomplete writes reported. Unsupported metadata, formatting or XML-inexpressible controls are refused rather than removed.

Limits are 64 MiB of serialized scene source/output, 20,000 chapters and scenes each, 100,000 paragraphs, 500,000 source runs and 500,000 literal object characters. Per-scene storage validation also applies. Word import, PDF export, comments and tracked changes remain outside these export slices.

## Saved export styles

On Project, open **Word export styles**. Choose Default layout, Standard manuscript or Compact reading, then edit paper size, equal margins, body spacing and the title/chapter/scene/body fonts, sizes, bold and italic. Default layout retains the original Letter/25.4 mm/Aptos layout and source paragraph spacing. Standard manuscript uses A4/25.4 mm/Times New Roman with double body spacing; Compact reading uses A4/20 mm/Aptos with 1.15 body spacing. Presets are editable starting points, not submission rules.

Styles belong to each novel. **Save Word export styles** saves an immutable project-data revision; exporting or navigating also drains valid pending edits. Fonts are names referenced by the Word copy, without installation or embedding. Font sizes accept 8–72 pt in half-point steps; equal margins accept 10–40 mm. Body spacing overrides affect source paragraphs; source and generated headings retain their paragraph spacing. Word style IDs remain Title, Heading1, Heading2 and Normal.

A stale save keeps the local draft and refuses overwrite. **Reload saved styles** discards pending changes only after confirmation. Competing synced style versions are resolved explicitly in Recovery. Export checks that the selected style revision is still current and reads styles plus manuscript in one transaction. Invalid or corrupt saved styles refuse export; they never silently become defaults. A novel with no style history uses the default without creating a revision. Styles travel with complete project archives and private sync; editing them does not create scene revisions.

## WordCraft integration

The implementation uses [WordCraft](https://github.com/storytold/wordcraft) commit `d7d9702bda1fe626952024dc37fd2624323dad0f`. Only its document model, geometry and DOCX library are vendored. The app retains its Unicode editor with Pango/Cairo. WordCraft's UI, renderer, fonts and dictionaries are not included.

WordCraft is MIT OR Apache-2.0. Licenses, notices, source hashes and a reviewed Arabic/Chinese OOXML patch are recorded in `vendor/wordcraft/SOURCE.json` and packaged notices. The patch emits complex-script bold/italic, east Asian font names and Arabic/Chinese language attributes. Dependencies are pinned in Cargo.lock; the copied writer uses upstream-compatible quick-xml 0.38.4.

## Verification

Rust tests cover exact text/order, Chapter 0, styles, language/direction, spacing, marks, unsupported data, literal U+FFFC, resource limits and a 250,000-word novel. Storage tests cover read-only snapshots and scene/tombstone conflicts. Frontend/native tests cover drains, failed saves, busy/transferring tabs and every expected window's acknowledgement.

`tools/check-docx-export.py` independently checks ZIP/XML against a project bundle. Optional LibreOffice import/export compares exact paragraph text/order and renders a PDF. Desktop probes use synthetic XDG stores and external WebKit instrumentation. The release export probe substitutes only the chooser in a disposable extraction to select its synthetic destination; production contains no release test bypass. Microsoft Word itself and physical keyboard/IME have not been qualified. TEST_RESULTS.md and PACKAGE_VALIDATION.md record observed results and package identity.
