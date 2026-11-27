# Developer Preview evidence — historical v0.1

This record describes the earlier preview. That preview was approved, committed and pushed at 5db158a24795f8a6c3e0e34e8e850933f0a91165. The current 0.2.0-rc.1 work is described in [PRODUCTIZATION.md](PRODUCTIZATION.md). Its receipts use the productization prefix and do not replace this historical evidence.

## Current status

The local Developer Preview candidate passed the documented checks, including an authorized native Pi v3 real-session evaluation. The user resumed final acceptance after the earlier input blocker was resolved. This document records local evidence. It does not announce a public release or an independent security endorsement.

The repository started at da0bc40664fa902d4c7ba7da3d16199dd8c95a59 with README.md, .gitignore and the existing GPLv3 LICENSE. The implementation described below was later committed at 5db158a24795f8a6c3e0e34e8e850933f0a91165. User-owned .pi state remains outside the implementation and packages.

## MVP evidence map

| Area | Implementation and evidence |
|---|---|
| Session discovery and selection | src/main.rs and src/adapters.rs. Scoped discovery and CLI regressions |
| Pi v3 lineage and correlation | src/adapters.rs. Native counts, synthetic branches and custom-boundary regression |
| Canonical file/stdin ingestion | src/events.rs. Event schema and CLI/schema checks |
| Repeated failures | RF001. Labeled positive, healthy, retry and boundary cases |
| Potential execution loops | LOOP001. Bounded repeated blocks and boundary/overlap cases |
| Potentially redundant reads | READ001. Complete-range, mutation, verification and unknown-data cases |
| Provenance and inspection | src/report.rs. Scoped references, recomputation and stale-input tests |
| Output and error policies | Human/compact/versioned JSON. CLI tests for exits 0, 1, 2 and 3 |
| Privacy and resource limits | Metadata-only evidence, escaping, bounded input and pending calls. Malicious-input tests and syscall checks |
| Diagnostic and runtime baselines | Twelve labeled synthetic cases, raw performance measurements and disclosed environment/cache limits |
| Agent-facing instructions | skills/traceveil/SKILL.md and version-matched embedded guide. Runtime examples checked |
| Demo, documentation and local distribution | Actual synthetic demo, native/private npm/Cargo install checks and prepared CI |

Paths in this table are repository-relative. Current commands and primary logs are in the [validation record](../../benchmarks/results/validation.json). Each receipt identifies its source snapshot or unchanged binary inputs. The declared source set is in the [source manifest](../../benchmarks/results/source-manifest.json). See [benchmark methodology](../benchmarks.md).

## Observed validation

The Rust suite has 31 passing tests. The latest labeled diagnostic evaluation and native/private npm/Cargo installation checks passed. All three installed binaries matched the rebuilt native executable. Runtime checks used a fresh working directory with PATH=/nonexistent. Build caches were retained.

The private real-case snapshot had 85 outer calls and 85 matching results. The core initially discarded a pending identity across custom metadata and reported an extra call. The shared-core correction preserves identities across boundaries and excludes their late results from sequence findings. Unresolved identities remain capped at 256 and are counted once at input end. Regressions cover exact capacity, overflow, duplicate IDs, all rules and interleaved results.

Real-case JSON output was deterministic for identical bytes. The source prefix and temporary snapshot remained unchanged during checks. The captured analysis launched no extra process or enumerated network syscall. Raw temporary inputs and discovery metadata were removed. The retained real-case evidence contains counts, hashes and fixed categories.

The real report remains partial because nested codemode activity is unsupported. RF001 and LOOP001 had two eligible observations each. READ001 was skipped and no finding appeared. This supports compatibility and coverage auditing. It does not establish positive diagnostic accuracy, native branching in that case or improved agent outcomes. Branching and positive finding predicates remain synthetic regression coverage.

The completion audit also exposed a mixed-telemetry coverage defect. Missing status, required output and successful-read range or scope evidence now add coverage warnings. Existing eligible observations remain partial, unknown-only rules remain skipped, and --require-complete returns 3. CLI regressions cover null and absent fields, known truncation, recorded empty output, failed-read controls and retained valid findings. The pre-fix failure is retained in benchmarks/results/check-logs/mixed-coverage-before-fix.log.

## Limits and handoff

Only Linux x86_64 WSL2/glibc was exercised. Remote CI, macOS/Windows and older-glibc portability remain unverified. The small labeled corpus was authored by the implementer. Its results do not establish field accuracy. Advisor review concerns the visible decisions and excerpts. No full-source security sign-off is claimed.

These validation checks preceded the approved preview commit and push. They did not publish a package, deploy, change the license, upload a transcript or change live agent configuration. Public release and signed provenance require separate work and authorization. Dashboard, daemon, database, extra agent adapters, live hooks, OTel/ACP and optional integrations remain deferred.
