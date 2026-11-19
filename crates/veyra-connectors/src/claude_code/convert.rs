//! Converts one Claude Code log line into canonical records.
//!
//! Rules (from the M2 validation spike, see `docs/connectors/claude-code.md`):
//! - Streaming writes the same request on several lines; the line with a `stop_reason`
//!   and the most tokens is the complete one ([`STOP_BIT`] precedence).
//! - `thinking_tokens` is part of `output_tokens`; a line reporting more thinking than
//!   output is inconsistent, so its reasoning is left unreported (never clamped).
//! - Cache writes are split into 5-minute and 1-hour TTLs; when the split does not add
//!   up to the total, the total is kept as unspecified TTL.
//! - `<synthetic>` lines are client-generated placeholders, not API usage.

use std::path::Path;

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::source::RawUsage;
use veyra_domain::telemetry::TelemetryRecord;
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::{ToolCall, ToolOutcome};

use super::record::{Block, CacheCreation, Content, Line, Usage};
use crate::shared::{plain_components, utc_millis};

const SYNTHETIC_MODEL: &str = "<synthetic>";

/// Precedence bit for observations that completed (carry a `stop_reason`); the lower
/// bits hold the observation's total tokens.
pub const STOP_BIT: i64 = 1 << 52;

/// Whether a file is a main session log or a subagent log, judged from its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Main,
    Subagent { agent: String },
}

impl FileKind {
    pub fn of(relative: &Path) -> Self {
        match plain_components(relative).as_deref() {
            Some([_, _, "subagents", file]) => Self::Subagent {
                agent: file.trim_end_matches(".jsonl").to_owned(),
            },
            _ => Self::Main,
        }
    }
}

fn blocks(line: &Line) -> &[Block] {
    match line.message.as_ref().map(|m| &m.content) {
        Some(Content::Blocks(blocks)) => blocks,
        _ => &[],
    }
}

/// Outcomes of earlier tool calls reported on this line.
pub fn tool_outcomes(line: &Line) -> impl Iterator<Item = ToolOutcome> + '_ {
    blocks(line).iter().filter_map(|block| {
        (block.kind.as_deref() == Some("tool_result"))
            .then(|| block.tool_use_id.clone())
            .flatten()
            .map(|tool_call_id| ToolOutcome {
                tool_call_id,
                is_error: block.is_error.unwrap_or(false),
            })
    })
}

/// The usage record on this line, `None` when the line carries no API usage, or the
/// kind of problem that made it unusable.
pub fn usage(line: &Line, file: &FileKind) -> Result<Option<RawUsage>, &'static str> {
    let Some(message) = &line.message else {
        return Ok(None);
    };
    let Some(usage) = &message.usage else {
        return Ok(None);
    };
    if message.model.as_deref() == Some(SYNTHETIC_MODEL) {
        return Ok(None);
    }
    let request = line
        .request_id
        .as_deref()
        .or(message.id.as_deref())
        .ok_or("missing-request-id")?;
    let occurred_at = line
        .timestamp
        .as_deref()
        .and_then(utc_millis)
        .ok_or("invalid-timestamp")?;

    let (tokens, reasoning_consistent) = canonical_tokens(usage);
    let completed = message.stop_reason.is_some();
    let is_subagent = matches!(file, FileKind::Subagent { .. }) || line.is_sidechain == Some(true);
    let agent = line.agent_id.clone().or_else(|| match file {
        FileKind::Subagent { agent } => Some(agent.clone()),
        FileKind::Main => None,
    });
    let (session_key, parent_session_key) =
        session_keys(line.session_id.as_deref(), is_subagent, agent.as_deref());

    Ok(Some(RawUsage {
        natural_key: format!("anthropic:{request}"),
        precedence: precedence(completed, &tokens),
        occurred_at,
        model: message.model.clone(),
        provider: Some("anthropic".to_owned()),
        gateway: None,
        tokens,
        requests: Some(1),
        reported_cost: None,
        interface: interface(line.entrypoint.as_deref()),
        service_tier: service_tier(usage),
        inference_geo_us: usage.inference_geo.as_deref() == Some("us"),
        granularity: Granularity::Request,
        session_key,
        parent_session_key,
        is_subagent,
        cwd: line.cwd.clone(),
        tool_version: line.version.clone(),
        accuracy: accuracy(reasoning_consistent),
        tool_calls: blocks(line)
            .iter()
            .filter(|b| b.kind.as_deref() == Some("tool_use"))
            .filter_map(|b| {
                Some(ToolCall {
                    id: b.id.clone()?,
                    name: b.name.clone()?,
                })
            })
            .collect(),
    }))
}

