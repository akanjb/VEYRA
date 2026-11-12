//! Where session reads come from: the raw events, or `rollup_15m_sessions` (ADR-030).
//!
//! Both are aliased `e` and share the column names the reads filter and group by
//! (`session_id`, `project_id`, `app_id`, `interface`, `bucket_start`), so one query
//! shape serves both. The rollup is used whenever it answers a query exactly: the bounds
//! fall on buckets and the filters are on the rollup's keys only.

use rusqlite::types::Value;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::sessions::SessionSort;
use veyra_domain::time::UtcMillis;

use crate::reader::filter_clause;
use crate::session_reads::event_clauses;
use crate::sql::{EVENT_SUMS_SELECT, EVENT_TOTAL_TOKENS, dimension_info, rollup_sums_select};

/// The dimensions `rollup_15m_sessions` is keyed by, besides session and bucket.
const SESSION_DIMENSIONS: [Dimension; 3] =
    [Dimension::Project, Dimension::App, Dimension::Interface];

/// One source of session rows.
pub(crate) struct SessionSource {
    /// The table, aliased `e`, with any join its sums need.
    pub from: &'static str,
    pub first_at: &'static str,
    pub last_at: &'static str,
    /// The models of a group, comma-separated (possibly repeated across rows).
    pub models: &'static str,
    pub tokens: String,
    pub sums: String,
    /// Whether rows are whole buckets (and may need merging per session and bucket).
    pub is_rollup: bool,
}

impl SessionSource {
    /// The expression a session page is ordered by.
    pub fn order(&self, sort: SessionSort) -> String {
        let (first, last) = (self.first_at, self.last_at);
        match (sort, self.is_rollup) {
            (SessionSort::LastActive, _) => last.to_owned(),
            (SessionSort::Tokens, _) => self.tokens.clone(),
            (SessionSort::Cost, true) => "SUM(e.cost_total_nusd)".to_owned(),
            (SessionSort::Cost, false) => "COALESCE(SUM(c.total_nusd), 0)".to_owned(),
            (SessionSort::Events, true) => "SUM(e.events)".to_owned(),
            (SessionSort::Events, false) => "COUNT(*)".to_owned(),
            (SessionSort::CacheWrites, _) => "COALESCE(SUM(COALESCE(e.cache_write_short, 0) \
                 + COALESCE(e.cache_write_long, 0) + COALESCE(e.cache_write_unspecified, 0)), 0)"
                .to_owned(),
            (SessionSort::Duration, _) => format!("{last} - {first}"),
        }
    }
}

/// Raw events.
pub(crate) fn event_source() -> SessionSource {
    SessionSource {
        from: "usage_events e LEFT JOIN event_costs c ON c.event_id = e.id",
        first_at: "MIN(e.occurred_at)",
        last_at: "MAX(e.occurred_at)",
        models: "GROUP_CONCAT(DISTINCT COALESCE(e.model_id, e.raw_model))",
        tokens: format!("COALESCE(SUM({EVENT_TOTAL_TOKENS}), 0)"),
        sums: EVENT_SUMS_SELECT.clone(),
        is_rollup: false,
    }
}

/// `rollup_15m_sessions`.
fn rollup_source() -> SessionSource {
    SessionSource {
        from: "rollup_15m_sessions e",
        first_at: "MIN(e.first_at)",
        last_at: "MAX(e.last_at)",
        models: "GROUP_CONCAT(e.models)",
        tokens: "SUM(e.tokens_total)".to_owned(),
        sums: rollup_sums_select(),
        is_rollup: true,
    }
}

/// Whether `rollup_15m_sessions` answers a query over `[from, until)` exactly.
fn rollup_answers(from: UtcMillis, until: UtcMillis, filters: &Filters) -> bool {
    from.is_bucket_aligned()
        && until.is_bucket_aligned()
        && filters
            .iter()
            .all(|(dimension, _)| SESSION_DIMENSIONS.contains(&dimension))
}

/// The source for a query, and the clauses (binding into `values`) that select its rows
/// in `[from, until)` matching the filters, events without a session excluded.
pub(crate) fn session_source(
    from: UtcMillis,
    until: UtcMillis,
    filters: &Filters,
    values: &mut Vec<Value>,
) -> (SessionSource, Vec<String>) {
    if !rollup_answers(from, until, filters) {
        return (event_source(), event_clauses(from, until, filters, values));
    }
    values.extend([Value::Integer(from.0), Value::Integer(until.0)]);
    let n = values.len();
    let mut clauses = vec![
        format!("e.bucket_start >= ?{}", n - 1),
        format!("e.bucket_start < ?{n}"),
    ];
    for (dimension, allowed) in filters.iter() {
        let (_, column, integer) = dimension_info(dimension);
        if let Some(clause) = filter_clause(&format!("e.{column}"), integer, allowed, values) {
            clauses.push(clause);
        }
    }
    (rollup_source(), clauses)
}
