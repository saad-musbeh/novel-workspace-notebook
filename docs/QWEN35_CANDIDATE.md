# Qwen3.5 candidate — metadata only, 3 October 2026

Qwen3 was the first small pinned pilot, not the winner of a comparison. Its measured clean-control failures are retained in [MODEL_EVALUATION.md](MODEL_EVALUATION.md). **Qwen3.5 has not been downloaded, loaded, or evaluated, and no superiority claim is justified.**

The author chose to keep Qwen3 as the experimental test model. This candidate remains prepared but unselected; no Qwen3.5 model or image-projector download is authorized.

The official upstream is [Qwen/Qwen3.5-4B](https://huggingface.co/Qwen/Qwen3.5-4B/tree/851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a), pinned upstream revision `851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a`, under [Apache-2.0](https://huggingface.co/Qwen/Qwen3.5-4B/blob/851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a/LICENSE). The prepared text-only evaluation candidate is its **LM Studio team community GGUF conversion**, not an official Qwen-published GGUF. Its [publisher card](https://huggingface.co/lmstudio-community/Qwen3.5-4B-GGUF/blob/f9f88ac3e234be915e23811a6d28ea287bdb927e/README.md) identifies the upstream and conversion with llama.cpp b8185, and declares Apache-2.0.

| Item | Pinned value |
| --- | --- |
| Quantization repository | `lmstudio-community/Qwen3.5-4B-GGUF` |
| Quantization revision | `f9f88ac3e234be915e23811a6d28ea287bdb927e` |
| File | `Qwen3.5-4B-Q4_K_M.gguf` |
| Model download | **2,707,513,696 bytes** (2.71 GB / 2.52 GiB) |
| SHA-256 | `25082a7dd3776cc3c741c6347d3bd04523f05796607b3fbc32fa3a25dfa1418c` |
| Exact artifact metadata | [Pinned file](https://huggingface.co/lmstudio-community/Qwen3.5-4B-GGUF/blob/f9f88ac3e234be915e23811a6d28ea287bdb927e/Qwen3.5-4B-Q4_K_M.gguf) |
| Download URL | `https://huggingface.co/lmstudio-community/Qwen3.5-4B-GGUF/resolve/f9f88ac3e234be915e23811a6d28ea287bdb927e/Qwen3.5-4B-Q4_K_M.gguf` |

File size and LFS SHA-256 were checked through the publisher's Hugging Face model API with `blobs=true`; only small JSON/model-card metadata was fetched. Hash verification establishes artifact integrity, not output quality. The publisher card identifies the upstream model but does not pin the exact upstream conversion commit; that provenance limit is retained. The [pinned file tree](https://huggingface.co/lmstudio-community/Qwen3.5-4B-GGUF/tree/f9f88ac3e234be915e23811a6d28ea287bdb927e) separates a 675,568,768-byte image projector. The prepared option excludes that projector and other quantizations. No vision projector, GPU runtime, alternate service, or extra package is needed for the proposed text benchmark.

The already verified llama.cpp b11146 contains [Qwen3.5 CPU model loading and inference implementation](https://github.com/ggml-org/llama.cpp/blob/b11146/src/models/qwen35.cpp), including the 4B architecture. **Compatibility is inferred from that source and the older conversion release, not established by a real load test.** The exact artifact, template behavior, memory, latency, and multilingual quality must be tested after separate download authorization. The [official model card](https://huggingface.co/Qwen/Qwen3.5-4B/blob/851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a/README.md) specifies API template settings for disabling thinking; the current adapter already uses `chat_template_kwargs:{enable_thinking:false}`. A 4,096-token context is a deliberately bounded local evaluation setting, not the model's advertised maximum context or a general quality guarantee.

Inspect the prepared option without downloading:

```sh
python3 tools/local-assistance.py --model qwen3.5-4b
```

After explicit authorization, the prepared download option is `--download --model qwen3.5-4b`. It verifies size and SHA-256, reuses the verified 16,998,357-byte runtime archive, and writes a separate `manifest-qwen3.5-4b.json`; the Qwen3 baseline and its manifest remain intact. If nothing is cached, the total pinned download is 2,724,512,053 bytes. Download approval has **not** been granted or exercised for this candidate.

After a separately authorized download, the same isolated synthetic benchmark can compare the candidate using `tools/benchmark-assistance.py --run --modes native-production --model qwen3.5-4b`. The baseline remains the default. The benchmark starts only its own temporary loopback server, uses four CPU threads, one slot, `nice 15`, and a 4,096-token context, then stops its own processes. Candidate selection neither downloads nor enables a startup service by itself.
