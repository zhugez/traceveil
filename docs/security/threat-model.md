# Threat model

## Assets and trust boundaries

Session files can contain credentials, source code, personal information and hostile instructions. Import records as data. The caller authorizes a file or one discovery directory. Contents cannot grant additional filesystem or execution permissions.

| Threat | Control | Regression evidence / limitation |
|---|---|---|
| Commands or paths embedded in logs | No subprocess/network API in the diagnostic library; referenced files are never opened | Hostile-content test checks a would-be command marker; syscall trace checks one controlled CLI run |
| Secret leakage through reports/errors | Metadata-only evidence, hashed IDs/arguments/output, static parser errors, unknown tool names reported as other | Tests include synthetic credentials and escape sequences; file discovery still lists private paths |
| Terminal and bidi injection | JSON escaping plus terminal-control/bidi escaping at CLI output | Session filename regression verifies safe bytes and reversible JSON values |
| Symlink/FIFO source input | Reject final symlinks/non-regular files; Unix opens use O_NOFOLLOW/O_NONBLOCK and compare inode/device | Final-link regression; parent-directory symlinks are not a sandbox boundary. Windows hardening is not verified here |
| Oversized/deep/malformed input | Byte, record, event, pending-call, window, warning and evidence caps; serde_json recursion limit | Limits tests cover oversized input, raw records, pending calls, findings and evidence. Memory is bounded by caps, not constant in session size |
| Live file mutation / branch contamination | Pi indexes one ancestry, rejects duplicate IDs/broken links, compares both pass digests before returning | Tests cover alternate roots, metadata leaf, stale references and a reader that changes between passes |
| Missing/unknown telemetry | Break pattern continuity and report partial/skipped coverage | No unobserved state or unknown success is invented |
| Prompt injection in evidence | No raw excerpts and no automatic action | A calling agent must still treat source records as untrusted; the CLI cannot secure that agent |
| Dependency compromise | Locked dependency graph, advisory and license checks | A passing advisory check detects known advisories only. It does not prove source or binary trust |

## Limits

Default ceilings are exposed by traceveil doctor. A source record includes its newline in the byte limit. Pi stores only bounded lineage metadata between passes. Canonical streams are parsed incrementally. Diagnostic windows retain at most 64 operations, not transcripts.

No shell command, model inference, source-file repair, live interception, external report upload or autonomous remediation occurs. Source permissions and path selection remain the caller's responsibility. Findings identify candidates, not verified root causes.
