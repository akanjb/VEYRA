//! Turns the lines of one rollout file into usage records, in file order.
//!
//! Two sources of usage, never mixed within a file:
//! - `token_usage_record` lines (newer versions): the exact usage of each model response.
//! - `token_count` running totals (older versions): usage is the increase between
//!   consecutive totals. A total that goes down (Codex overwrote it after compacting or
//!   exceeding the context window) starts a new baseline and adds nothing.
//!
//! Forked threads copy their parent's lines first. Copied usage records carry the
//! parent's thread id and are skipped; for older files, totals before the fork's first
//! own turn are its baseline.

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::source::RawUsage;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::{ToolCall, ToolOutcome};

use super::record::{CallResult, CompletedItem, Event, ResponseItem, SessionMeta, TokenUsage};

/// Response item types that are tool calls, with the name used when the item has none.
const TOOL_CALLS: [(&str, &str); 4] = [
    ("function_call", ""),
    ("custom_tool_call", ""),
    ("local_shell_call", "local_shell"),
    ("web_search_call", "web_search"),
];

/// Accumulates one file's usage and tool outcomes.
#[derive(Debug, Default)]
pub struct Rollout {
    meta: SessionMeta,
    model: Option<String>,
    cwd: Option<String>,
    /// Exact per-response records, with the tool calls made since the previous one.
    records: Stream,
    /// Increases of the running totals, with the tool calls made since the previous one.
    increases: Stream,
    last_total: Option<TokenUsage>,
    /// False in a forked file until its first own turn starts.
    counting: bool,
    pub outcomes: Vec<ToolOutcome>,
}

#[derive(Debug, Default)]
struct Stream {
    usage: Vec<RawUsage>,
    pending_calls: Vec<ToolCall>,
}

impl Stream {
    fn push(&mut self, mut usage: RawUsage) {
        usage.tool_calls = std::mem::take(&mut self.pending_calls);
        self.usage.push(usage);
    }
}

impl Rollout {
    pub fn session_meta(&mut self, meta: SessionMeta) {
        self.counting = meta.forked_from_id.is_none();
        self.cwd.clone_from(&meta.cwd);
        self.meta = meta;
    }

    pub fn turn_context(&mut self, model: Option<String>, cwd: Option<String>) {
        if model.is_some() {
            self.model = model;
        }
        if cwd.is_some() {
            self.cwd = cwd;
        }
    }

    pub fn response_item(&mut self, item: &ResponseItem) {
        let Some((_, fallback)) = TOOL_CALLS.iter().find(|(kind, _)| *kind == item.kind) else {
            return;
        };
        let (Some(id), Some(name)) = (
            item.call_id.clone(),
            item.name
                .clone()
                .or_else(|| (!fallback.is_empty()).then(|| (*fallback).to_owned())),
        ) else {
            return;
        };
        let call = ToolCall { id, name };
        self.records.pending_calls.push(call.clone());
        self.increases.pending_calls.push(call);
    }

    /// An exact per-response record. Records copied from a parent thread are skipped.
    pub fn usage_record(
        &mut self,
        at: UtcMillis,
        thread: Option<&str>,
        response: Option<&str>,
        usage: TokenUsage,
    ) {
        if thread.is_some_and(|t| t != self.meta.id) {
            return;
        }
        let Some(response) = response else { return };
        let raw = self.raw(
            format!("codex:{response}"),
            at,
            usage,
            Some(1),
            Accuracy::Exact,
        );
        self.records.push(raw);
    }

    pub fn event(&mut self, at: UtcMillis, event: Event) {
        match event {
            Event::TokenCount { info } => {
                if let Some(total) = info.and_then(|i| i.total_token_usage) {
                    self.running_total(at, total);
                }
            }
            Event::TaskStarted {} | Event::TurnStarted {} => self.counting = true,
            Event::ItemCompleted { item } => self.outcomes.extend(item_outcome(item)),
            Event::McpToolCallEnd { call_id, result } => {
                if let Some(result) = result {
                    self.outcomes.push(ToolOutcome {
                        tool_call_id: call_id,
                        is_error: matches!(result, CallResult::Err(_)),
                    });
                }
            }
            Event::PatchApplyEnd { call_id, success } => {
                if let Some(success) = success {
                    self.outcomes.push(ToolOutcome {
                        tool_call_id: call_id,
                        is_error: !success,
                    });
                }
            }
            Event::Other => {}
        }
    }

    /// Usage of the whole file: exact records when the file has any, otherwise the
    /// increases of the running totals.
    pub fn into_usage(self) -> Vec<RawUsage> {
        if self.records.usage.is_empty() {
            self.increases.usage
        } else {
            self.records.usage
        }
    }

