use crate::{
    Error, MAX_EVIDENCE, MAX_FINDINGS, MAX_PENDING, MAX_RECORDS, Result, SCHEMA_VERSION, VERSION,
    digest,
    events::{Data, SourcedEvent, known_tool},
    report::{Evidence, Finding, Report, RuleCoverage},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

pub const RULES: [(&str, &str); 3] = [
    (
        "RF001",
        "Exact consecutive failed operations with matching recorded output. Intent and time-based retry policy are unknown.",
    ),
    (
        "LOOP001",
        "Repeated blocks of 2–4 recorded operations. Matching observations do not prove lack of progress.",
    ),
    (
        "READ001",
        "Consecutive complete matching read ranges. Matching content does not establish that verification was unnecessary.",
    ),
];

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub repeat_threshold: usize,
    pub max_gap_seconds: u32,
    pub rule_thresholds: [Option<usize>; 3],
    pub disabled_rules: [bool; 3],
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            repeat_threshold: 3,
            max_gap_seconds: 600,
            rule_thresholds: [None; 3],
            disabled_rules: [false; 3],
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<Self> {
        if !(3..=16).contains(&self.repeat_threshold) {
            return Err(Error::new(
                "CONFIGURATION_INVALID",
                "The repeat threshold must be between 3 and 16.",
            ));
        }
        if !(1..=86_400).contains(&self.max_gap_seconds) {
            return Err(Error::new(
                "CONFIGURATION_INVALID",
                "The maximum known time gap must be between 1 and 86400 seconds.",
            ));
        }
        if self
            .rule_thresholds
            .iter()
            .flatten()
            .any(|n| !(3..=16).contains(n))
        {
            return Err(Error::new(
                "CONFIGURATION_INVALID",
                "Rule thresholds must be between 3 and 16.",
            ));
        }
        Ok(self)
    }
    fn threshold(self, rule: usize) -> usize {
        self.rule_thresholds[rule].unwrap_or(self.repeat_threshold)
    }
}
struct Call {
    tool: String,
    arguments: Option<String>,
    line: usize,
    overlapping: bool,
    read_scope_known: bool,
}
#[derive(Clone)]
struct Operation {
    key: String,
    evidence: Evidence,
}
#[derive(Default)]
struct Run {
    key: String,
    count: usize,
    evidence: Vec<Evidence>,
    finding: Option<usize>,
}
pub struct Engine {
    pub report: Report,
    settings: Settings,
    session: Option<String>,
    pending: HashMap<String, Call>,
    seen_calls: HashSet<String>,
    seen_results: HashSet<String>,
    failures: Run,
    reads: Run,
    window: VecDeque<Operation>,
    position: usize,
    last_loop: Option<(String, usize, Option<usize>)>,
    eligible: [usize; 3],
    processed_events: usize,
    last_timestamp: Option<time::OffsetDateTime>,
}
impl Engine {
    pub fn new(adapter: &'static str, settings: Settings) -> Result<Self> {
        Ok(Self {
            report: Report::new(adapter),
            settings: settings.validate()?,
            session: None,
            pending: HashMap::new(),
            seen_calls: HashSet::new(),
            seen_results: HashSet::new(),
            failures: Run::default(),
            reads: Run::default(),
            window: VecDeque::new(),
            position: 0,
            last_loop: None,
            eligible: [0; 3],
            processed_events: 0,
            last_timestamp: None,
        })
    }
    pub fn warning(&mut self, code: &'static str, line: Option<usize>) {
        self.report.warn(code, line);
    }
    fn reset_patterns(&mut self) {
        self.failures = Run::default();
        self.reads = Run::default();
        self.window.clear();
        self.last_loop = None;
    }
    pub fn gap(&mut self) {
        self.reset_time_window();
        self.last_timestamp = None;
    }
    pub fn set_session(&mut self, session: &str) -> Result<()> {
        if session.is_empty() || session.len() > 128 {
            return Err(Error::input());
        }
        let key = digest(session);
        if self.session.as_ref().is_some_and(|s| s != &key) {
            return Err(Error::new(
                "MULTIPLE_SESSIONS",
                "Analyze one session at a time.",
            ));
        }
        self.report.session_sha256 = key.clone();
        self.session = Some(key);
        Ok(())
    }
    fn reset_time_window(&mut self) {
        self.reset_patterns();
        for call in self.pending.values_mut() {
            call.overlapping = true;
        }
    }
    pub fn observe_timestamp(&mut self, timestamp: Option<&str>, line: usize) {
        if let Some(timestamp) = timestamp {
            let parsed = if timestamp.len() <= 128 {
                time::OffsetDateTime::parse(
                    timestamp,
                    &time::format_description::well_known::Rfc3339,
                )
                .ok()
            } else {
                None
            };
            if let Some(now) = parsed {
                if let Some(previous) = self.last_timestamp {
                    let gap = now - previous;
                    if now < previous {
                        self.warning("TIMESTAMP_ORDER_UNCERTAIN", Some(line));
                        self.reset_time_window();
                    } else if gap
                        > time::Duration::seconds(i64::from(self.settings.max_gap_seconds))
                    {
                        self.reset_time_window();
                    }
                }
                self.last_timestamp = Some(now);
            } else {
                self.warning("TIMESTAMP_INVALID", Some(line));
                self.reset_time_window();
                self.last_timestamp = None;
            }
        }
    }
    pub fn push(&mut self, input: SourcedEvent) -> Result<()> {
        self.processed_events += 1;
        if self.processed_events > MAX_RECORDS {
            return Err(Error::new("INPUT_TOO_LARGE", "Too many normalized events."));
        }
        let event = input.event;
        self.observe_timestamp(event.timestamp.as_deref(), input.line);
        if event.schema_version != SCHEMA_VERSION {
            return Err(Error::new(
                "SCHEMA_VERSION_UNSUPPORTED",
                "The canonical event schema version is not supported.",
            ));
        }
        self.set_session(&event.session_id)?;
        if event.event_id.is_empty() || event.event_id.len() > 128 {
            return Err(Error::input());
        }
        match event.event_type.as_str() {
            "tool.execution.started" => self.start(event.data, input.line),
            "tool.execution.completed" => {
                self.complete(event.data, input.line, input.block, &event.event_id)
            }
            "message.assistant" => Ok(()),
            "session.started"
            | "message.user"
            | "state.changed"
            | "context.compacted"
            | "session.ended"
            | "configuration.changed" => {
                self.gap();
                Ok(())
            }
            _ => {
                self.report.coverage.unknown_records += 1;
                self.warning("EVENT_TYPE_UNSUPPORTED", Some(input.line));
                self.gap();
                Ok(())
            }
        }
    }
    fn start(&mut self, data: Data, line: usize) -> Result<()> {
        let tool = data
            .tool
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(Error::input)?;
        let call = data
            .call_id
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or_else(Error::input)?;
        let id = digest(call);
        if !self.seen_calls.insert(id.clone()) {
            return Err(Error::new(
                "DUPLICATE_TOOL_CALL",
                "Tool call identifiers must be unique within a session.",
            ));
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(Error::new(
                "INPUT_TOO_LARGE",
                "Too many outstanding tool calls.",
            ));
        }
        let overlapping = !self.pending.is_empty();
        if overlapping {
            self.failures = Run::default();
            self.reads = Run::default();
            self.window.clear();
            self.last_loop = None;
            for call in self.pending.values_mut() {
                call.overlapping = true;
            }
            self.warning("OVERLAPPING_TOOL_CALLS", Some(line));
        }
        if tool != "read" {
            self.reads = Run::default();
        }
        let arguments = argument_hash(data.arguments.as_ref())?;
        let read_scope_known = data
            .arguments
            .as_ref()
            .is_some_and(|v| v["path"].as_str().is_some_and(|s| !s.is_empty()));
        self.pending.insert(
            id,
            Call {
                tool,
                arguments,
                line,
                overlapping,
                read_scope_known,
            },
        );
        self.report.coverage.tool_calls += 1;
        Ok(())
    }
    fn complete(&mut self, data: Data, line: usize, block: usize, event_id: &str) -> Result<()> {
        let tool = data
            .tool
            .as_deref()
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(Error::input)?;
        if data.success == Some(true) && data.exit_code.is_some_and(|c| c != 0) {
            return Err(Error::input());
        }
        let call = if let Some(id) = data.call_id.as_ref() {
            if id.is_empty() || id.len() > 256 {
                return Err(Error::input());
            }
            let id = digest(id);
            if !self.seen_results.insert(id.clone()) {
                return Err(Error::new(
                    "DUPLICATE_TOOL_RESULT",
                    "A tool call can have only one completed result.",
                ));
            }
            self.pending.remove(&id)
        } else {
            None
        };
        if call.as_ref().is_some_and(|c| c.tool != tool) {
            return Err(Error::new(
                "TOOL_CORRELATION_INVALID",
                "The result tool does not match its invocation.",
            ));
        }
        let supplied = argument_hash(data.arguments.as_ref())?;
        if let Some(c) = &call
            && c.arguments.is_some()
            && supplied.is_some()
            && c.arguments != supplied
        {
            return Err(Error::new(
                "TOOL_CORRELATION_INVALID",
                "The result arguments do not match their invocation.",
            ));
        }
        let arguments = supplied.or_else(|| call.as_ref().and_then(|c| c.arguments.clone()));
        if call.is_none() {
            self.report.coverage.tool_calls += 1;
        }
        self.report.coverage.tool_results += 1;
        if data.success.is_some() {
            self.report.coverage.known_success += 1;
        }
        if arguments.is_none() {
            self.report.coverage.unpaired_calls += 1;
            self.warning("TOOL_ARGUMENTS_UNAVAILABLE", Some(line));
            self.gap();
            return Ok(());
        }
        let read_scope_known = data
            .arguments
            .as_ref()
            .is_some_and(|v| v["path"].as_str().is_some_and(|s| !s.is_empty()))
            || call.as_ref().is_some_and(|c| c.read_scope_known);
        let read_complete = if tool == "read" {
            data.output_complete
        } else {
            None
        };
        if data.success.is_none() {
            self.warning("TOOL_STATUS_UNAVAILABLE", Some(line));
        }
        if data.output.is_none() && (data.success != Some(true) || tool == "read") {
            self.warning("TOOL_OUTPUT_UNAVAILABLE", Some(line));
        }
        if tool == "read" && data.success != Some(false) {
            match read_complete {
                Some(true) => {}
                Some(false) => self.warning("READ_RANGE_INCOMPLETE", Some(line)),
                None => self.warning("READ_COMPLETENESS_UNAVAILABLE", Some(line)),
            }
            if !read_scope_known {
                self.warning("READ_SCOPE_UNAVAILABLE", Some(line));
            }
        }
        let evidence = Evidence {
            event_ref: format!("e{line}.{block}"),
            record_line: line,
            call_record_line: call.as_ref().map(|c| c.line),
            source_event_sha256: digest(event_id),
            tool: known_tool(tool),
            arguments_sha256: arguments.clone(),
            output_sha256: data.output.as_ref().map(digest),
            success: data.success,
            exit_code: data.exit_code,
            read_range_complete: read_complete,
        };
        if tool != "read" {
            self.reads = Run::default();
        }
        if call.as_ref().is_some_and(|c| c.overlapping) || !self.pending.is_empty() {
            self.failures = Run::default();
            self.reads = Run::default();
            self.window.clear();
            self.last_loop = None;
            return Ok(());
        }
        let key = digest(
            serde_json::to_vec(&(
                digest(tool),
                arguments,
                &evidence.output_sha256,
                data.success,
                data.exit_code,
            ))
            .map_err(|_| Error::input())?,
        );
        let eligible_failure = data.success == Some(false) && data.output.is_some();
        let eligible_read = tool == "read"
            && data.success == Some(true)
            && read_complete == Some(true)
            && data.output.is_some()
            && read_scope_known;
        if eligible_failure {
            self.eligible[0] += 1;
            update_run(&mut self.failures, &key, &evidence);
            self.record_run(0);
        } else {
            self.failures = Run::default();
        }
        if eligible_read {
            self.eligible[2] += 1;
            self.report.coverage.complete_reads += 1;
            update_run(&mut self.reads, &key, &evidence);
            self.record_run(2);
        } else {
            self.reads = Run::default();
        }
        if eligible_failure || eligible_read {
            self.eligible[1] += 1;
            self.window.push_back(Operation { key, evidence });
            if self.window.len() > 64 {
                self.window.pop_front();
            }
            self.position += 1;
            self.detect_loop();
        } else {
            self.window.clear();
            self.last_loop = None;
        }
        Ok(())
    }
    fn record_run(&mut self, rule: usize) {
        let run = if rule == 0 {
            &self.failures
        } else {
            &self.reads
        };
        if self.settings.disabled_rules[rule] || run.count < self.settings.threshold(rule) {
            return;
        }
        if run.count == self.settings.threshold(rule) {
            let count = run.count;
            let evidence = run.evidence.clone();
            let index = self.add_finding(rule, count, evidence);
            let run = if rule == 0 {
                &mut self.failures
            } else {
                &mut self.reads
            };
            run.finding = index;
        } else if let Some(index) = run.finding {
            let finding = &mut self.report.findings[index];
            finding.occurrences = run.count;
            finding.evidence = run.evidence.clone();
            finding.evidence_omitted = run.count.saturating_sub(finding.evidence.len());
        }
    }
    fn detect_loop(&mut self) {
        if self.settings.disabled_rules[1] {
            return;
        }
        for width in 2..=4 {
            let length = width * self.settings.threshold(1);
            if self.window.len() < length {
                continue;
            }
            let start = self.window.len() - length;
            let pattern: Vec<_> = self
                .window
                .iter()
                .skip(start)
                .take(width)
                .map(|op| op.key.as_str())
                .collect();
            if pattern.iter().all(|k| *k == pattern[0]) {
                continue;
            }
            if !self
                .window
                .iter()
                .skip(start)
                .enumerate()
                .all(|(i, op)| op.key == pattern[i % width])
            {
                continue;
            }
            let key = (0..width)
                .map(|rotation| {
                    (0..width)
                        .map(|i| pattern[(i + rotation) % width])
                        .collect::<Vec<_>>()
                        .join(":")
                })
                .min()
                .unwrap_or_default();
            let previous = self
                .last_loop
                .as_ref()
                .filter(|(old, pos, _)| old == &key && *pos + 1 == self.position)
                .map(|(_, _, index)| *index);
            let index = if let Some(previous) = previous {
                if let Some(index) = previous {
                    let f = &mut self.report.findings[index];
                    f.occurrences += 1;
                    append_evidence(
                        f,
                        self.window
                            .back()
                            .expect("nonempty loop window")
                            .evidence
                            .clone(),
                    );
                }
                previous
            } else {
                let evidence = self
                    .window
                    .iter()
                    .skip(start)
                    .take(MAX_EVIDENCE)
                    .map(|op| op.evidence.clone())
                    .collect();
                self.add_finding(1, length, evidence)
            };
            self.last_loop = Some((key, self.position, index));
            return;
        }
        self.last_loop = None;
    }
    fn add_finding(
        &mut self,
        rule: usize,
        occurrences: usize,
        evidence: Vec<Evidence>,
    ) -> Option<usize> {
        if self.report.findings.len() >= MAX_FINDINGS {
            self.report.findings_omitted += 1;
            self.warning("FINDINGS_LIMIT_REACHED", None);
            return None;
        }
        let (description, recommendation) = match rule {
            0 => (
                "Repeated equivalent tool failures",
                "Inspect the original failure and retry policy before another equivalent attempt.",
            ),
            1 => (
                "Potential execution loop",
                "Inspect the repeated sequence and verify whether it produces useful progress.",
            ),
            _ => (
                "Potentially redundant file reads",
                "Check whether repeated verification is required before removing a read.",
            ),
        };
        let index = self.report.findings.len();
        self.report.findings.push(Finding { reference:String::new(), rule_id:RULES[rule].0, severity:if rule==2 {"low"} else {"medium"}, category:if rule==0 {"reliability"} else {"efficiency"}, description, classification:"heuristic", confidence:"matching_observations", methodology:RULES[rule].1, limitations:match rule {0=>"Only exact consecutive arguments, output and status match. Transient retries may be intentional. A failed command can still modify state.",1=>"Only bounded exact sequences of failures or complete reads are checked. Side effects, task intent and external progress are not established. Timestamp gaps are checked only when reported.",_=>"Only the recorded text range is compared. External file changes and deliberate verification remain possible."}, recommendation, occurrences, evidence_omitted:occurrences.saturating_sub(evidence.len()), evidence });
        Some(index)
    }
    pub fn finish(mut self, source_sha256: String, leaf: Option<&str>) -> Report {
        if !self.pending.is_empty() {
            self.report.coverage.unpaired_calls += self.pending.len();
            self.warning("TOOL_RESULTS_UNAVAILABLE", None);
        }
        if self.report.coverage.tool_results == 0 {
            self.warning("INSUFFICIENT_TELEMETRY", None);
        }
        self.report.source_sha256 = source_sha256;
        self.report.max_gap_seconds = self.settings.max_gap_seconds;
        self.report.leaf_sha256 = leaf.map(digest);
        for (i, (rule, limitations)) in RULES.iter().enumerate() {
            self.report.coverage.rules.push(RuleCoverage {
                rule_id: rule,
                suppressed: self.settings.disabled_rules[i],
                threshold: self.settings.threshold(i),
                status: if self.settings.disabled_rules[i] || self.eligible[i] == 0 {
                    "skipped"
                } else if self.report.complete {
                    "evaluated"
                } else {
                    "partial"
                },
                eligible_observations: self.eligible[i],
                limitations,
            });
        }
        self.report.report_id = digest(
            serde_json::to_vec(&(
                &self.report.source_sha256,
                &self.report.session_sha256,
                &self.report.leaf_sha256,
                [0, 1, 2].map(|i| self.settings.threshold(i)),
                self.settings.max_gap_seconds,
                self.settings.disabled_rules,
                VERSION,
            ))
            .unwrap_or_default(),
        )[..24]
            .to_owned();
        for (i, finding) in self.report.findings.iter_mut().enumerate() {
            finding.reference = format!("@{}:f{:03}", self.report.report_id, i + 1);
        }
        self.report
    }
}
fn argument_hash(arguments: Option<&Value>) -> Result<Option<String>> {
    match arguments {
        Some(Value::Object(_)) => Ok(Some(digest(
            serde_json::to_vec(&arguments).map_err(|_| Error::input())?,
        ))),
        Some(_) => Err(Error::input()),
        None => Ok(None),
    }
}
fn update_run(run: &mut Run, key: &str, evidence: &Evidence) {
    if run.key != key {
        *run = Run {
            key: key.to_owned(),
            ..Run::default()
        };
    }
    run.count += 1;
    if run.evidence.len() < MAX_EVIDENCE {
        run.evidence.push(evidence.clone());
    }
}
fn append_evidence(finding: &mut Finding, evidence: Evidence) {
    if finding.evidence.len() < MAX_EVIDENCE {
        finding.evidence.push(evidence);
    } else {
        finding.evidence_omitted += 1;
    }
}
