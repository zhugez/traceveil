# Productization plan

Baseline: 5db158a, a synchronous Rust crate with Pi v3 and canonical JSONL adapters, three deterministic rules, source-bound evidence, 31 tests, local packaging and metadata-only validation. The working tree contains user-owned .pi state, which remains excluded.

## Initial audit

Reuse the bounded two-pass Pi reader, correlation and pattern boundaries, warning-driven partial coverage, schemas, labeled fixtures, offline core guide, packaging checks and measured benchmark method. Gaps at the baseline: required subcommand/source, no installed demo, plain JSON for auxiliary human commands, no interactive discovery, no rule policy, no installers, Linux-only package script and no product assets. The existing macOS CI failed because its filesystem rejected the invalid-UTF-8 filename used by a test. Preserve that security assertion through a portable boundary test and explicit filesystem-capability handling.

Upstream Pi reference: earendil-works/pi d8c76f0a3f8dd2b9e9bb9c98913c78f640c2ee66. Verified sessionDir settings, PI_CODING_AGENT_DIR, PI_CODING_AGENT_SESSION_DIR, encoded project folders, and project/user skills directories. Detection is not adapter verification.

## P0 — complete user journey

1. Add analyze, standalone production-engine demo, safe source-bound demo inspection and a useful non-TTY welcome. Preserve diagnose, envelope 1.0 and exit codes.
2. Add scoped Pi discovery with header-only metadata, project filtering, explicit authorization and active/incomplete hints. No home recursion or paths granted by untrusted headers.
3. Add a small TTY menu, cancellation, quiet/verbose/color controls, readable reports and actionable errors. JSON output bypasses prompts, colors and verbose messages.
4. Add private user configuration, explicit rule thresholds/suppression and visible effective policy. Do not load repository-controlled Traceveil policy automatically.
5. Create branding, product-first README and task-oriented documentation with executed examples.
6. Verify a fresh-directory installation, demo, discovery, analysis, inspection, export and uninstall without repository access at runtime.

## P1 — distribution and integration

Prepare secure POSIX/PowerShell installers, deterministic native archives, npm distribution, a generated Homebrew formula, release metadata/checksums/attestations and native CI smoke jobs. Implement Pi skill installation only into an explicitly selected scope and preserve unrelated/modified files. Generate shell completions. Keep credentials, package ownership, actual platform execution and remote CI as explicit release gates.

## Local candidate status

The P0 journey is implemented and exercised on Linux x86_64/WSL2 with fresh homes and working directories. Native archives, the POSIX installer, private local npm and clean-prefix Cargo installation pass demo, discovery, analysis, inspection, configuration and optional skill checks. The Rust suite has 48 passing tests. The PTY suite has 11 passing cases, including arrow keys and interrupt cleanup.

P1 includes checksums, local build evidence, explicit optional GPG signing, native artifact preparation and a manual CI workflow. Signing tests use and remove a disposable key. The default candidate remains unsigned. Archive replacement and leading-dash filename regressions exercise installer safety. Current command logs and source inventories are indexed by [productization.json](../../benchmarks/results/productization.json).

Native Windows/macOS execution, PowerShell, Homebrew, public download installation, package ownership and maintainer signing identity remain release gates. The manual workflow is prepared and has not been run remotely for this worktree. Advisor input does not constitute a full-source security endorsement.

## P2 — evaluated, not prebuilt

Additional adapters need source/version research, sanitized fixtures and authorized real validation. No directory-only support claims. No speculative diagnostics, health scores, daemon, dashboard, hooks, telemetry, database or MCP runtime.

## Validation and release boundary

Run core/CLI/schema/privacy tests, isolated configuration and skill ownership tests, PTY keyboard/cancel checks, installer corruption/overwrite/uninstall checks, local package installation and reproducibility comparisons. Retain actual outputs and timings. Advertise only exercised platforms. Public packages, tags, releases and pushes require separate authorization. No new publish command is run by this plan.
