# Vercel agent tooling study

Checked 2026-11-27 (+07:00). Source snapshots: [agent-browser a67ba08](https://github.com/vercel-labs/agent-browser/tree/a67ba089fec5fbfec07346aebad11666463d9799), [agent-desktop ce2ddf0](https://github.com/vercel-labs/agent-desktop/tree/ce2ddf07226346818de2a1c6bae9869cfad142df), [skills 57217cf](https://github.com/vercel-labs/skills/tree/57217cf86ba38148a57af9e90d14a4c049558330). Read their READMEs plus browser cli/src/skills.rs, package.json, scripts/postinstall.js and desktop crates/core/src/commands/skills.rs.

| Pattern | Why it helps | Traceveil adaptation | Maintenance/validation |
|---|---|---|---|
| Capability CLI | Calling agent owns reasoning and action selection | One offline Rust binary, no autonomous agent or daemon | CLI tests exercise real process entry point |
| Scoped references | Desktop references identify one snapshot | Digest-scoped finding IDs, recomputed inspection | Source/leaf/threshold/version changes must reject stale refs |
| Discovery/runtime skills | Browser separates discovery stubs and runtime guides; desktop embeds guides with include_str! | One portable stub and one embedded core guide | Test guide version and command examples; add modules only when needed |
| JSON envelope | Explicit success/data/error contracts support agents | Versioned JSON with data-only stdout | Parse stdout, check stderr and exit codes |
| Native npm packaging | Browser optional platform packages locate a native binary; postinstall can download if absent | Prepare local native artifacts first, avoid an unverified downloader | Clean local package/install smoke tests; publish separately with permission |
| Progressive disclosure | Compact views lead to scoped detail | compact findings → inspect metadata/evidence lines | Bound evidence and output; do not claim token savings without measuring |

The [Agent Skills specification](https://agentskills.io/specification) was read on this date. It requires name/description frontmatter and a directory matching the skill name. The fetched page did not expose a release number; none is invented.

[ACP overview](https://agentclientprotocol.com/protocol/overview) describes negotiated JSON-RPC client/agent sessions and session/update notifications. Reference: [ACP 927e318](https://github.com/agentclientprotocol/agent-client-protocol/tree/927e31867b31b1e3c961c7edee7e3680b90be718). It does not expose arbitrary existing terminal sessions.

The [OpenTelemetry GenAI page](https://opentelemetry.io/docs/specs/semconv/gen-ai/) points to the separate [GenAI repository 81b5d43](https://github.com/open-telemetry/semantic-conventions-genai/tree/81b5d438519f868a186bff088d9de24aefdb2f6b). [OTLP 3a6e438](https://github.com/open-telemetry/opentelemetry-proto/tree/3a6e438003e673112efd4a00e9836c00672eae3c) and [OpenInference 2d10c59](https://github.com/Arize-ai/openinference/tree/2d10c590ad6a779382e740ca6182e70dee54f1e3) are future mapping references. No version of these standards is claimed as implemented. Canonical JSONL 1.0 is an offline interchange contract, not a competing live protocol. No branding or affiliation is copied.
