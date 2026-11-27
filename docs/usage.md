# CLI contracts

Start with [your first report](getting-started.md), [Pi discovery](pi.md), [reports](reports.md), [configuration](configuration.md) or [installation](installation.md).

analyze is the primary product command. diagnose preserves explicit-source automation and its original defaults. Pi is the default adapter for files. Canonical JSONL is the default for --stdin. Standard input is not a native Pi adapter.

~~~sh
traceveil analyze --help
traceveil diagnose --help
traceveil analyze --stdin --json < events.jsonl
traceveil diagnose SESSION --agent pi --json
~~~

Canonical input is one ordered session. parent_event_id, when supplied, must refer to the preceding accepted event. Duplicate IDs, mixed sessions and ambiguous lineage fail closed. Unknown event types or missing required telemetry reduce coverage. Runtime also enforces byte, record, pending-call and evidence limits.

| Exit | Meaning |
|---|---|
| 0 | Command/report produced, possibly with findings or partial coverage |
| 1 | Explicit --fail-on severity policy matched |
| 2 | Command, input, permission or resource error |
| 3 | --require-complete rejected partial input or a skipped enabled rule |
| 130 | Interactive cancellation |

--require-complete checks source completeness and enabled rules. Explicitly suppressed rules do not count as missing telemetry. Missing source evidence still does.

--json emits one envelope and bypasses interaction and color. Normal structured errors use stdout and leave stderr empty. Help, version and shell-completion generation are explicitly requested text interfaces. --quiet suppresses successful human output while preserving status. --compact keeps a shorter human report. --color accepts auto, always or never. NO_COLOR overrides color. Terminal cursor controls remain necessary inside interactive menus.

The [event schema](../schemas/event.schema.json) and [analysis response schema](../schemas/report.schema.json) use version 1.0. The report schema covers analyze, diagnose, demo and errors. Auxiliary command data is version-enveloped but command-specific. Consumers check schema_version and ok, accept documented nulls and ignore unknown additive fields. The suppressed and threshold coverage fields are additive.

Evidence is source-bound and recomputed. Changes to source bytes, leaf, rule policy, maximum gap or binary version invalidate references. CLI input is never executed. There is no automatic remediation or persisted report cache.
