> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Desktop scale measurements

## Incremental layout investigation — October 5

The frozen replacement remains the qualified implementation. A scratch 54,000-character mixed-script experiment measured full Pango itemization at 16.839 ms, full FriBidi type/bracket/embedding analysis including UTF-32 conversion at 1.923 ms, and a 512-character tail layout at 0.865 ms (12-sample medians). These are separate components, not an end-to-end result. An earlier RLO caused tail-only itemization to return a different final embedding level from full-paragraph analysis, so naive tail caching is unsafe. A future shared glyph/line cache must retain full-paragraph bidi analysis and invalidate changed levels/clusters; global direction or width edits need a full fallback. No 3 ms claim follows from these experiments.

## Replacement writer — October 5, 2026

The application-owned model now uses Pango/Cairo for shaping and viewport painting. Processing timing includes edit application and changed-paragraph shaping/reflow; output timing covers bitmap generation/document snapshots, with IPC outside both. Font fallback is delegated to Pango per formatted run, rather than allocating a font attribute for each script transition. Unchanged paragraph layouts are retained and old text/grapheme caches are pruned. Default rendering spacing is 1.15; explicit saved spacing remains unchanged.

Eight warm helper input samples at 850×680 pixels measured short text at median **0.176 ms processing / 9.052 ms output**, a single **54,000-character mixed-script paragraph at 70.194 / 12.185 ms**, and a single **56,250-character Latin paragraph at 26.882 / 11.411 ms**. These source-helper observations exclude the browser, IPC, queue and screen. They do not guarantee latency. The general **3 ms processing target remains unmet for giant paragraphs**; further improvement needs incremental layout with tested bidi context, rather than dropping shaping from the timing. Exact packaged measurements and fresh physical input remain separate.

The following October 3–4 writer observations describe the retired LibreOffice backend. They are retained as historical evidence and do not qualify the replacement.

## Long-paragraph input correction — October 3, 2026

An isolated native debug app used one paragraph with 54,000 UTF-16 code units of synthetic English, Arabic/kasra and Chinese text. Before the correction, the caret could move far below the 680-pixel bitmap being painted. The native helper now paints around the caret using the actual scaled viewport height, repaints when layout changes its origin or top, and returns matching viewport coordinates. React scrolls after committing that frame and rejects older responses following manual scrolling. Printed-page shadows are disabled; this is still Writer page layout, not a claim of continuous layout. Whole-scene grapheme counting runs in a worker and preserves its existing Unicode semantics.

The native regression passed exact input persistence, all caret/visible-viewport assertions, correct final grapheme count and manual-scroll preservation during an in-flight response. One retained timing run observed typical edit application of 36–38 ms, preview output of 14–19 ms and native request/response of 56–59 ms, with slower samples up to 95 ms. A burst of 56 code points merged into six native requests and drained in 811 ms with exact saved text. The main-thread 10 ms heartbeat recorded a maximum extra delay of 14 ms. These are synthetic debug measurements on this host, not physical key-to-screen times or isolated hardware benchmarks.

This fixes painting/scrolling failures and removes character segmentation from the typing thread. LibreOffice still reflows very long paragraphs, so the requested 3 ms processing budget is not met for this fixture. A direct native key-event candidate crashed during installed LibreOffice autocorrect teardown; no completed fidelity/speed comparison was retained and that path was not adopted. The validated InsertText path remains in use. Physical input, IME and release resource measurements remain separate checks.

The synthetic desktop fixture passed save, search, archive round trip, and reopen checks on October 3, 2026. This measures the local companion and a server-rendered board. It does not establish interactive typing latency or a release performance budget.

## Resize and full-screen observations — October 3, 2026

The original resize probe reproduced incomplete bitmap coverage when shrinking the window. Window width/viewport height now invalidate older responses, preserve the native document anchor, and repaint after 100 ms of settled geometry. Programmatic scroll suppression records the browser's actual clamped position. If manual scrolling reaches the DOM before its event, an in-flight frame is rejected and refreshed at the new position. The draft's scroll spacer has no painted background; its bitmap, cursor, input node and clipped selection overlays share a bounded viewport. The input node remains mounted across size changes.

