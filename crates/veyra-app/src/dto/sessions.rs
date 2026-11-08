use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{AccuracyClass, Measures, Scope};

/// How the session explorer is ordered (always descending).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SessionSort {
    LastActive,
    Tokens,
    Cost,
    Events,
}

/// Request for [`crate::Api::list_sessions`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionsRequest {
    pub scope: Scope,
    pub sort: SessionSort,
    /// Zero-based page number.
    pub page: u32,
    /// Rows per page (1–500).
    pub page_size: u32,
}

/// A page of the session explorer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    /// Matching sessions across all pages.
    #[ts(type = "number")]
    pub total: u64,
    pub rows: Vec<SessionRow>,
}

/// One session. In the explorer, numbers cover the scope's period; in a session detail,
/// the whole session. Times are UTC epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    #[ts(type = "number")]
    pub id: i64,
    /// Source-scoped key, e.g. `claude-code:<session-id>`.
    pub key: String,
    pub app: String,
    /// Normalised project path.
    pub project: Option<String>,
    pub models: Vec<String>,
    pub is_subagent: bool,
    #[ts(type = "number | null")]
    pub parent_id: Option<i64>,
    #[ts(type = "number")]
    pub started_at: i64,
    #[ts(type = "number")]
    pub ended_at: i64,
    /// Derived active time (gaps capped at the idle threshold).
    #[ts(type = "number")]
    pub active_ms: i64,
    #[ts(type = "number")]
    pub events: u64,
    pub measures: Measures,
    /// Requests with no price at all (unrecognised model); their cost is missing.
    #[ts(type = "number")]
    pub unpriced_requests: u64,
    /// Whether some requests were priced with an assumption (e.g. an unknown cache
    /// lifetime) or lack a rate for one component, so the cost is approximate.
    pub cost_assumptions: bool,
    pub accuracy: Option<AccuracyClass>,
}

/// Request for [`crate::Api::get_session`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionRequest {
    #[ts(type = "number")]
    pub id: i64,
}

/// Token counts of one request; `null` when the source did not report a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TokenBreakdown {
    #[ts(type = "number | null")]
    pub input: Option<u64>,
    #[ts(type = "number | null")]
    pub cache_read: Option<u64>,
    #[ts(type = "number | null")]
    pub cache_write: Option<u64>,
    #[ts(type = "number | null")]
    pub output: Option<u64>,
    /// Part of `output`, not added to the total.
    #[ts(type = "number | null")]
    pub reasoning: Option<u64>,
    #[ts(type = "number")]
    pub total: u64,
}

/// A tool invocation (name only, never arguments or output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallSummary {
    pub name: String,
    pub mcp_server: Option<String>,
    /// `null` when the outcome is unknown.
    pub is_error: Option<bool>,
}

/// One request on a session's timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    #[ts(type = "number")]
    pub at: i64,
    /// Canonical model id, or the reported string when unrecognised.
    pub model: Option<String>,
    pub tokens: TokenBreakdown,
    /// API-equivalent estimate; `null` when the request could not be priced.
    #[ts(type = "number | null")]
    pub cost_nano_usd: Option<i64>,
    pub accuracy: AccuracyClass,
    pub tools: Vec<ToolCallSummary>,
}

/// A session with its timeline and subagents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionRow,
    pub events: Vec<TimelineEvent>,
    pub subagents: Vec<SessionRow>,
}