/// The usage in Claude Code's documented `api_request` OpenTelemetry event (ADR-038).
///
/// - The request id gives the same natural key as the log line of the same request, so
///   enabling both sources never counts a request twice.
/// - The event has no `stop_reason`, so its precedence is that of an incomplete line:
///   the completed log line, which also knows the project, always supersedes it.
/// - `cost_usd` is Claude Code's own estimate, not a charge, so it is not kept.
pub fn telemetry_usage(record: &TelemetryRecord) -> Option<RawUsage> {
    if record.text("event.name") != Some("api_request") {
        return None;
    }
    let request = record.text("request_id")?;
    let occurred_at = record
        .text("event.timestamp")
        .and_then(utc_millis)
        .or(record.time)?;
    let usage = Usage {
        input_tokens: record.count("input_tokens"),
        output_tokens: record.count("output_tokens"),
        cache_creation_input_tokens: record.count("cache_creation_tokens"),
        cache_read_input_tokens: record.count("cache_read_tokens"),
        cache_creation: match (
            record.count("cache_creation_5m_tokens"),
            record.count("cache_creation_1h_tokens"),
        ) {
            (None, None) => None,
            (short, long) => Some(CacheCreation {
                ephemeral_5m_input_tokens: short,
                ephemeral_1h_input_tokens: long,
            }),
        },
        output_tokens_details: None,
        service_tier: None,
        inference_geo: None,
        speed: record.text("speed").map(str::to_owned),
    };
    let (tokens, reasoning_consistent) = canonical_tokens(&usage);
    let agent = record.text("agent_id");
    let is_subagent = agent.is_some();
    let (session_key, parent_session_key) =
        session_keys(record.text("session.id"), is_subagent, agent);
    Some(RawUsage {
        natural_key: format!("anthropic:{request}"),
        precedence: precedence(false, &tokens),
        occurred_at,
        model: record.text("model").map(str::to_owned),
        provider: Some("anthropic".to_owned()),
        gateway: None,
        tokens,
        requests: Some(1),
        reported_cost: None,
        interface: interface(record.text("app.entrypoint")),
        service_tier: service_tier(&usage),
        inference_geo_us: false,
        granularity: Granularity::Request,
        session_key,
        parent_session_key,
        is_subagent,
        cwd: None,
        tool_version: record.service_version.clone(),
        accuracy: accuracy(reasoning_consistent),
        tool_calls: Vec::new(),
    })
}

/// Session keys: a subagent with a known id is its own session under the main one.
fn session_keys(
    session: Option<&str>,
    is_subagent: bool,
    agent: Option<&str>,
) -> (Option<String>, Option<String>) {
    match (session, is_subagent, agent) {
        (Some(s), true, Some(agent)) => (
            Some(format!("claude-code:{s}:{agent}")),
            Some(format!("claude-code:{s}")),
        ),
        (session, _, _) => (session.map(|s| format!("claude-code:{s}")), None),
    }
}

/// Token counts come straight from the API; cost is always derived from published
/// prices.
const fn accuracy(reasoning_consistent: bool) -> AccuracyMap {
    AccuracyMap {
        tokens: Accuracy::Exact,
        cache: Accuracy::Exact,
        reasoning: if reasoning_consistent {
            Accuracy::Exact
        } else {
            Accuracy::Partial
        },
        requests: Accuracy::Exact,
        cost: Accuracy::Estimated,
    }
}

/// Canonical tokens, and whether the reported reasoning was consistent with output.
fn canonical_tokens(usage: &Usage) -> (TokenCounts, bool) {
    let output = usage.output_tokens;
    let thinking = usage
        .output_tokens_details
        .as_ref()
        .and_then(|d| d.thinking_tokens);
    let reasoning = match (thinking, output) {
        (Some(t), Some(o)) if t <= o => Some(t),
        _ => None,
    };
    let split = usage.cache_creation.as_ref().map(|c| {
        (
            c.ephemeral_5m_input_tokens.unwrap_or(0),
            c.ephemeral_1h_input_tokens.unwrap_or(0),
        )
    });
    let (short, long, unspecified) = match (usage.cache_creation_input_tokens, split) {
        (Some(total), Some((s, l))) if s + l == total => (Some(s), Some(l), None),
        (Some(total), _) => (None, None, Some(total)),
        (None, Some((s, l))) => (Some(s), Some(l), None),
        (None, None) => (None, None, None),
    };
    let tokens = TokenCounts {
        input_uncached: usage.input_tokens,
        cache_read: usage.cache_read_input_tokens,
        cache_write_short: short,
        cache_write_long: long,
        cache_write_unspecified: unspecified,
        output_total: output,
        reasoning,
        tool_tokens: None,
    };
    (tokens, thinking.is_none() || reasoning.is_some())
}

fn precedence(completed: bool, tokens: &TokenCounts) -> i64 {
    let total = i64::try_from(tokens.total().unwrap_or(0)).unwrap_or(i64::MAX);
    (if completed { STOP_BIT } else { 0 }) | total.min(STOP_BIT - 1)
}

/// The interface, from Claude Code's `entrypoint`. Unknown values count as CLI.
fn interface(entrypoint: Option<&str>) -> Interface {
    match entrypoint {
        Some("claude-desktop") => Interface::Desktop,
        Some(e) if e.contains("vscode") || e.contains("jetbrains") || e.contains("ide") => {
            Interface::Ide
        }
        Some(e) if e.starts_with("sdk") => Interface::Api,
        _ => Interface::Cli,
    }
}

fn service_tier(usage: &Usage) -> ServiceTier {
    if usage.speed.as_deref() == Some("fast") {
        return ServiceTier::Fast;
    }
    match usage.service_tier.as_deref() {
        Some("batch") => ServiceTier::Batch,
        Some("priority") => ServiceTier::Priority,
        Some("flex") => ServiceTier::Flex,
        _ => ServiceTier::Standard,
    }
}
