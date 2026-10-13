# Local proofreading pilot — 3 October 2026

LanguageTool 6.6 was downloaded with explicit authorization and tested through the actual native companion with synthetic text. No personal writing was read or changed. Its temporary server stopped after the test; no startup service or global installation was created.

The official versioned archive was 251,998,221 bytes. Local SHA-256 is `53600506b399bb5ffe1e4c8dec794fd378212f14aaf38ccef9b6f89314d11631`. This is a recorded integrity hash, not an upstream authenticated checksum. The package and all component notices remain in ignored `runtime/local-proofreading/`; they are excluded from the installer and source archive. Source: <https://languagetool.org/download/LanguageTool-6.6.zip>.

The installed package README requires Java 17 or later. Its core COPYING.txt specifies LGPL 2.1 or later; third-party components have their own notices in `third-party-licenses/`. The Arabic Hunspell dictionary's COPYING allows GPL 2.0/LGPL 2.1/MPL 1.1 alternatives. The original complete package is retained locally with these notices. Imported project dictionaries have a separate author-declared license and checksum policy.

## Observed results

| Fixture | Result | Wall time |
| --- | --- | --- |
| English `She go to school every day.` | One pending suggestion, valid exact evidence | 1.893 s |
| English corrected control | No suggestions | 0.148 s |
| Arabic `ذهبت الفتاة إلى المدرسه.` | One pending suggestion, valid exact evidence | 0.794 s |
| Arabic corrected control | No suggestions | 0.240 s |
| Simplified Chinese clean control | No suggestions | 0.270 s |
| Traditional Chinese clean control | Explicit unsupported-variant error; no fallback | 0.003 s |

The service advertised app-supported inputs `en`, `ar`, and `zh-Hans`; it did not advertise an explicit Traditional Chinese variant. These labels indicate installed capability, not equal grammar quality. Generic English uses LanguageTool's general `en` rules; its package documentation says regional variants are needed for English spell checking. The small Chinese clean control establishes only a successful request with no warning, not useful Chinese error detection.

All five successful responses passed the frontend's real evidence validator; all six manuscript documents and revision IDs were unchanged. This pilot did not accept any replacement automatically. Actual server startup took 1.556 seconds; Linux socket inspection verified an exclusively loopback binding before requests. The server used a 512 MB maximum Java heap and no `--public` flag, remote rules, wildcard CORS or cloud provider.

Reproduce explicitly with `python3 tools/local-proofreading.py --download`, followed by `python3 tools/benchmark-proofreading.py --run`. The benchmark uses an isolated database and starts/stops only its own server. For an interactive session, explicitly run `python3 tools/local-proofreading.py --serve`, then check `http://127.0.0.1:8081` in Review's local connection settings. Close that terminal process when finished. Starting the app itself never starts this service.

The raw synthetic record and own-server log are in the workspace scratch folder as `proofreading-20261003T112614Z.json` and `.log`. This is a diagnostic pilot, not a broad grammar-quality evaluation.
