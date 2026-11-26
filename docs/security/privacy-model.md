# Privacy model

The runtime reads only the selected source and, when explicitly requested, one non-recursive session directory. Pi parentSession paths and paths inside tool arguments are not followed. There is no hidden telemetry, upload, external model, persistent index or report cache.

Raw record content exists temporarily in memory while parsing and fingerprinting. Pi retains bounded IDs/parent links until lineage selection. The rule engine retains hashes, safe categories, statuses and evidence line references. Process memory and swap are not securely erased. OS access controls still matter.

Default reports and inspect never include raw prompts, outputs or arguments. No automated secret classifier is used: raw content is omitted instead of claiming complete secret detection. Hashes can reveal guesses about low-entropy values. Statuses, repeated actions, timing policy and line numbers also reveal behavior.

The sessions command lists actual paths from the authorized directory. Review that metadata before sharing it. Standard JSON escaping and CLI control/bidi escaping prevent terminal control effects; they do not make paths non-sensitive.

Traceveil writes only stdout/stderr during analysis. If the caller redirects a report, that caller controls retention and deletion. Build scripts, benchmark scripts and package tools create explicit development artifacts, not hidden runtime data.

Never publish a real transcript merely to reproduce a bug. Prefer a synthetic or deliberately sanitized fixture. Review consent and identifiers separately before any real-data contribution.
