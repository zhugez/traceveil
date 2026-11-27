# Benchmarks and validation

The tables below preserve historical v0.1 preview measurements. Current 0.2.0-rc.1 checks are indexed by [productization evidence](../benchmarks/results/productization.json). Candidate [diagnostic results](../benchmarks/results/productization-diagnostic.json) and [performance results](../benchmarks/results/productization-performance.json) identify their current executable. They do not establish field accuracy, improved agent outcomes or superiority over other tools.

## Reproduce

~~~sh
cargo build --release --locked
python3 benchmarks/diagnostic/evaluate.py --output benchmarks/results/productization-diagnostic.json
python3 benchmarks/performance/measure.py --output benchmarks/results/productization-performance.json
python3 scripts/package.py
python3 scripts/check_install.py
~~~

## Diagnostic baseline

[Labels](../benchmarks/fixtures/labels.json) were fixed before the held-out set first ran. The implementer authored the cases. Four development cases and eight held-out cases are small synthetic regression sets, not independent real-world samples. Each case is classified by presence/absence of a candidate pattern per rule. Repeated actions can still be intentional.

| Held-out rule | TP | FP | FN | TN |
|---|---:|---:|---:|---:|
| RF001 | 1 | 0 | 0 | 7 |
| LOOP001 | 1 | 0 | 0 | 7 |
| READ001 | 1 | 0 | 0 | 7 |

Each rule has only one positive and seven negative held-out labels. Do not generalize a percentage from this set. [Raw results](../benchmarks/results/diagnostic.json) include each case, denominators and evidence fingerprint verification. These labels assess observable patterns, not root causes or whether an action was truly wasteful.

## Local performance

Platform: Linux-6.18.33.2-microsoft-standard-WSL2-x86_64-with-glibc2.43. CPU: 11th Gen Intel(R) Core(TM) i5-1155G7 @ 2.50GHz. The OS exposed 6 logical CPUs. Compiler: rustc 1.97.1 (8bab26f4f 2026-07-14).

Each workload used five process invocations and 20,000 operations per input. OS cache and host activity were not controlled. These are not verified cold-start measurements. Diagnose wall times include GNU time process overhead.

| Adapter | Input bytes | Median wall ms | Highest observed peak RSS KiB |
|---|---:|---:|---:|
| jsonl | 4828890 | 646.58 | 5552 |
| pi | 8973391 | 1388.68 | 14600 |

Native release binary: 1265952 bytes. [Raw measurements](../benchmarks/results/performance.json) include every run, output size, startup samples, source fingerprint and binary SHA256. Git commit da0bc40664fa902d4c7ba7da3d16199dd8c95a59 is the initial repository commit, not a commit containing this uncommitted implementation. The runtime-source fingerprint covers src/*.rs, Cargo.toml, Cargo.lock and the embedded core guide.

## Other checks and limits

[Validation evidence](../benchmarks/results/validation.json) records 31 passing Rust tests, schema/runtime-skill checks, the known-advisory snapshot, license/source policy, source package verification and local installs. [Installation checks](../benchmarks/results/installation.json) use new prefixes and a fresh working directory with PATH=/nonexistent for the native archive, local npm tarball and Cargo binary. Build caches remain present.

[Runtime syscall check](../benchmarks/results/runtime-security.json) observed one executable invocation, no additional process creation or networking, and read-only no-follow source access in the controlled demo. Rust tests cover malicious contents, symlinks, resource caps, partial input, branches, stale references and timestamp boundaries. Neither check proves absence of all vulnerabilities.

A user-authorized native Pi v3 snapshot was analyzed locally. The cutoff precedes this validation run. It contains 193 selected entries and 85 paired outer tool calls/results. Independent counts found 83 recorded successes and two failures. The initial run exposed a correlation bug across custom metadata entries. A synthetic reproduction failed before the shared-core correction and passed afterward. See [real-case evidence](../benchmarks/results/real-pi-validation.json) and the retained pre-fix log.

No actionable rule finding appeared in this case. RF001 and LOOP001 each had two eligible observations. READ001 was skipped because no eligible complete outer read was recorded. Nested codemode activity remains unsupported. The report supports compatibility and coverage auditing. It does not establish field accuracy or improved agent outcomes. Private source paths and transcript contents are absent from the retained evidence.

No agent-effectiveness experiment, competing-tool comparison, macOS/Windows execution or older-glibc portability check is reported. The CI matrix is prepared but has not run remotely. Artifacts remain unpublished.