The final 54,000-code-unit native fixture passed eight size/maximize/full-screen changes and exact Arabic/kasra/Chinese saving. There were one to three requests per settled change and zero idle requests; eighteen rapid size changes produced three requests. All viewport/anchor/control/caret assertions passed. Its five-character insertion samples took 75–122 ms in native application and 104–172 ms including response and a frame wait. The main-thread 10 ms heartbeat recorded at most 26 ms extra delay. A release build was active concurrently, and insertion sizes differ from the earlier per-code-point probe, so this is not a controlled latency comparison.

A separate run after the build, using the existing 20-paragraph Arabic fixture, passed the same checks and exact saving. Its native application samples were 0.75–4.49 ms; response-plus-frame samples were 19–40 ms and heartbeat extra delay at most 7 ms. The resizing correction establishes bounded rendering, stable positions and no idle request loop in these fixtures; it does not remove the installed engine's long-paragraph reflow cost or establish physical keyboard/IME latency.

## Reproduce

Use the already installed Rust and JavaScript dependencies. No download or service installation is needed.

```sh
cargo build --offline -j 2 -p novel-companion
python3 tests/desktop_scale_test.py --binary target/debug/novel-companion --report work/desktop-scale-results.json
```

The script creates a temporary database under `work/`, starts its own stdio companion, and removes that database and frontend bundles afterwards. It never opens the normal application database, starts a model, or opens a network listener. The report contains synthetic metrics only. Each RPC timing includes request serialization/transfer, native processing, response transfer and parsing. Reopen uses a warm operating-system cache. No forced cache clearing was performed.

The fixture has 1,000 scenes with 750 Chinese characters each (750,000 total), 500 character records, 2,000 fixed-day events and a 1,000-card board. Ten edits create normal scene revisions. Archive import creates a separate project. Counts, exact rich manuscript documents, exact story records, search hits and the last acknowledged revision after process restart are asserted. The Chinese fixture is not treated as linguistically equivalent to 250,000 English words.

## Observed results

Measured at `2026-10-03T11:05:05Z` on Linux `7.2.7-200.fc44.x86_64`, x86-64, 16 logical CPUs. Debug companion SHA-256: `83a7d0e9feed42c415a6622905bb9350c17f31ea38b5f263f08600260c52fb7d`. Development builds and other work were active on the host; these are observations, not isolated hardware comparisons.

| Operation | Samples | Observed time |
| --- | ---: | ---: |
| Start companion and create project | 1 | 41.76 ms |
| Create scene | 1,000 | median 4.00 ms; p95 5.62 ms; max 20.56 ms |
| Save scene revision | 10 | median 4.08 ms; p95/max 6.38 ms |
| Save planning document | 1 | 245.79 ms |
| Open all 1,000 scenes | 1 | 232.00 ms |
| Exact search, 1,000 hits | 1 | 154.53 ms |
| Count manuscript statistics | 1 | 297.03 ms |
| Export complete archive | 1 | 1,143.33 ms |
| Import complete archive | 1 | 1,254.55 ms |
| Load imported planning | 1 | 125.96 ms |
| Restart process and reopen project | 1 | 245.94 ms |
| Render board with React SSR | 10 | median 24.52 ms; p95/max 61.51 ms |
| Hash all scene sources with Node WebCrypto | 1 | 17.83 ms |

The archive was 6,736,966 bytes. After exporting and importing, companion resident memory was 94,080 KiB and its high-water mark was 113,256 KiB. The reopened process used 18,272 KiB, with a 31,652 KiB high-water mark. Node's peak resident memory for board rendering and hashing was 126,528 KiB; this is a separate process and is not the desktop app's total memory.

