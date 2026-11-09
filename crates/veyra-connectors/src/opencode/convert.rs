//! Turns the rows of the OpenCode queries into usage records.
//!
//! OpenCode counts input without cache reads and writes. Version 1.4 and later, and the
//! 2.0 tables, count reasoning apart from output; earlier 1.x versions count it within
//! output. Versions before 1.0.100 recorded the raw provider values, whose cache
//! semantics vary by provider, so their tokens are marked partial.

use std::collections::HashMap;

use serde::Deserialize;
use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::source::RawUsage;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::{ToolCall, ToolOutcome};

use crate::shared::BestByKey;

/// Providers that route requests to model vendors; the vendor comes from the model.
const GATEWAYS: [&str; 3] = ["github-copilot", "openrouter", "opencode"];

/// One query row; `kind` says which.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Row {
    pub kind: String,
    // Usage rows.
    pub schema: u8,
    pub id: Option<String>,
    pub session: Option<String>,
    pub parent: Option<String>,
    pub directory: Option<String>,
    pub version: Option<String>,
    pub at: Option<i64>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    // Tool rows.
    pub message: Option<String>,
    pub call: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
}

/// Accumulates the rows of one database.
#[derive(Debug, Default)]
pub struct Rows {
    usage: Vec<RawUsage>,
    /// Tool calls by message id.
    calls: HashMap<String, Vec<ToolCall>>,
    pub outcomes: Vec<ToolOutcome>,
}

impl Rows {
    /// Adds a row; `false` when it is not usable.
    pub fn add(&mut self, row: Row) -> bool {
        match row.kind.as_str() {
            "usage" => usage(row).map(|u| self.usage.push(u)).is_some(),
            "tool" => self.tool(row),
            _ => false,
        }
    }

    fn tool(&mut self, row: Row) -> bool {
        let (Some(message), Some(id), Some(name)) = (row.message, row.call, row.name) else {
            return false;
        };
        match row.status.as_deref() {
            Some("completed") => self.outcomes.push(ToolOutcome {
                tool_call_id: id.clone(),
                is_error: false,
            }),
            Some("error") => self.outcomes.push(ToolOutcome {
                tool_call_id: id.clone(),
                is_error: true,
            }),
            _ => {}
        }
        let calls = self.calls.entry(message).or_default();
        if !calls.iter().any(|c| c.id == id) {
            calls.push(ToolCall { id, name });
        }
        true
    }

    /// One usage record per message (the newer copy of an imported message), with its
    /// tool calls attached.
    pub fn into_usage(self) -> Vec<RawUsage> {
        let mut best = BestByKey::default();
        for mut usage in self.usage {
            let message = usage.natural_key.trim_start_matches("opencode:");
            usage.tool_calls = self.calls.get(message).cloned().unwrap_or_default();
            best.add(usage);
        }
        best.into_records()
    }
}

/// A version string such as `1.3.17` as comparable numbers; unknown parts are zero.
fn version(text: Option<&str>) -> (u64, u64, u64) {
    let mut parts = text
        .unwrap_or("")
        .split(['.', '-'])
        .map(|p| p.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn usage(row: Row) -> Option<RawUsage> {
    let id = row.id?;
    let session = row.session?;
    let created_by = version(row.version.as_deref());
    // The 2.0 tables and 1.4 or later count reasoning apart from output.
    let reasoning_apart = row.schema >= 2 || created_by >= (1, 4, 0);
    let raw_values = row.schema < 2 && created_by < (1, 0, 100);
    let output = row.output.unwrap_or(0);
    let reasoning = row.reasoning.unwrap_or(0);
    let tokens = TokenCounts {
        input_uncached: row.input,
        cache_read: row.cache_read,
        cache_write_unspecified: row.cache_write,
        output_total: Some(if reasoning_apart {
            output + reasoning
        } else {
            output
        }),
        reasoning: Some(if reasoning_apart {
            reasoning
        } else {
            reasoning.min(output)
        }),
        ..TokenCounts::default()
    };
    let gateway = row
        .provider
        .as_deref()
        .filter(|p| GATEWAYS.contains(p))
        .map(str::to_owned);
    Some(RawUsage {
        natural_key: format!("opencode:{id}"),
        // The newer copy of an imported message wins.
        precedence: i64::from(row.schema),
        occurred_at: UtcMillis(row.at?),
        model: row.model,
        provider: if gateway.is_some() {
            None
        } else {
            row.provider
        },
        gateway,
        tokens,
        requests: Some(1),
        reported_cost: None,
        interface: Interface::Cli,
        service_tier: ServiceTier::Standard,
        inference_geo_us: false,
        granularity: Granularity::Request,
        session_key: Some(format!("opencode:{session}")),
        parent_session_key: row.parent.as_ref().map(|p| format!("opencode:{p}")),
        is_subagent: row.parent.is_some(),
        cwd: row.directory,
        tool_version: row.version,
        accuracy: AccuracyMap {
            tokens: if raw_values {
                Accuracy::Partial
            } else {
                Accuracy::Exact
            },
            cache: if raw_values {
                Accuracy::Partial
            } else {
                Accuracy::Exact
            },
            reasoning: Accuracy::Exact,
            requests: Accuracy::Exact,
            // Veyra prices tokens itself; OpenCode's own cost is an estimate too.
            cost: Accuracy::Estimated,
        },
        tool_calls: Vec::new(),
    })
}
