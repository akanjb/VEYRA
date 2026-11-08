//! Session reads: the explorer list and the per-session timeline (FR-AN-07).
//!
//! Only metadata and numbers cross this port: identifiers, models, token counts, costs,
//! tool *names* and outcomes. Never prompt, response or tool content.

use veyra_domain::accuracy::Accuracy;
use veyra_domain::money::NanoUsd;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;

use crate::dimension::Filters;
use crate::port::ReadError;
use crate::sums::BaseSums;

/// How the session list is ordered (always descending).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSort {
    LastActive,
    Tokens,
    Cost,
    Events,
}

/// Sessions with at least one matching event in `[from, until)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionQuery {
    pub from: UtcMillis,
    pub until: UtcMillis,
    pub filters: Filters,
    pub sort: SessionSort,
    pub limit: u32,
    pub offset: u32,
}

/// One session and its totals over the events considered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: i64,
    /// Source-scoped key, e.g. `claude-code:<session-id>`.
    pub key: String,
    pub parent_id: Option<i64>,
    pub is_subagent: bool,
    pub app: String,
    /// Normalised project path.
    pub project: Option<String>,
    /// Distinct models used (canonical id, or the raw string when unrecognised).
    pub models: Vec<String>,
    pub first_at: UtcMillis,
    pub last_at: UtcMillis,
    pub sums: BaseSums,
}

/// A page of sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPage {
    /// Matching sessions in total, across all pages.
    pub total: u64,
    pub sessions: Vec<SessionSummary>,
}

/// A tool invocation within a request (name only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolUse {
    pub name: String,
    pub mcp_server: Option<String>,
    /// `None` when the outcome is unknown.
    pub is_error: Option<bool>,
}

/// One request in a session's timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEvent {
    pub at: UtcMillis,
    pub model: Option<String>,
    pub raw_model: Option<String>,
    pub tokens: TokenCounts,
    pub token_accuracy: Accuracy,
    /// API-equivalent estimate; `None` when the event could not be priced.
    pub cost: Option<NanoUsd>,
    pub tools: Vec<ToolUse>,
}

/// A session with its whole timeline and its subagents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub events: Vec<SessionEvent>,
    pub subagents: Vec<SessionSummary>,
}

/// Session reads (implemented by the data layer).
pub trait SessionReader {
    /// A page of sessions matching the query.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn sessions(&self, query: &SessionQuery) -> Result<SessionPage, ReadError>;

    /// One session over its whole lifetime; `None` when it does not exist.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn session(&self, id: i64) -> Result<Option<SessionDetail>, ReadError>;
}
