//! The parts of a Copilot CLI session event Veyra reads. Content fields (messages, tool
//! arguments, results, code changes) are left out, so serde skips them unread.

use std::collections::BTreeMap;

use serde::Deserialize;

/// One event: `{id, timestamp, type, data}`.
#[derive(Debug, Deserialize)]
pub struct Event {
    pub id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: Data,
}

/// The `data` fields of the event types Veyra reads.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Data {
    // session.start and session.resume
    pub session_id: Option<String>,
    pub copilot_version: Option<String>,
    pub context: Option<Context>,
    // session.context_changed
    pub pending_git_context: Option<bool>,
    // tool.execution_start and tool.execution_complete
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub success: Option<bool>,
    // session.shutdown
    pub model_metrics: Option<BTreeMap<String, ModelMetrics>>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Context {
    pub cwd: Option<String>,
}

/// One model's totals in a shutdown summary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ModelMetrics {
    pub requests: Requests,
    pub usage: Usage,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Requests {
    pub count: u64,
}

/// Copilot's token totals; `inputTokens` includes the cached tokens.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[expect(
    clippy::struct_field_names,
    reason = "the names Copilot serialises, kept so the record reads like the file"
)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
}
