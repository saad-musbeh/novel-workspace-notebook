# Development Working Rules

This file describes the application's engineering constraints, not instructions to override any assistant's safety or system requirements.

Build one small, testable vertical slice at a time. The accepted requirements and current milestone are in MASTER_PLAN.md and PROJECT_STATE.md. Do not claim access to the user's PC or phone unless an actual authorized tool provides it. Generated code is not a passed test.

Preserve local-first writing, multilingual text, revision history, and explicit conflict handling. Never silently overwrite concurrent manuscript versions, turn AI claims into canon, apply stale offsets, or sync active database files. Do not make the LLM necessary for editing or saving.

No cloud AI, automatic cloud fallback, paid editor extension, automatic model download, telemetry, public model endpoint, or unrestricted remote command execution. Tailscale is the approved private transport; application authentication and project authorization are still required. Do not disable SELinux or firewall protections to avoid diagnosing a setup issue.

Ask before installing dependencies, changing system settings, enabling persistent services, opening network access, downloading large packs/models, or doing destructive database operations. A read-only inspection and ordinary source-file edits are distinct from those actions.

Keep credentials, signing keys, personal reports, manuscript files, fonts, and model weights out of the repository unless an explicit, appropriate asset policy permits a particular non-secret item. Use synthetic multilingual fixtures, not private writing, for general tests.

Pin dependencies after compatibility checks. Review licenses of libraries, dictionaries, and model weights separately. Keep migrations recoverable. Update tests, decisions, and PROJECT_STATE.md with observed results after each completed slice.
