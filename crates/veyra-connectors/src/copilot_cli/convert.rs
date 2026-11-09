//! Turns one session's events into usage records.
//!
//! Copilot CLI writes no per-request usage. When a session ends it writes a summary of
//! tokens and requests per model (`session.shutdown`), so each record covers one run of
//! the session for one model. How summaries add up changed in version 1.0.66:
//! - before 1.0.66, each summary covers only its own run, so every summary counts;
//! - from 1.0.66, each summary covers the whole session so far, so a run is the
//!   difference from the previous summary. A summary whose totals went down started
//!   over (a session resumed by an older version), and counts in full.
//!
//! A run that crashed wrote no summary, and its usage is not recorded.

use std::collections::BTreeMap;

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::source::RawUsage;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::{ToolCall, ToolOutcome};

use super::record::{Data, Event, ModelMetrics, Usage};
use crate::shared::utc_millis;

/// The version from which shutdown summaries are cumulative.
const CUMULATIVE_FROM: (u64, u64, u64) = (1, 0, 66);

/// The service between the user and the model vendors.
const GATEWAY: &str = "github-copilot";

type Summary = BTreeMap<String, ModelMetrics>;

/// Accumulates one session file.
#[derive(Debug, Default)]
pub struct Session {
    id: Option<String>,
    /// The Copilot version that started the session, as written and as numbers.
    version_text: Option<String>,
    version: Option<(u64, u64, u64)>,
    cwd: Option<String>,
    /// The previous summary, for cumulative versions.
    previous: Summary,
    /// Tool calls since the previous summary.
    pending_calls: Vec<ToolCall>,
    pub usage: Vec<RawUsage>,
    pub outcomes: Vec<ToolOutcome>,
}

impl Session {
    pub fn new(fallback_id: Option<String>) -> Self {
        Self {
            id: fallback_id,
            ..Self::default()
        }
    }

    pub fn event(&mut self, event: Event) {
        let Event {
            id,
            timestamp,
            kind,
            data,
        } = event;
        match kind.as_str() {
            "session.start" => {
                self.id = data.session_id.clone().or(self.id.take());
                self.version = data.copilot_version.as_deref().map(version);
                self.version_text.clone_from(&data.copilot_version);
                self.context(&data);
            }
            "session.resume" => self.context(&data),
            "session.context_changed" if data.pending_git_context != Some(true) => {
                self.context(&data);
            }
            "tool.execution_start" => {
                if let (Some(id), Some(name)) = (data.tool_call_id, data.tool_name) {
                    self.pending_calls.push(ToolCall { id, name });
                }
            }
            "tool.execution_complete" => {
                if let (Some(id), Some(success)) = (data.tool_call_id, data.success) {
                    self.outcomes.push(ToolOutcome {
                        tool_call_id: id,
                        is_error: !success,
                    });
                }
            }
            "session.shutdown" => {
                let at = timestamp.as_deref().and_then(utc_millis);
                if let (Some(event_id), Some(at), Some(summary)) = (id, at, data.model_metrics) {
                    self.shutdown(&event_id, at, summary);
                }
            }
            _ => {}
        }
    }

    fn context(&mut self, data: &Data) {
        if let Some(cwd) = data.context.as_ref().and_then(|c| c.cwd.clone()) {
            self.cwd = Some(cwd);
        }
    }

    /// Records each model's usage in the run that ended with this summary.
    fn shutdown(&mut self, event_id: &str, at: UtcMillis, summary: Summary) {
        let Some(session) = self.id.clone() else {
            return;
        };
        let cumulative = self.version.is_none_or(|v| v >= CUMULATIVE_FROM);
        let previous = std::mem::take(&mut self.previous);
        let starts_over = summary.iter().any(|(model, now)| {
            previous
                .get(model)
                .is_some_and(|before| decreased(now, before))
        }) || previous.keys().any(|model| !summary.contains_key(model));
        let mut calls = std::mem::take(&mut self.pending_calls);
        for (model, now) in &summary {
            let run = match previous.get(model) {
                Some(before) if cumulative && !starts_over => difference(now, before),
                _ => *now,
            };
            if run == ModelMetrics::default() {
                continue;
            }
            let mut raw = self.raw(&session, event_id, at, model, run);
            raw.tool_calls = std::mem::take(&mut calls);
            self.usage.push(raw);
        }
        if cumulative {
            self.previous = summary;
        }
    }

