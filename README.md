# Traceveil

**Reveal what your agents do.**

A local-first Rust CLI that finds observable failure patterns in AI coding-agent sessions and links each finding to source evidence.

**Status: unreleased Developer Preview candidate.** Pi session v3 and canonical JSONL 1.0 pass regression tests. An authorized native Pi v3 snapshot was also checked locally. Nested codemode activity remains partial and field accuracy is unproven. Local validation covers Linux x86_64 under WSL2. Other platforms are not verified.

## First report

From this checkout:

```sh
cargo install --path . --locked
traceveil doctor
traceveil diagnose benchmarks/fixtures/demo.pi.jsonl --agent pi --compact
```

Actual output from the **synthetic fixture**, not a captured agent run:

```text
@5b8ffcdb4f736ce7f9372c68:f001 MEDIUM Repeated equivalent tool failures (3 observations)
@5b8ffcdb4f736ce7f9372c68:f002 MEDIUM Potential execution loop (6 observations)
@5b8ffcdb4f736ce7f9372c68:f003 LOW    Potentially redundant file reads (3 observations)
```

Inspect the scoped reference with the same source:

```sh
traceveil inspect '@5b8ffcdb4f736ce7f9372c68:f001' benchmarks/fixtures/demo.pi.jsonl --agent pi --json
```

For an authorized real source, pass its path instead. To select the newest modified file in one authorized directory:

```sh
traceveil diagnose --agent pi --last --session-dir /authorized/sessions --compact
```

## What it detects

- Repeated equivalent tool failures with matching arguments and recorded output.
- Candidate execution loops in bounded repeated action sequences.
- Potentially redundant reads of matching complete text ranges.

Findings are **candidates, not established causes**. Repetition can be intentional. Traceveil does not know hidden reasoning, unobserved filesystem state, full model context or task success. Missing evidence produces skipped/partial coverage. Read [rule requirements and limitations](docs/diagnostics.md).

## For agents and pipelines

```sh
traceveil diagnose benchmarks/fixtures/demo.pi.jsonl --agent pi --json
traceveil diagnose --stdin --json < benchmarks/fixtures/demo.jsonl
traceveil skills get core
traceveil --help
```

[Portable discovery skill](skills/traceveil/SKILL.md) points to instructions embedded in the binary. CLI access does not provide universal access to agent telemetry. [Usage](docs/usage.md) explains discovery, lineage, JSON, exit codes and evidence inspection.

## Privacy and architecture

Source files are read-only. Default reports and inspect contain metadata and fingerprints, never raw prompts, arguments or tool output. Discovery lists private file paths. Hashes are not anonymization. Read [SECURITY.md](SECURITY.md) before analyzing sensitive data.

Pi / canonical JSONL → ingestion adapters → one canonical event contract → shared deterministic rules → CLI / JSON / Agent Skill.

One synchronous crate, no mandatory MCP, model provider, cloud, database, daemon, browser or JavaScript runtime. See [architecture and adapter development](docs/architecture.md), [event schema](schemas/event.schema.json) and [report schema](schemas/report.schema.json).

## Demo, validation and installation

```sh
cargo build --release --locked
python3 scripts/demo.py
```

[Captured synthetic demo](benchmarks/results/demo.txt), [benchmark methodology/results](docs/benchmarks.md), and [contributor checks](CONTRIBUTING.md) are reproducible. Small labeled fixtures establish a regression baseline, not general diagnostic accuracy or improved agent outcomes.

Local native/npm artifacts can be prepared with `python3 scripts/package.py` on the verified Linux x86_64 glibc environment. They are not published. No `npm install -g traceveil` or registry Cargo install is advertised. See [installation and uninstall details](docs/usage.md).

## Scope and license

Future agent adapters, live hooks, OpenTelemetry, ACP, comparison and integrations remain optional follow-up work. No dashboard or autonomous remediation is included. [Research](docs/research/competitive-analysis.md) records observed overlap and unverified positioning assumptions.

Contributions: [CONTRIBUTING.md](CONTRIBUTING.md). License: the existing [GNU GPL version 3](LICENSE), retained unchanged.
