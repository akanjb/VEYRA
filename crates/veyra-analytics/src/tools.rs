//! Tool calls: what the agents did, which MCP servers they used and how often a call
//! failed (ADR-034). Counts come from the tool rollup; nothing here reads a tool's input
//! or output, which Veyra never stores.

use std::collections::BTreeMap;

use veyra_domain::time::UtcMillis;
use veyra_domain::tooling::ToolCategory;

use crate::engine::period_index;

/// What the failure rate means.
pub const FAILURE_RATE_DEFINITION: &str = "Calls reported as errors, divided by the calls whose outcome is known. Applications that do not report outcomes are left out of both.";

/// What calls per request means.
pub const CALLS_PER_REQUEST_DEFINITION: &str =
    "Tool calls divided by model requests: how much each request asked the tools to do.";

/// What the MCP share means.
pub const MCP_SHARE_DEFINITION: &str =
    "Calls to tools provided by MCP servers, divided by all tool calls.";

/// What the subagent share means.
pub const SUBAGENT_SHARE_DEFINITION: &str =
    "Calls made inside subagents, divided by all tool calls.";

/// Additive counts of tool calls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolCounts {
    pub calls: u64,
    /// Calls reported as errors.
    pub failed: u64,
    /// Calls whose outcome is known, failed or not.
    pub with_outcome: u64,
    /// Calls made inside subagents.
    pub in_subagents: u64,
}

impl ToolCounts {
    /// Adds `other` to these counts.
    pub fn add(&mut self, other: &Self) {
        self.calls = self.calls.saturating_add(other.calls);
        self.failed = self.failed.saturating_add(other.failed);
        self.with_outcome = self.with_outcome.saturating_add(other.with_outcome);
        self.in_subagents = self.in_subagents.saturating_add(other.in_subagents);
    }

    /// Failed calls over calls with a known outcome; `None` when no outcome is known.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a ratio shown to the user")]
    pub fn failure_rate(&self) -> Option<f64> {
        (self.with_outcome > 0).then(|| self.failed as f64 / self.with_outcome as f64)
    }
}

/// `part / whole`, or `None` when `whole` is zero.
#[must_use]
#[expect(clippy::cast_precision_loss, reason = "a ratio shown to the user")]
pub fn share(part: u64, whole: u64) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

/// The calls of one tool over a range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolUse {
    pub tool: String,
    /// The MCP server providing the tool, for MCP tools.
    pub mcp_server: Option<String>,
    pub category: ToolCategory,
    pub counts: ToolCounts,
}

/// The calls of one category in one 15-minute bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryCalls {
    pub bucket_start: UtcMillis,
    pub category: ToolCategory,
    pub calls: u64,
}

/// The calls to one MCP server's tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerUse {
    pub server: String,
    /// Distinct tools of the server that were called.
    pub tools: u32,
    pub counts: ToolCounts,
}

/// The counts of every tool together.
#[must_use]
pub fn total(uses: &[ToolUse]) -> ToolCounts {
    let mut counts = ToolCounts::default();
    for tool in uses {
        counts.add(&tool.counts);
    }
    counts
}

/// The counts per category, in [`ToolCategory::ALL`] order, with categories never
/// called left out.
#[must_use]
pub fn by_category(uses: &[ToolUse]) -> Vec<(ToolCategory, ToolCounts)> {
    let mut counts: BTreeMap<ToolCategory, ToolCounts> = BTreeMap::new();
    for tool in uses {
        counts.entry(tool.category).or_default().add(&tool.counts);
    }
    ToolCategory::ALL
        .into_iter()
        .filter_map(|category| counts.get(&category).map(|c| (category, *c)))
        .collect()
}