    fn raw(
        &self,
        session: &str,
        event_id: &str,
        at: UtcMillis,
        model: &str,
        run: ModelMetrics,
    ) -> RawUsage {
        RawUsage {
            natural_key: format!("copilot:{session}:{event_id}:{model}"),
            precedence: 0,
            occurred_at: at,
            model: Some(model.to_owned()),
            provider: None,
            gateway: Some(GATEWAY.to_owned()),
            tokens: canonical(run.usage),
            requests: u32::try_from(run.requests.count).ok(),
            reported_cost: None,
            interface: Interface::Cli,
            service_tier: ServiceTier::Standard,
            inference_geo_us: false,
            // A total over a run, not one request.
            granularity: Granularity::Bucket,
            session_key: Some(format!("copilot:{session}")),
            parent_session_key: None,
            is_subagent: false,
            cwd: self.cwd.clone(),
            tool_version: self.version_text.clone(),
            accuracy: AccuracyMap {
                tokens: Accuracy::ProviderReported,
                // Whether the input count includes cache writes is not documented.
                cache: Accuracy::Partial,
                reasoning: Accuracy::ProviderReported,
                requests: Accuracy::ProviderReported,
                // Copilot bills premium requests; tokens are priced at API rates.
                cost: Accuracy::Estimated,
            },
            tool_calls: Vec::new(),
        }
    }
}

/// A version string such as `1.0.66` as comparable numbers; unknown parts are zero.
fn version(text: &str) -> (u64, u64, u64) {
    let mut parts = text
        .split(['.', '-'])
        .map(|p| p.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn decreased(now: &ModelMetrics, before: &ModelMetrics) -> bool {
    let (n, b) = (&now.usage, &before.usage);
    now.requests.count < before.requests.count
        || n.input_tokens < b.input_tokens
        || n.output_tokens < b.output_tokens
        || n.cache_read_tokens < b.cache_read_tokens
        || n.cache_write_tokens < b.cache_write_tokens
        || n.reasoning_tokens < b.reasoning_tokens
}

/// `now - before`; callers ensure no field decreased.
fn difference(now: &ModelMetrics, before: &ModelMetrics) -> ModelMetrics {
    let (n, b) = (&now.usage, &before.usage);
    let mut run = *now;
    run.requests.count = now.requests.count - before.requests.count;
    run.usage = Usage {
        input_tokens: n.input_tokens - b.input_tokens,
        output_tokens: n.output_tokens - b.output_tokens,
        cache_read_tokens: n.cache_read_tokens - b.cache_read_tokens,
        cache_write_tokens: n.cache_write_tokens - b.cache_write_tokens,
        reasoning_tokens: n.reasoning_tokens - b.reasoning_tokens,
    };
    run
}

/// Canonical tokens: the cached tokens are taken out of Copilot's input count.
fn canonical(usage: Usage) -> TokenCounts {
    let cached = usage.cache_read_tokens.min(usage.input_tokens);
    let written = usage.cache_write_tokens.min(usage.input_tokens - cached);
    TokenCounts {
        input_uncached: Some(usage.input_tokens - cached - written),
        cache_read: Some(cached),
        cache_write_unspecified: Some(written),
        output_total: Some(usage.output_tokens),
        reasoning: Some(usage.reasoning_tokens.min(usage.output_tokens)),
        ..TokenCounts::default()
    }
}
