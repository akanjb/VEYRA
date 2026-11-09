//! The parts of a Codex rollout line Veyra reads. Every content field (instructions,
//! messages, tool arguments and output) is left out, so serde skips it unread.

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::value::RawValue;

/// One rollout line: `{timestamp, type, payload}`.
#[derive(Debug, Deserialize)]
pub struct Line<'a> {
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(borrow)]
    pub payload: Option<&'a RawValue>,
}

/// `session_meta`: the thread this file records.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub forked_from_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub cwd: Option<String>,
    pub originator: Option<String>,
    pub cli_version: Option<String>,
    /// `"cli"`, `"vscode"`, `"exec"`, or an object such as `{"subagent": …}`.
    pub source: Option<serde_json::Value>,
    pub model_provider: Option<String>,
}

/// `turn_context`: the model and directory of the turns that follow.
#[derive(Debug, Deserialize)]
pub struct TurnContext {
    pub cwd: Option<String>,
    pub model: Option<String>,
}

/// Token counts as Codex records them: `cached_input_tokens` and
/// `cache_write_input_tokens` are part of `input_tokens`, `reasoning_output_tokens` is
/// part of `output_tokens`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_field_names,
    reason = "the names Codex serialises, kept so the record reads like the file"
)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_write_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

/// `token_usage_record`: the exact usage of one model response (newer versions).
#[derive(Debug, Deserialize)]
pub struct UsageRecord {
    pub thread_id: Option<String>,
    pub response_id: Option<String>,
    pub usage: TokenUsage,
}

/// `event_msg`, by its inner `type`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Running thread totals (older versions' only usage record).
    TokenCount {
        info: Option<TokenInfo>,
    },
    TaskStarted {},
    TurnStarted {},
    ItemCompleted {
        item: CompletedItem,
    },
    McpToolCallEnd {
        call_id: String,
        result: Option<CallResult>,
    },
    PatchApplyEnd {
        call_id: String,
        success: Option<bool>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct TokenInfo {
    pub total_token_usage: Option<TokenUsage>,
}

/// A finished tool item (newer versions): command executions and MCP tool calls.
#[derive(Debug, Deserialize)]
pub struct CompletedItem {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Option<String>,
    pub status: Option<String>,
}

/// Whether an MCP call succeeded; its content is skipped unread.
#[derive(Debug, Deserialize)]
pub enum CallResult {
    Ok(IgnoredAny),
    Err(IgnoredAny),
}

/// `response_item`: only tool calls are read, and only their names and ids.
#[derive(Debug, Deserialize)]
pub struct ResponseItem {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: Option<String>,
    pub call_id: Option<String>,
}
