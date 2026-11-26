# Security

Traceveil processes untrusted, potentially private session logs. Core diagnostics run locally and read sources without modifying them. No runtime network client, external inference, command execution or report database is included.

Reports contain source line numbers, known tool categories, statuses and SHA256 fingerprints. Raw prompts, arguments, paths inside records and tool outputs are omitted, including from inspect. Session discovery deliberately lists file paths. Paths are private metadata. Fingerprints limit raw-content exposure; they are not anonymization or encryption.

Read [the threat model](docs/security/threat-model.md) and [privacy model](docs/security/privacy-model.md). Automated controls do not guarantee protection against all secrets, prompt injection or filesystem races on every platform.

For a suspected vulnerability, use private reporting in the repository Security tab if available. Otherwise request a private channel with a minimal issue. Do not include transcripts, credentials or exploitable private data in public issues. No private reporting address or response-time guarantee is advertised.

Before release, run the tests, contract checks and dependency checks in CONTRIBUTING.md. Only tested platforms and source formats may be described as supported.
