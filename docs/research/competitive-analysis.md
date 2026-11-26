# Competitive analysis

Checked 2026-11-27 (+07:00) through GitHub repository/release APIs and upstream READMEs. All eight repositories resolved with archived=false. A recent push is an activity signal, not maintenance quality. No competitor binary was installed or benchmarked. Capabilities are upstream descriptions rather than independently verified implementations.

| Repository | Category | GitHub release observed | License API | Last push (UTC) |
|---|---|---|---|---|
| [vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser) | agent utility | v0.45.0 | Apache-2.0 | 2026-11-26T15:25:23Z |
| [vercel-labs/agent-desktop](https://github.com/vercel-labs/agent-desktop) | agent utility | v1.3.1 | Apache-2.0 | 2026-11-25T23:55:29Z |
| [vercel-labs/skills](https://github.com/vercel-labs/skills) | skill distribution | v1.9.0 | MIT | 2026-11-17T00:58:04Z |
| [camtrik/agent-trail](https://github.com/camtrik/agent-trail) | session viewer/dashboard | v0.2.0 | MIT | 2026-11-11T14:26:05Z |
| [mishanefedov/agentwatch](https://github.com/mishanefedov/agentwatch) | local observability/triage | v0.2.0 | MIT | 2026-07-09T14:48:43Z |
| [mstuart/peek](https://github.com/mstuart/peek) | session analysis/evaluation | None returned | MIT | 2026-10-21T18:28:33Z |
| [clay-good/agent-replay](https://github.com/clay-good/agent-replay) | trace collection/replay/evaluation | None returned | MIT | 2026-11-22T21:16:24Z |
| [Dicklesworthstone/coding_agent_session_search](https://github.com/Dicklesworthstone/coding_agent_session_search) | session search | v0.11.0 | NOASSERTION | 2026-11-26T17:15:56Z |

Agent Trail describes local history/tool replay and token/cost dashboards. agentwatch includes anomaly triage. peek includes historical context analysis and Pi support. agent-replay includes local SQLite, replay, diff and deterministic/LLM evaluations. cass includes cross-agent search and bounded agent output. Do not claim Traceveil invented those capabilities or that a CLI/local execution alone differentiates it.

GitHub NOASSERTION is not a license grant. cass README describes MIT plus an OpenAI/Anthropic rider. Review the terms before reuse. Traceveil copies no source from these projects.

Adjacent repositories checked through their API descriptions: [Langfuse](https://github.com/langfuse/langfuse), [Phoenix](https://github.com/Arize-ai/phoenix), [OpenInference](https://github.com/Arize-ai/openinference), [AgentOps](https://github.com/AgentOps-AI/agentops). They describe observability/evaluation platforms or instrumentation SDKs. This is not a capability/license audit. Their API license results were NOASSERTION, NOASSERTION, Apache-2.0 and MIT respectively.

Preference for Traceveil remains a hypothesis: an existing file → candidate diagnosis → bounded source evidence, without a server or model call. No superiority or workflow improvement is established.