/// The MCP servers used, most called first (ties by name).
#[must_use]
pub fn by_server(uses: &[ToolUse]) -> Vec<ServerUse> {
    let mut servers: BTreeMap<&str, ServerUse> = BTreeMap::new();
    for tool in uses {
        let Some(server) = tool.mcp_server.as_deref() else {
            continue;
        };
        let entry = servers.entry(server).or_insert_with(|| ServerUse {
            server: server.to_owned(),
            tools: 0,
            counts: ToolCounts::default(),
        });
        entry.tools += 1;
        entry.counts.add(&tool.counts);
    }
    let mut servers: Vec<ServerUse> = servers.into_values().collect();
    servers.sort_by_key(|s| std::cmp::Reverse(s.counts.calls));
    servers
}

/// Calls per period and category: one row per period start, with a count per
/// [`ToolCategory::ALL`] entry. Buckets before the first start are ignored.
#[must_use]
pub fn calls_by_period(
    rows: &[CategoryCalls],
    starts: &[UtcMillis],
) -> Vec<[u64; ToolCategory::ALL.len()]> {
    let mut periods = vec![[0_u64; ToolCategory::ALL.len()]; starts.len()];
    for row in rows {
        let Some(period) = period_index(starts, row.bucket_start) else {
            continue;
        };
        let Some(slot) = ToolCategory::ALL.iter().position(|c| *c == row.category) else {
            continue;
        };
        periods[period][slot] = periods[period][slot].saturating_add(row.calls);
    }
    periods
}

#[cfg(test)]
mod tests {
    use veyra_domain::time::UtcMillis;
    use veyra_domain::tooling::ToolCategory;

    use super::{
        CategoryCalls, ToolCounts, ToolUse, by_category, by_server, calls_by_period, share, total,
    };

    fn tool(name: &str, server: Option<&str>, calls: u64, failed: u64) -> ToolUse {
        ToolUse {
            tool: name.to_owned(),
            mcp_server: server.map(str::to_owned),
            category: ToolCategory::of(name),
            counts: ToolCounts {
                calls,
                failed,
                with_outcome: calls,
                in_subagents: 0,
            },
        }
    }

    #[test]
    fn counts_add_up_per_category_and_server() {
        let uses = [
            tool("Bash", None, 10, 2),
            tool("Read", None, 30, 0),
            tool("mcp__github__get_issue", Some("github"), 4, 1),
            tool("mcp__github__list_prs", Some("github"), 2, 0),
            tool("mcp__docs__search", Some("docs"), 5, 0),
        ];
        let all = total(&uses);
        assert_eq!(all.calls, 51);
        assert_eq!(all.failure_rate(), Some(3.0 / 51.0));

        let categories = by_category(&uses);
        assert_eq!(
            categories[0].0,
            ToolCategory::ReadSearch,
            "in display order"
        );
        assert_eq!(categories.len(), 3);

        let servers = by_server(&uses);
        assert_eq!(servers[0].server, "github");
        assert_eq!(servers[0].tools, 2);
        assert_eq!(servers[0].counts.calls, 6);
        assert_eq!(servers[1].server, "docs");
    }

    #[test]
    fn unknown_outcomes_give_no_failure_rate() {
        assert_eq!(ToolCounts::default().failure_rate(), None);
        assert_eq!(share(1, 0), None);
        assert_eq!(share(1, 4), Some(0.25));
    }

    #[test]
    fn calls_fall_into_their_period() {
        let rows = [
            CategoryCalls {
                bucket_start: UtcMillis(5),
                category: ToolCategory::Shell,
                calls: 3,
            },
            CategoryCalls {
                bucket_start: UtcMillis(15),
                category: ToolCategory::Mcp,
                calls: 2,
            },
            CategoryCalls {
                bucket_start: UtcMillis(-1),
                category: ToolCategory::Mcp,
                calls: 9,
            },
        ];
        let periods = calls_by_period(&rows, &[UtcMillis(0), UtcMillis(10)]);
        assert_eq!(periods[0][2], 3);
        assert_eq!(periods[1][6], 2);
        assert_eq!(periods.iter().flatten().sum::<u64>(), 5);
    }
}