The database contained both original and imported projects and retained revision history. Before shutdown it occupied 11,083,776 bytes plus an 8,561,392-byte WAL and 32,768-byte shared-memory file. After clean shutdown the database remained 11,083,776 bytes and the WAL was checkpointed. A 98-byte synthetic authentication metadata file was retained; its contents are never printed in the report.

That earlier default board viewport rendered 25 of 1,000 card bodies. Its HTML was 173,870 bytes, including the complete card-selection controls. This verifies viewport culling of card bodies; SSR does not measure pointer movement, scrolling, hit testing, accessibility, layout or paint. The October 4 changes further bound lists, connectors and connection forms and reuse unchanged review hashes; the application still opens complete project scene snapshots.

An earlier debug snapshot on the same host measured shorter open/search/archive times. Its binary hash differed and concurrent workload was uncontrolled. The script records the exact binary hash on every run; these two observations must not be presented as a controlled regression comparison.

## Remaining desktop release evidence

- Measure editor input, navigation, board interaction, project switching and native app memory with the full fixture in the actual release build. SSR and storage timing do not substitute for desktop frame or keystroke measurements.
- Measure cold launch, slow-disk behavior, low disk space, recovery after process termination and interrupted export/import. This run verifies clean shutdown and acknowledged revision persistence.
- Complete physical English/Arabic/Chinese keyboard and IME composition, mixed direction, selection, clipboard and accessibility checks. Custom-font rendering needs an author-supplied permitted font.
- Measure real private sync transport bytes and latency separately. Archive bytes are not a network sync measurement.
- Evaluate the installed local model and LanguageTool language variants separately for evidence validity, false positives, memory, latency and cancellation. Grammar enforcement and mock responses establish structure and safety, not language quality.
- Retain the native blocked-provider responsiveness test: writing/storage must acknowledge while inference is pending. It is an independent companion integration test, not a result inferred from this scale script.

The independent `native_sqlite_acknowledgement_arrives_while_provider_http_is_blocked` integration test passed on October 3. The 24 provider tests also passed, including cancellation tombstones, attempt isolation, timeouts, exact unique quotation resolution, rejected invented/ambiguous/partial-grapheme evidence and selected-passage restrictions. These checks establish evidence handling and storage independence; they do not establish model semantic accuracy.

These are desktop validation gaps. Android is outside this measurement scope.

## Final desktop release observations — October 4, 2026

Both release companion fixtures passed: 1,000 scenes, 500 story records, 2,000 events and 1,000 cards, with either 250,000 English whitespace words or 750,000 Chinese characters. Exact rich documents/records, counts, 1,000 search hits, separate archive import and the last acknowledged revision after restart were checked. Companion SHA-256: `ab26df33f85c5348a89d715f4ea241442ae1d85d9bd7c6b7ff122481870001dc`. These are warm-cache synthetic stdio measurements, not engine input, physical IME, cold disk or frame latency. Other desktop work was active on the host.

| Operation | English | Chinese |
| --- | ---: | ---: |
| Save revision median (10 samples) | 3.166 ms | 3.214 ms |
| Open project | 34.608 ms | 37.362 ms |
| Exact search | 25.340 ms | 18.021 ms |
| Statistics | 71.800 ms | 28.154 ms |
| Archive export | 103.332 ms | 103.003 ms |
| Archive import | 229.580 ms | 196.941 ms |
| Process restart/reopen | 53.565 ms | 59.725 ms |

Archives were 7,213,336/6,736,966 bytes; companion high-water memory after export/import was 114,412/111,016 KiB. This is companion memory, not total desktop/LibreOffice memory. The current default SSR viewport renders 25/1,000 cards with 14,848 markup characters; SSR does not measure paint. All-source Node hashing in this benchmark is a baseline, not the app's incremental cache timing. Seven cache regressions verify one new digest when one of 1,000 saved scenes changes.

