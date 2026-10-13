# Local model evaluation — 3 October 2026

The local runtime and exact-evidence pipeline work. **Qwen3-4B Q4_K_M is not qualified for reliable continuity checking:** it detected four seeded contradictions but also raised false-positive questions on all four clean controls. Its output remains an optional, experimental proposal for author review. Deterministic continuity checks remain the dependable path for explicit facts and constraints.

## Reproducible configuration

- Runtime: llama.cpp **b11146**, version `0.5.0-dev`, commit `7fe450e19`; verified Linux CPU release, GNU 11.4.0 build. The bundled runtime license is MIT.
- Model: official **Qwen3-4B Q4_K_M**, pinned repository revision `bc640142c66e1fdd12af0bd68f40445458f3869b`; 2,497,280,256 bytes. Model SHA-256: `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5`.
- Runtime archive: 16,998,357 bytes; SHA-256 `c150306eb16b5ab696f76a8bdf810c35fd98a24e82158742e6fa28f420ff8410`.
- Hardware: Intel Core i7-11800H, approximately 15.3 GiB system RAM. CPU only, four inference/prompt threads, one parallel slot, 4,096-token context, process priority `nice 15`, loopback `127.0.0.1:8080`.
- Model weights and binaries remain in ignored `runtime/local-assistance/`. They are excluded from Git and project archives. No OS packages, fonts, services, firewall settings, or global model caches were changed.

The [pinned official model card](https://huggingface.co/Qwen/Qwen3-4B-GGUF/blob/bc640142c66e1fdd12af0bd68f40445458f3869b/README.md) identifies the model license as [Apache-2.0](https://huggingface.co/Qwen/Qwen3-4B-GGUF/blob/bc640142c66e1fdd12af0bd68f40445458f3869b/LICENSE). The [pinned server documentation](https://github.com/ggml-org/llama.cpp/blob/b11146/tools/server/README.md) describes the local API and schema/template options. This evaluation covers that particular downloaded model and runtime, not other quantizations or models.

## Actual production evaluation

`tools/benchmark-assistance.py --run --modes native-production` creates isolated synthetic projects, commits each manuscript passage and approved story-fact snapshot, and invokes the real storage companion's `provider_run`. No personal database or author writing is read. Each language has one explicit green-versus-brown eye-color contradiction and one matching green-eye control.

The final protocol requests `{suggestions:[{title,explanation,evidence:[{sourceId,quote}],action}]}`. The native adapter requires exact, unique quotations from the frozen source; it calculates trusted revisions, hashes, UTF-16 offsets and grapheme boundaries. Consistency findings can only ask questions. The benchmark then runs the application's actual suggestion validator and verifies that the stored document and revision are unchanged.

| Input | Seed detected | Clean false positive | Exact evidence valid | Seed / clean latency |
| --- | --- | --- | --- | --- |
| English | Yes | Yes | 2 / 2 | 27.3 / 14.2 s |
| Arabic | Yes | Yes | 2 / 2 | 25.1 / 22.2 s |
| Simplified Chinese | Yes | Yes | 2 / 2 | 15.2 / 19.6 s |
| Traditional Chinese | Yes | Yes | 2 / 2 | 14.1 / 19.1 s |

All eight normalized results passed exact-evidence validation; all eight manuscript revisions and documents remained unchanged. Mean request latency was **19.6 s**, model load **3.0 s**, generation **10.9–12.1 tokens/s**. Peak server process RSS was **5,492,700 KiB (5.24 GiB)**; this includes mapped model pages and runtime allocations, not total system memory.

The clean false positives were unsupported questions about timing despite matching eye color. The Arabic clean explanation acknowledged no contradiction before raising a timing doubt. The Traditional Chinese clean explanation used simplified Chinese and incorrectly claimed the passage omitted noon. Chinese output-variant fidelity and semantic accuracy therefore remain unqualified.

## Earlier protocol and schema probe

The earlier full-suggestion protocol asked the model to generate source hashes and numeric evidence offsets. All eight multilingual seed/control results failed exact-evidence validation because the offsets did not match their quotations; no result was applied. It also proposed redundant facts on the four clean controls. Mean latency was 50.6 s. The compact production protocol fixes evidence integrity and reduces latency; it does not fix semantic false positives.

A deliberate schema probe requested a different object but enforced the constant `{"schemaVerified":true}`. In this pinned runtime, `response_format:{type:'json_object',schema:...}` enforced the schema; `type:'json_schema'` with a top-level `schema` did not. With `chat_template_kwargs:{enable_thinking:false}`, the probe returned only that object, no `reasoning_content`, and ten completion tokens. That verifies observed API behavior; hidden internal processing is not independently observable.

Raw synthetic records and server timing logs are outside the repository in the workspace's `work/` folder:

- `model-evaluation-20261003T110202Z.json` / `.log`: eight earlier-protocol failures, schema probe, memory measurements.
- `model-evaluation-20261003T111108Z.json` / `.log`: eight final production results, validated evidence, unchanged-revision checks, latency and memory.

Both owned temporary servers were stopped. No startup service was enabled. To reproduce, explicitly download and verify with `tools/local-assistance.py --download`, build the companion, then explicitly run the benchmark. Its default mode uses the production adapter and stops only its own server and temporary companion.

This is a small synthetic diagnostic, with one run per fixture. It does not establish grammar, style, large-context, invented-alphabet, or general literary quality. The model must not be presented as a source of canon or as a proven contradiction detector.
