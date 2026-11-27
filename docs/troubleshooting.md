# Troubleshooting

| Symptom | Action |
|---|---|
| No session found | Run from the correct project. Use sessions --agent pi or init --session-dir DIR. Try demo without touching real data. |
| DISCOVERY_AUTHORIZATION_REQUIRED | Select an explicit file/directory. A project settings redirect needs separate approval. JSON mode cannot prompt. |
| SESSION_AMBIGUOUS | Use --last or an exact path from sessions. |
| SOURCE_CHANGED | Wait for writes to stop or analyze an authorized stable copy. |
| STALE_REFERENCE | Analyze the same bytes, leaf and policy again. Copy the new reference. |
| EVIDENCE_CONTEXT_REQUIRED | Supply the source or --demo. A short finding ID is not a global lookup. |
| Partial or skipped rule | Check warnings and eligible observations. Unsupported or absent telemetry is not a clean result. |
| DESTINATION_UNSAFE | Use a physical configuration/skill directory without symlinks. Traceveil refuses redirected writes. |
| SKILL_MODIFIED / SKILL_NOT_OWNED | Preserve the files. --force explicitly authorizes replacing SKILL.md in that scope after review. Receipts identify known distributed content, not ownership. Unknown receipts require manual review. |
| Command not found after install | Add the chosen prefix/bin to PATH. Installers print a session-local command and never edit shell startup files. |
| Unsupported Pi version | Use a supported v3 source. Traceveil never migrates the original. |

Use traceveil doctor --json for adapter and limit information. Share the version, platform and a synthetic reproduction. Review paths and fingerprints before attaching a report. Raw transcripts can contain credentials and hostile instructions.

Escape or q cancels a selection menu. Ctrl-C exits with status 130. Linux PTY checks cover terminal and cursor restoration. Native Windows/macOS terminal checks remain release gates.
