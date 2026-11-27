# Read a report

A report answers four questions:

1. What was recorded? Adapter, report identity, record counts and tool correlation counters.
2. Which patterns matched? Rule, severity, matching observations and a scoped evidence reference.
3. What could not be evaluated? Warnings, eligible counts and evaluated, partial or skipped rule status.
4. What should you investigate? An advisory next action with the rule's limitations.

No finding proves a root cause, task failure, hidden reasoning or wasted work. Timing, token cost and savings are not inferred from fingerprints or repetition.

## Coverage

The JSON complete field means supported input and required telemetry have no reported gaps. It does not mean every enabled rule had eligible evidence. Check each rule. The human summary says Limited when an enabled rule is skipped despite otherwise complete input.

An explicit suppression appears as suppressed: true and status: skipped. That is a user policy decision, not missing telemetry. Each rule shows its effective threshold. Source gaps remain partial even when rules are suppressed.

A recorded empty output is available evidence. Missing success status stays unknown. Successful non-read operations do not need output for these rules. Failed reads do not need successful-read completeness metadata.

coverage.known_success retains the original contract: it counts both recorded true and false statuses. Unknown statuses are excluded.

## References

~~~sh
traceveil demo --json
traceveil inspect f001 --demo --json
~~~

A full reference has the form @REPORT:f001. The report digest binds source bytes, selected Pi leaf, binary version, thresholds, suppression and maximum time gap. The digest is a scope check, not authentication. Equivalent effective thresholds share an identity even when their CLI representation differs. JSON exposes max_gap_seconds alongside per-rule threshold and suppression fields.

Inspection recomputes the report using original rule defaults plus explicit flags. The report template pins nondefault thresholds, suppressions and maximum gap. Repeat those options after configuration changes. Templates require the exact source and any selected leaf; join wrapped lines before running them. No transcript or report database exists. Short finding IDs require explicit source context. Full references from the built-in demo can resolve against that embedded source automatically.

Evidence uses one-based physical source record lines. It includes normalized categories and fingerprints, not excerpts. Review original records with an authorized local viewer when necessary.

## Automation

~~~sh
traceveil analyze SESSION --agent pi --json
traceveil analyze --stdin --json < events.jsonl
traceveil analyze SESSION --fail-on medium --require-complete --json
~~~

JSON mode bypasses menus, banners and color. It emits one versioned envelope. Check ok before data. Errors use stdout in JSON mode. Normal structured errors leave stderr empty. --verbose adds no output in JSON mode.

[CLI contracts and exit codes](usage.md) · [Event schema](../schemas/event.schema.json) · [Report schema](../schemas/report.schema.json)
