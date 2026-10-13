> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Story workflows

Desktop additions approved October 6, 2026. Android remains deferred.

| Tool | Where | Behavior |
|---|---|---|
| What-if branches | Project → What-if branches | Copy a project and its history into a separate branch. Compare exact saved versions, then explicitly bring back one mapped scene or the whole story bible as a new revision. |
| Whole-book writing | Book | Scroll through scenes in manuscript order using the same native Arabic/bidi writer. Each scene saves and undoes independently. |
| Ripple preview | Planning → Story bible / Chronology | Review the authored dependencies affected by a fact or event-time change. Cancel retains the current data. Apply checks the acknowledged revision and commits the reviewed change. |
| Character cut | Book → character selector | Write the canonical scenes linked to a character in manuscript order. This uses explicit scene/event/fact/reveal/knowledge links, not name matching in prose. |
| Paragraph alternatives | Write → Paragraph alternatives | Capture the rich paragraph at the caret, save versions, and swap one into the draft. The original stays immutable and one native Undo restores the previous paragraph. |
| Reader rehearsal | Planning → Reader rehearsal | Move through manuscript progress. Authored future reveals stay hidden; visible clues and setup/payoff notes follow the chosen progress. |

## Preservation and limits

Branches are complete bounded portable project copies, with durable source lineage and scene mapping. They retain history/assets/typed links. Bring-back validates exact source and branch heads, lineage revision, ownership and checksums in one transaction. It does not merge manuscript order, propagate deletions, or apply a selected subset of bible records. A newly imported main with different local IDs needs future explicit reattachment; comparison currently refuses that mismatch. Existing archive limits apply (64 MiB and 20,000 items); branch comparisons show 30 pieces per page with a native maximum of 100.

Whole-book writing loads bodies only for a bounded visible window plus the focused/restore scene. Offscreen editors drain their acknowledged saves before parking. Private transient JSON checkpoints retain shared rich blocks, cursor/selection and the existing bounded Undo/Redo, then release the expensive shaping process. The maximum is sixteen live native helpers across the app. Checkpoints are capped at 128 MiB each, are data rather than executable serialization, stay in the private app cache, and are removed when their owning tab/window session is released. They are session state, not portable backups; acknowledged manuscript revisions remain in SQLite. A failed save, composition or checkpoint leaves the mounted scene available. Hidden-window layout waits have a timer fallback. Tab moves retain the character selection and logical reading passage; restoration waits for an active measured viewport and committed native geometry before consuming its anchor.

A character cut edits the same scene identities as the whole book. It creates no independent draft copies. Changing a cut drains and parks excluded cards before unmounting them. Missing authored links are excluded; no presence or chronology is inferred.

Paragraph alternatives keep exact rich originals and versions under revisioned project data. Swaps validate the saved scene revision and whole-document hash plus block index, so any intervening scene change requires a fresh capture. Duplicate paragraph text is never globally replaced. Draft alternative edits and failed metadata saves block close until acknowledged. New text inherits formatting at the edit; unchanged passages retain formatting. There are at most twenty versions per group and one hundred groups per scene, within the existing 8 MiB project-data limit.

Ripple previews and rehearsal follow explicit authored links. They do not predict prose changes or approve model claims. A stale preview is refused. Deleting a fact or event with retained dependents requires the author to resolve those links first; no silent cascade occurs. Rehearsal does not infer an unrecorded reveal, character presence or missing payoff.

## Verification

Source and packaged evidence is recorded in TEST_RESULTS.md and PACKAGE_VALIDATION.md. Synthetic tests exercise exact Unicode, CAS/conflicts, failed barriers, hidden windows, bounded loading, native raster geometry and session ownership. They do not establish physical Arabic/IME or clean-host qualification.
