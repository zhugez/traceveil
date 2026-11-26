# Diagnostic rules

All rules consume canonical observations. Arguments use structural JSON serialization: object-key order is immaterial, array order and value types matter. Output fingerprints match recorded text exactly. No command normalization, root-cause model or external inference is used.

| Rule | Required evidence | Detection | Important exclusions |
|---|---|---|---|
| RF001 | Known arguments, explicit failure, recorded text output | Consecutive equivalent tool/arguments/output/status, default three | Success, different observations, user/state/unknown-data boundaries and excessive known timestamp gaps break the run |
| LOOP001 | Exact operation/outcome keys for failures or complete reads | Default three repetitions of a 2–4-operation block, at least two distinct operation keys | Single-operation runs are handled by other rules; other successful operations and missing/ambiguous telemetry reset the window |
| READ001 | Read tool, observed path/range arguments, explicit success, positive completeness and matching output | Default three consecutive identical recorded ranges | Any intervening non-read tool invocation, including one that later fails, invalidates read continuity. Changed/truncated/unknown content and explicit user intervention are excluded |

Only explicit user/state/unknown-data events can establish those boundaries. Repeated reads can be deliberate verification even when no boundary is recorded. A failed shell command can modify files before failing. No finding asserts that the filesystem did not change, that progress was absent or that repetition was wasteful.

Pi maps plain text read results conservatively. Known truncation metadata/continuation notices and non-text results cannot establish complete read content. New Pi output conventions may require adapter changes. Nested tool calls inside wrappers are not reconstructed.

Timestamp comparisons use recorded RFC3339 timestamps. Known gaps above the configured limit, backward timestamps and invalid timestamps reset pattern windows. User, state, custom and unknown-activity boundaries also reset these windows. In-flight calls retain their correlation identity across these boundaries. Their results cannot seed new sequence findings. Missing timestamps stay unknown. Recorded timestamps do not reveal retry intent.

RF001 and LOOP001 are medium severity because observed repetitions warrant inspection while task impact is unknown. READ001 is low severity. Severity is separate from confidence. matching_observations describes exact recorded pattern evidence, not a calibrated probability or cause confidence.

A finding includes bounded evidence, occurrence and omission counts, a rule ID, method, recommendation and limitations. Findings are ordered deterministically by discovery in the selected lineage. No automatic action follows a finding. Long groups keep early bounded evidence and aggregate counts. Warnings/findings beyond caps are counted and cause partial coverage.
