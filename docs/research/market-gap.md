# Market gap and feasibility

Hypothesis: a developer with an authorized existing session file can obtain relevant candidate failure patterns with inspectable evidence, without a server, external inference or transcript upload. Raw logs remain the authority. Existing projects already cover local analysis and anomaly triage.

Initial observable problems: exact consecutive tool failures, bounded repeated operation blocks, and potentially redundant matching read ranges. Test legitimate retries, file changes and deliberate verification alongside positive patterns. Findings do not establish root cause, wasted tokens, hidden reasoning or task completion.

Primary repositories checked: [AgentBoard](https://github.com/hkust-nlp/AgentBoard) describes analytical multi-turn evaluation, [AgentBench](https://github.com/THUDM/AgentBench) describes agent benchmarks, and [AgentDojo](https://github.com/ethz-spylab/agentdojo) describes attack/defense evaluation. These units differ from offline deterministic pattern classification. No benchmark scores, datasets or literature-derived accuracy claims are imported. SWE-bench trajectory applicability and agent-effectiveness experiments remain unvalidated.

Start with labeled local fixtures and healthy controls. The small held-out set is authored by the implementer, with labels fixed before its first run. Passing it is a regression baseline, not field precision/recall or proof of better agent outcomes. Authorized real Pi input is a separate required acceptance gate.
