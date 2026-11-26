use crate::{
    MAX_EVIDENCE, MAX_FINDINGS, MAX_INPUT_BYTES, MAX_PENDING, MAX_RECORD_BYTES, MAX_RECORDS,
    MAX_WARNINGS, SCHEMA_VERSION, VERSION,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub event_ref: String,
    pub record_line: usize,
    pub call_record_line: Option<usize>,
    pub source_event_sha256: String,
    pub tool: &'static str,
    pub arguments_sha256: Option<String>,
    pub output_sha256: Option<String>,
    pub success: Option<bool>,
    pub exit_code: Option<i32>,
    pub read_range_complete: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct Finding {
    pub reference: String,
    pub rule_id: &'static str,
    pub severity: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub classification: &'static str,
    pub confidence: &'static str,
    pub methodology: &'static str,
    pub limitations: &'static str,
    pub recommendation: &'static str,
    pub occurrences: usize,
    pub evidence: Vec<Evidence>,
    pub evidence_omitted: usize,
}
#[derive(Debug, Serialize)]
pub struct Warning {
    pub code: &'static str,
    pub record_line: Option<usize>,
}
#[derive(Debug, Serialize)]
pub struct RuleCoverage {
    pub rule_id: &'static str,
    pub status: &'static str,
    pub eligible_observations: usize,
    pub limitations: &'static str,
}
#[derive(Debug, Default, Serialize)]
pub struct Coverage {
    pub records: usize,
    pub selected_records: usize,
    pub malformed_records: usize,
    pub unknown_records: usize,
    pub unpaired_calls: usize,
    pub tool_calls: usize,
    pub tool_results: usize,
    pub known_success: usize,
    pub complete_reads: usize,
    pub rules: Vec<RuleCoverage>,
}
#[derive(Debug, Serialize)]
pub struct Limits {
    pub input_bytes: u64,
    pub record_bytes: usize,
    pub records: usize,
    pub pending_calls: usize,
    pub normalized_events: usize,
    pub findings: usize,
    pub evidence_per_finding: usize,
    pub warnings: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            input_bytes: MAX_INPUT_BYTES,
            record_bytes: MAX_RECORD_BYTES,
            records: MAX_RECORDS,
            pending_calls: MAX_PENDING,
            normalized_events: MAX_RECORDS,
            findings: MAX_FINDINGS,
            evidence_per_finding: MAX_EVIDENCE,
            warnings: MAX_WARNINGS,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: &'static str,
    pub traceveil_version: &'static str,
    pub report_id: String,
    pub session_sha256: String,
    pub source_sha256: String,
    pub adapter: &'static str,
    pub leaf_sha256: Option<String>,
    /// Supported parsing and required rule telemetry are complete, not proof of task success.
    pub complete: bool,
    pub coverage: Coverage,
    pub findings: Vec<Finding>,
    pub findings_omitted: usize,
    pub warnings: Vec<Warning>,
    pub warnings_omitted: usize,
    pub limits: Limits,
}
impl Report {
    pub fn new(adapter: &'static str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            traceveil_version: VERSION,
            report_id: String::new(),
            session_sha256: String::new(),
            source_sha256: String::new(),
            adapter,
            leaf_sha256: None,
            complete: true,
            coverage: Coverage::default(),
            findings: vec![],
            findings_omitted: 0,
            warnings: vec![],
            warnings_omitted: 0,
            limits: Limits::default(),
        }
    }
    pub fn warn(&mut self, code: &'static str, line: Option<usize>) {
        self.complete = false;
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(Warning {
                code,
                record_line: line,
            });
        } else {
            self.warnings_omitted += 1;
        }
    }
}