    fn running_total(&mut self, at: UtcMillis, total: TokenUsage) {
        let previous = self.last_total.replace(total);
        let increase = match previous {
            None => Some(total),
            Some(previous) if previous == total => None,
            Some(previous) => difference(&total, &previous),
        };
        let Some(increase) = increase.filter(|_| self.counting) else {
            return;
        };
        // The running total identifies the increase, whichever copy of the file
        // (live, archived or compressed) it is read from.
        let key = format!(
            "codex:{}:{}-{}-{}",
            self.meta.id, total.input_tokens, total.output_tokens, total.total_tokens
        );
        // One increase may span several responses (retries), so requests are partial.
        let raw = self.raw(key, at, increase, Some(1), Accuracy::Partial);
        self.increases.push(raw);
    }

    fn raw(
        &self,
        natural_key: String,
        occurred_at: UtcMillis,
        usage: TokenUsage,
        requests: Option<u32>,
        request_accuracy: Accuracy,
    ) -> RawUsage {
        let is_subagent = self.meta.parent_thread_id.is_some()
            || self
                .meta
                .source
                .as_ref()
                .is_some_and(|s| s.get("subagent").is_some());
        RawUsage {
            natural_key,
            precedence: 0,
            occurred_at,
            model: self.model.clone(),
            provider: self.meta.model_provider.clone(),
            gateway: None,
            tokens: canonical(&usage),
            requests,
            reported_cost: None,
            interface: interface(&self.meta),
            service_tier: ServiceTier::Standard,
            inference_geo_us: false,
            granularity: Granularity::Request,
            session_key: Some(format!("codex:{}", self.meta.id)),
            parent_session_key: self
                .meta
                .parent_thread_id
                .as_ref()
                .map(|p| format!("codex:{p}")),
            is_subagent,
            cwd: self.cwd.clone(),
            tool_version: self.meta.cli_version.clone(),
            accuracy: AccuracyMap {
                tokens: Accuracy::Exact,
                cache: Accuracy::Exact,
                reasoning: Accuracy::Exact,
                requests: request_accuracy,
                // Cost is always derived from tokens and published prices.
                cost: Accuracy::Estimated,
            },
            tool_calls: Vec::new(),
        }
    }
}

/// `total - previous`, field by field; `None` when any field decreased (a reset).
fn difference(total: &TokenUsage, previous: &TokenUsage) -> Option<TokenUsage> {
    let fields = [
        (total.input_tokens, previous.input_tokens),
        (total.cached_input_tokens, previous.cached_input_tokens),
        (
            total.cache_write_input_tokens,
            previous.cache_write_input_tokens,
        ),
        (total.output_tokens, previous.output_tokens),
        (
            total.reasoning_output_tokens,
            previous.reasoning_output_tokens,
        ),
        (total.total_tokens, previous.total_tokens),
    ];
    if fields.iter().any(|(now, before)| now < before) {
        return None;
    }
    Some(TokenUsage {
        input_tokens: total.input_tokens - previous.input_tokens,
        cached_input_tokens: total.cached_input_tokens - previous.cached_input_tokens,
        cache_write_input_tokens: total.cache_write_input_tokens
            - previous.cache_write_input_tokens,
        output_tokens: total.output_tokens - previous.output_tokens,
        reasoning_output_tokens: total.reasoning_output_tokens - previous.reasoning_output_tokens,
        total_tokens: total.total_tokens - previous.total_tokens,
    })
}

/// Canonical tokens: cache reads and writes are separated out of the input count, and
/// reasoning stays a subset of output.
fn canonical(usage: &TokenUsage) -> TokenCounts {
    let count = |n: i64| u64::try_from(n.max(0)).ok();
    let input = usage.input_tokens.max(0);
    let cached = usage.cached_input_tokens.clamp(0, input);
    let written = usage.cache_write_input_tokens.clamp(0, input - cached);
    TokenCounts {
        input_uncached: count(input - cached - written),
        cache_read: count(cached),
        cache_write_unspecified: count(written),
        output_total: count(usage.output_tokens),
        reasoning: count(usage.reasoning_output_tokens.min(usage.output_tokens)),
        ..TokenCounts::default()
    }
}

/// The interface, from the client that wrote the file.
fn interface(meta: &SessionMeta) -> Interface {
    let originator = meta
        .originator
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let source = meta.source.as_ref().and_then(|s| s.as_str()).unwrap_or("");
    if originator.contains("desktop") {
        Interface::Desktop
    } else if originator.contains("vscode") || source == "vscode" {
        Interface::Ide
    } else {
        Interface::Cli
    }
}

/// The outcome of a finished command or MCP tool call.
fn item_outcome(item: CompletedItem) -> Option<ToolOutcome> {
    if item.kind != "CommandExecution" && item.kind != "McpToolCall" {
        return None;
    }
    let is_error = match item.status.as_deref()? {
        "completed" => false,
        "failed" | "declined" => true,
        _ => return None,
    };
    Some(ToolOutcome {
        tool_call_id: item.id?,
        is_error,
    })
}