The exact independently extracted final RPM passed native keyboard reveal/movement, retained focus during pan, bounded SVG/cards, 50-card list pages and current-page arrow navigation with 1,000 cards/999 edges. Save/remount/archive checks preserve the full exact planning document and create no revision for mere selection. External WebKit instrumentation changes process timing; no native frame-rate claim follows. The app still loads whole project snapshots. A manual sync pass explicitly pauses input for up to two minutes; future background sync needs staged receive/apply, not removal of the save barrier.

## October 5 tabs and background work

Writing tabs no longer instantiate hidden review controllers or compute hashes for every scene. Review work begins in a Review tab, which remains mounted when another tab is active. The tab coordinator keeps editor sessions mounted when switching tabs and transfers the same native session between windows. Layout callbacks skip zero-width hidden surfaces; drain checkpoints keep finite native coordinates and the last scroll anchor. Unchanged alphabet loads do not reopen the editor.

The two-window external fixture records aggregate owned-process peak RSS and workflow time, not a typing budget. Summed RSS includes shared pages and is not proportional/private memory. Physical IME, real compositor resize, three-window input latency and a strict 3 ms long-paragraph target remain unqualified.

The final release companion also passed 1,000-scene scale fixtures on October 5: 250,000 English whitespace words and 750,000 Simplified Chinese characters, each with 500 records, 2,000 events and 1,000 cards. Median acknowledged saves were 3.305 ms and 3.563 ms; open-project was 30.586 ms and 36.462 ms; archive export/import was 115.104/192.700 ms and 131.711/224.203 ms. Reopening retained the last acknowledged revisions and exact rich text/planning. These are stdio/storage observations using a warm filesystem cache, separate from native editor processing and physical typing.

The exact final RPM’s two-window transfer/return workflow took 8.250 s with aggregate peak owned RSS 1,342,712 KiB; original-window-close took 6.438 s with aggregate peak RSS 1,120,892 KiB. The former sampled two native editors and one companion; the latter sampled one editor and retained one companion after original destruction. Both passed functional state/save/undo checks. RSS sums shared pages across processes; these instrumented whole-workflow times include waits and are not per-key processing or private-memory measurements.


## Typing priority — October 6, 2026

Ordinary input batches retain original input boundaries and separate Undo steps, with limits of 128 inputs/64 KiB per batch and 256 queued entries/about 4 MiB. Saturation preserves pending input for recovery and blocks a misleading successful drain. Composition, paste, commands, snapshots and viewport generations remain barriers. If a further ordinary text request is already waiting, an intermediate apply-only acknowledgement can defer layout/painting; final input and drain still prepare the full paragraph. No timing claim treats deferred shaping as completed work.

Three samples of 260 inputs in a 54,000-character mixed paragraph at 850×680 measured median **309.8 ms** with three complete renders versus **136.7 ms** with two apply-only acknowledgements and one final render. Both retained 260 Undo steps and identical text, final bitmap, caret and height. The earlier merged baseline retained only three Undo steps and measured 251.5 ms; it is not a semantically equivalent comparison. Intermediate processing was about 25.7 ms per 128-input request; final four-input/full-shaping processing about 72 ms. The strict 3 ms general target remains unmet.

Readonly workspace/history reads and incremental review-source hashing yield while native input/composition is pending, then resume after a quiet interval. Saves and close barriers never wait behind that background scheduling. Reference projections cache acknowledged metadata and bound visible rows to 30. Tests cover cancelled work, exact Unicode, transaction rollback, final-render failure recovery, composition barriers and separate Undo semantics. Physical key-to-screen and compositor changes remain unqualified while the author is remote.


## October 6, 2026 — giant-paragraph performance accepted

After the benchmark was explained as one 54,000-character mixed-script paragraph with approximately 72 ms processing (roughly 8,000–10,000 English words without paragraph breaks), the author replied “I am OK with this.” That extreme-case performance is accepted and no longer blocks desktop completion. Further incremental bidi/layout optimization is optional. Measurements are unchanged; no general 3 ms guarantee or physical-input qualification is inferred. Ordinary typing responsiveness remains a design goal. Android stays deferred.
