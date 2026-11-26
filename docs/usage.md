# Usage and contracts

## Install and first report

From this source checkout:

~~~sh
cargo install --path . --locked
traceveil doctor
traceveil diagnose benchmarks/fixtures/demo.pi.jsonl --agent pi --compact
~~~

The demo is synthetic. Source installation requires Rust; running the resulting native binary does not require Rust, Node, Python or network access. The npm registry package is not published. Local native/npm packaging is described below.

For a real session, explicitly select a file you are authorized to read:

~~~sh
traceveil diagnose /authorized/session.jsonl --agent pi --json
traceveil sessions --session-dir /authorized/sessions --json
traceveil diagnose --agent pi --last --session-dir /authorized/sessions --compact
~~~

Without --session-dir, explicit sessions/--last requests use PI_CODING_AGENT_SESSION_DIR when set, otherwise Pi's current-working-directory session folder under HOME or USERPROFILE. Pi settings.json is not read. Discovery is one directory only, skips symlinks and sorts by modification time. --last means newest modified file, not verified task completion or active runtime state.

Pi files must have a v3 header. v1/v2 files are rejected without migration. Default lineage ends at the last stored entry, including a metadata entry. --leaf ID selects another stored tip/ancestor. Branch navigation that Pi did not persist cannot be recovered from the file. No parentSession file is followed.

## Evidence inspection

Use the complete reference from diagnose, with the same source and options:

~~~text
traceveil inspect REFERENCE SESSION --agent pi --json
~~~

A reference includes a report digest. Bare @f_01 references are not accepted. Inspect recomputes the report; it does not read a cache. Input bytes, leaf, repeat threshold, maximum time gap and binary version affect references. The shortened report digest scopes references, not authentication. Full source SHA256 remains in JSON.

Evidence contains one-based physical record lines and normalized event/block references. call_record_line identifies the invocation record when available. Inspect those lines locally with an authorized viewer if raw details are necessary. Traceveil does not print raw excerpts.

## Canonical JSONL and output

~~~sh
traceveil diagnose benchmarks/fixtures/demo.jsonl --agent jsonl --json
traceveil diagnose --stdin --json < benchmarks/fixtures/demo.jsonl
~~~

Default adapter: pi for files, jsonl for stdin. Canonical input is one ordered session. If parent_event_id is supplied, it must refer to the immediately preceding accepted event. Duplicate IDs and mixed sessions fail. Unknown event types produce a coverage gap. Invalid records can produce partial results; structural ambiguity fails closed.

The [event schema](../schemas/event.schema.json) describes version 1.0. Runtime additionally enforces UTF-8 byte and resource ceilings. A tool result may be self-contained with explicit arguments, or correlate through call_id to a started event. Unknown success stays null. Tool result text is recorded evidence, not proof of actual filesystem state.

--json emits one envelope. stdout contains data; normal command errors are static structured errors in JSON mode. --compact is mutually exclusive with --json. Color is never emitted, so NO_COLOR and redirection need no special handling. --repeat-threshold accepts 3–16, default 3. --max-gap-seconds accepts 1–86400, default 600. Missing timestamps remain unknown; invalid/backward timestamps break evidence continuity and add warnings.

Coverage counters describe accepted normalized observations. coverage.known_success counts recorded true or false success statuses; both successful and failed observations contribute. Unknown statuses are excluded.

complete means supported input and required rule telemetry have no reported gaps. Missing status, failure output or successful-read evidence adds a coverage warning, even when other observations are eligible. Explicitly incomplete read ranges also reduce coverage. Recorded empty output remains available evidence. Successful non-read operations do not require output for these rules. It does not mean every rule had telemetry, that the task succeeded or that the agent had no defect. Inspect per-rule evaluated/partial/skipped status and eligible counts. rules explains limitations; doctor exposes limits.

| Exit | Meaning |
|---|---|
| 0 | Report/command produced successfully, possibly with findings or partial coverage |
| 1 | Explicit --fail-on severity policy matched |
| 2 | CLI, source, schema, permission or resource error |
| 3 | Explicit --require-complete policy failed due to partial input or skipped rules |

The [report schema](../schemas/report.schema.json) covers diagnose success/error envelopes. Additive output fields are permitted. Consumers must check schema_version and ok, accept documented nulls, and ignore unknown additive fields. Breaking semantic/required-field changes require a new schema version. Preview contracts can evolve through an explicit version change.

## Local distribution preparation

On the verified Linux x86_64 glibc environment:

~~~sh
python3 scripts/package.py
cd target/dist
sha256sum -c SHA256SUMS
~~~

The native archive contains the executable, license and security/readme text. The local npm tarball links directly to the native executable and has no dependencies, install scripts or downloader. It is marked private to prevent accidental publication. A maintainer must separately authorize any publication. Checksums detect corruption; they are not signed provenance. No updater is included. Replace the binary only after checking the new version/checksum.

Uninstall a directly installed binary by removing that selected binary. For a Cargo install, use cargo uninstall traceveil. For an npm prefix install, use npm uninstall --prefix PREFIX traceveil. Traceveil creates no runtime database or daemon to remove.
