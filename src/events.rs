use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Linear canonical JSONL. Unknown event types become explicit coverage gaps.
#[derive(Debug, Deserialize, Serialize)]
pub struct Event {
    pub schema_version: String,
    pub event_id: String,
    pub session_id: String,
    #[serde(default)]
    pub parent_event_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    pub event_type: String,
    #[serde(default)]
    pub data: Data,
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Data {
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub call_id: Option<String>,
    #[serde(default)]
    pub arguments: Option<Value>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub output: Option<String>,
    /// True means the supplied text covers the requested read range without truncation.
    #[serde(default)]
    pub output_complete: Option<bool>,
}
#[derive(Debug)]
pub struct SourcedEvent {
    pub event: Event,
    pub line: usize,
    pub block: usize,
}

pub fn known_tool(name: &str) -> &'static str {
    match name {
        "bash" => "bash",
        "read" => "read",
        "write" => "write",
        "edit" => "edit",
        "test" => "test",
        "search" => "search",
        "fetch" => "fetch",
        _ => "other",
    }
}
