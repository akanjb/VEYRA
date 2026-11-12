//! The Tools & Agents page: what did the agents do with their tools, which MCP servers
//! did they use, and which tools fail? Every number comes from the analytics engine
//! (ADR-034).

use std::cmp::Reverse;
use std::thread;

use veyra_analytics::engine::period_starts;
use veyra_analytics::metrics::MetricValue;
use veyra_analytics::query::{Period, TimeGrain};
use veyra_analytics::tools::{
    self as engine, CALLS_PER_REQUEST_DEFINITION, FAILURE_RATE_DEFINITION, MCP_SHARE_DEFINITION,
    SUBAGENT_SHARE_DEFINITION, ToolCounts, ToolUse, share,
};
use veyra_domain::tooling::{ToolCategory, mcp_parts};

use super::kpis::kpi;
use super::{evaluated, internal, joined, local_date};
use crate::DataServices;
use crate::dto::{
    McpServerRow, ToolCallCounts, ToolKind, ToolKindRow, ToolRow, Tools, ToolsDay, ToolsKpis,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Tools listed, most called first.
const TOOLS_SHOWN: usize = 25;
/// Tools listed by failed calls.
const FAILING_SHOWN: usize = 10;

/// What the calls figure means.
const CALLS_DEFINITION: &str = "Tool calls the models asked for, counted once each.";

/// The tools of a period and its model requests.
struct PeriodTools {
    uses: Vec<ToolUse>,
    requests: Option<u64>,
}

impl PeriodTools {
    fn counts(&self) -> ToolCounts {
        engine::total(&self.uses)
    }

    fn mcp_calls(&self) -> u64 {
        self.uses
            .iter()
            .filter(|u| u.category == ToolCategory::Mcp)
            .map(|u| u.counts.calls)
            .sum()
    }
}

fn period_tools(
    data: &DataServices,
    scope: &Resolved,
    period: Period,
    requests: Option<u64>,
) -> Result<PeriodTools, ApiError> {
    let uses = data
        .analytics
        .tool_uses(&scope.range(period))
        .map_err(|e| internal(&e))?;
    Ok(PeriodTools { uses, requests })
}

const fn kind(category: ToolCategory) -> ToolKind {
    match category {
        ToolCategory::ReadSearch => ToolKind::ReadSearch,
        ToolCategory::Edit => ToolKind::Edit,
        ToolCategory::Shell => ToolKind::Shell,
        ToolCategory::Web => ToolKind::Web,
        ToolCategory::Subagent => ToolKind::Subagent,
        ToolCategory::Planning => ToolKind::Planning,
        ToolCategory::Mcp => ToolKind::Mcp,
        ToolCategory::Other => ToolKind::Other,
    }
}

const fn counts(counts: &ToolCounts) -> ToolCallCounts {
    ToolCallCounts {
        calls: counts.calls,
        failed: counts.failed,
        with_outcome: counts.with_outcome,
    }
}

fn ratio(value: Option<f64>) -> Option<MetricValue> {
    value.map(MetricValue::Ratio)
}

fn kpis(now: &PeriodTools, before: Option<&PeriodTools>) -> ToolsKpis {
    let all = now.counts();
    let earlier = before.map(|b| (b.counts(), b));
    // Counts of calls carry no token accuracy.
    let figure = |current: Option<MetricValue>,
                  previous: &dyn Fn(&ToolCounts, &PeriodTools) -> Option<MetricValue>,
                  definition: &str| {
        kpi(
            current,
            earlier.as_ref().and_then(|(c, b)| previous(c, b)),
            None,
            definition,
        )
    };
    let per_request = |c: &ToolCounts, p: &PeriodTools| {
        ratio(p.requests.and_then(|requests| share(c.calls, requests)))
    };
    ToolsKpis {
        calls: figure(
            Some(MetricValue::Count(all.calls)),
            &|c, _| Some(MetricValue::Count(c.calls)),
            CALLS_DEFINITION,
        ),
        calls_per_request: figure(
            per_request(&all, now),
            &per_request,
            CALLS_PER_REQUEST_DEFINITION,
        ),
        failure_rate: figure(
            ratio(all.failure_rate()),
            &|c, _| ratio(c.failure_rate()),
            FAILURE_RATE_DEFINITION,
        ),
        mcp_share: figure(
            ratio(share(now.mcp_calls(), all.calls)),
            &|c, p| ratio(share(p.mcp_calls(), c.calls)),
            MCP_SHARE_DEFINITION,
        ),
        subagent_share: figure(
            ratio(share(all.in_subagents, all.calls)),
            &|c, _| ratio(share(c.in_subagents, c.calls)),
            SUBAGENT_SHARE_DEFINITION,
        ),
    }
}

fn tool_row(tool: &ToolUse) -> ToolRow {
    let label = mcp_parts(&tool.tool).map_or(tool.tool.as_str(), |(_, name)| name);
    ToolRow {
        name: tool.tool.clone(),
        label: label.to_owned(),
        mcp_server: tool.mcp_server.clone(),
        kind: kind(tool.category),
        counts: counts(&tool.counts),
    }
}

/// The most called tools, and those with the most failed calls (ties by name).
fn rankings(uses: &[ToolUse]) -> (Vec<ToolRow>, Vec<ToolRow>) {
    let mut by_calls: Vec<&ToolUse> = uses.iter().collect();
    by_calls.sort_by_key(|u| Reverse(u.counts.calls));
    let mut by_failures: Vec<&ToolUse> = uses.iter().filter(|u| u.counts.failed > 0).collect();
    by_failures.sort_by_key(|u| Reverse(u.counts.failed));
    let rows = |tools: Vec<&ToolUse>, shown| tools.into_iter().take(shown).map(tool_row).collect();
    (
        rows(by_calls, TOOLS_SHOWN),
        rows(by_failures, FAILING_SHOWN),
    )
}

/// Calls per local day and kind, with the kinds called in the period.
fn daily(
    data: &DataServices,
    scope: &Resolved,
    kinds: &[ToolCategory],
) -> Result<Vec<ToolsDay>, ApiError> {
    let rows = data
        .analytics
        .category_calls(&scope.range(scope.period))
        .map_err(|e| internal(&e))?;
    let starts =
        period_starts(&scope.period, TimeGrain::Day, &scope.time_zone).map_err(|e| internal(&e))?;
    let slots: Vec<usize> = kinds
        .iter()
        .filter_map(|k| ToolCategory::ALL.iter().position(|c| c == k))
        .collect();
    starts
        .iter()
        .zip(engine::calls_by_period(&rows, &starts))
        .map(|(start, calls)| {
            Ok(ToolsDay {
                date: local_date(*start, &scope.time_zone)?,
                calls: slots.iter().map(|slot| calls[*slot]).collect(),
            })
        })
        .collect()
}

/// Builds the Tools & Agents page for a scope.
pub(crate) fn tools(data: &DataServices, scope: &Resolved) -> Result<Tools, ApiError> {
    let totals = evaluated(data, &scope.spec(vec![], TimeGrain::Total, true))?;
    thread::scope(|s| {
        let before = s.spawn(|| {
            scope
                .previous
                .map(|period| {
                    let requests = totals.previous_totals.as_ref().and_then(|t| t.requests);
                    period_tools(data, scope, period, requests)
                })
                .transpose()
        });
        let now = period_tools(data, scope, scope.period, totals.totals.requests)?;
        let by_kind = engine::by_category(&now.uses);
        let kinds: Vec<ToolCategory> = by_kind.iter().map(|(k, _)| *k).collect();
        let daily = s.spawn(move || daily(data, scope, &kinds));
        let (tools, failing) = rankings(&now.uses);
        Ok(Tools {
            kpis: kpis(&now, joined(before)?.as_ref()),
            kinds: by_kind
                .iter()
                .map(|(k, c)| ToolKindRow {
                    kind: kind(*k),
                    counts: counts(c),
                })
                .collect(),
            daily: joined(daily)?,
            tools,
            servers: engine::by_server(&now.uses)
                .iter()
                .map(|s| McpServerRow {
                    server: s.server.clone(),
                    tools: s.tools,
                    counts: counts(&s.counts),
                })
                .collect(),
            failing,
            failure_definition: FAILURE_RATE_DEFINITION.to_owned(),
        })
    })
}
