# Contributing

Keep the CLI useful without a service, external model or agent-specific runtime. Put source-format logic in adapters and diagnostic behavior in the shared engine. Reuse existing code before adding dependencies or modes.

Run from the repository root:

~~~sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
uv run --no-project --with 'jsonschema>=4,<5' python scripts/check_contracts.py
python3 benchmarks/diagnostic/evaluate.py --output benchmarks/results/productization-diagnostic.json
python3 benchmarks/performance/measure.py --output benchmarks/results/productization-performance.json
python3 scripts/demo.py
python3 scripts/package.py
python3 scripts/check_install.py
python3 scripts/check_release.py
python3 scripts/check_runtime.py
python3 scripts/check_terminal.py target/release/traceveil
python3 scripts/prepare_release.py
~~~

Python, uv/jsonschema, npm, Ruby, GPG, Linux readelf and audit tools are development/distribution tools only. The release check uses a disposable GPG home. Keep historical preview receipts unchanged. The native diagnostic runtime requires none of them. With cargo-audit 0.23.0 and cargo-deny 0.22.1 installed, run cargo audit and cargo deny check licenses sources. Preserve Cargo.lock. A dependency change needs tests and an updated advisory/license check; do not expand the license policy without reviewing the dependency.

Regression fixtures must be synthetic or reviewed and authorized for publication. Never copy personal transcripts into tests by default. Label expected candidate patterns before evaluation. Keep held-out cases out of tuning and disclose when the implementer authored them. Add a focused test for a regression at the responsible layer.

For an adapter, follow docs/architecture.md and keep rules unchanged. For a rule, document evidence requirements, legitimate-repeat exclusions, uncertainty and missing-telemetry behavior. Show commands and raw results for any benchmark claim. Read SECURITY.md before handling real sessions.

No contribution workflow here authorizes commits, publication, releases, agent configuration changes or transcript uploads on a maintainer's behalf.
