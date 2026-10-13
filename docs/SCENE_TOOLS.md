> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Scene tools

Updated October 6, 2026. Desktop only.

## References while writing

Select **Reference** above a draft. The pane follows its scene and shows explicitly linked people/places, facts, occurrence events, reader reveals and character knowledge. **Pin scene** keeps that reference scene when you open another draft; **Follow current scene** resumes following. The scene selector can pin any scene. Reference browsing does not alter the draft or reset its Undo history. Each split draft has its own pane.

Add links in Planning first. Unlinked facts are not guessed from manuscript text. Missing scene links remain visible. Facts awaiting approval, character beliefs and unresolved timing retain their labels.

## Two timeline orders

In **Planning → Chronology**, **Story time** shows fictional occurrence dates. **Reader learns** shows only reveals you author, ordered by manuscript scene and explicit position within that scene. A flashback can appear early for the reader while occurring later or earlier in story time. Moving a manuscript scene changes reader order without changing its fictional date.

Use **Add reader reveal** to select an event or fact, its scene and its position. Unknown dates and contradictory constraints stay unresolved.

## Character knowledge

In **Planning → Story bible**, select a character and use **Knowledge by scene → Add knowledge**. Record what the character knows, believes or misunderstands, its source, and either explicit scenes or a story-event range. “Knows” requires an approved fact; a belief or misunderstanding also needs the character’s interpretation.

Each entry retains the exact referenced fact at authoring time. If canon changes later, the old claim stays and is marked **Referenced fact changed**. Knowledge does not automatically approve facts. Event ranges cannot establish internal order when a scene spans both boundaries; the pane reports uncertainty rather than assuming.

## Project time machine

Open **Project → Project time machine**. Choose a recorded point to preview a scene and the story bible together. Scene order and placement come from recorded hierarchy revisions. Older projects lacking those records show **Historical placement unavailable**; current order is not substituted.

Recover a selected scene or explicitly recover the whole story bible. Recovery copies exact saved content into a new revision and retains newer versions. It first drains open drafts in this window, then atomically checks the target’s current heads. If another window or sync advances the head, reload history and review before retrying. Current conflicts must be resolved first. The preview’s database epoch, sequence and revision identity prevent an old cutoff being reused after database replacement.

Points describe this database’s recorded order. Imported timestamps do not establish a synchronized global timeline. This is selective content recovery, not a whole-database replacement or disk-loss backup.

## Typing priority

Background reads and review hashing yield while input or composition is pending. Queued ordinary typing can share a bounded native request while keeping each input’s Undo step. Intermediate acknowledgements skip redundant painting only when another text request is already waiting; the final edit, command and save barrier prepare the complete paragraph. Pasting, composition, commands and manual viewport changes remain barriers.

A synthetic 260-input burst into a 54,000-character mixed paragraph improved from about 310 ms to 137 ms in the helper comparison. Final full-paragraph shaping still took about 72 ms. The author accepts this giant-paragraph limit, so it no longer blocks the desktop plan. A general 3 ms guarantee is not claimed; these are helper measurements, not physical keyboard latency.
