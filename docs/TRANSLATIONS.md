> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Optional scene translations

Write each scene in one language. In **Write**, choose **Translate** to open its optional translations. Language tabs appear only after a draft is added; there is no requirement to fill every language.

Choose the original language and target: English, Arabic, Simplified Chinese or Traditional Chinese. **Translate** requests a draft from the local model configured in Review. **Add translation myself** opens an empty draft without a model. Both paths leave the manuscript intact.

Edit any wording in the translation and choose **Save translation**. Closing the panel, switching scenes/tabs or closing the app also drains pending translation edits. Composition and failed saves block leaving; **Retry translation save** retains the same pending draft. Competing project-data revisions use the existing explicit conflict handling.

**Show original beside translation** displays the exact saved source used for that draft. If current original text differs, the panel shows **Translation may need updating**. It never replaces the author's wording automatically. Translate again or add a new manual draft against the changed original; use **Saved draft** to revisit older drafts. Each save also retains project revision history.

Translations are stored as local `sceneTranslations` project data and included in complete project archives, branches and sync. Their scene/revision references are remapped on archive and branch copying. Source ownership and exact original text are validated by storage. Ordinary manuscript/Word exports continue to export the original manuscript; translation text can be copied from its editor.

Automatic translation accepts a complete saved scene up to 12,000 UTF-16 units, with bounded output and a two-minute timeout. Manual source/draft text supports 500,000 UTF-16 units within the 8 MB project-data limit. Requests use the configured literal-loopback local model, with no automatic service start, model download or cloud fallback. Closing/cancelling discards late inference output. Translation quality depends on the supplied local model and is always reviewable by the author.
