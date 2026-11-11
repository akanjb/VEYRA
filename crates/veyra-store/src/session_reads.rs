//! Session-level reads: activity timestamps, the session explorer, timelines and project
//! labels. Totals use the same SQL as the rollups ([`EVENT_SUMS_SELECT`]), so a session's
//! numbers always add up to the dashboard's.

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};
use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{ActivityReader, LabelReader, ProjectLabel, RangeQuery, ReadError};
use veyra_analytics::sessions::{
    SessionDetail, SessionEvent, SessionPage, SessionQuery, SessionReader, SessionSort,
    SessionSummary, ToolUse,
};
use veyra_domain::money::NanoUsd;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;

use crate::mapping::accuracy;
use crate::reader::{filter_clause, sums_from};
use crate::sql::{EVENT_SUMS_SELECT, EVENT_TOTAL_TOKENS, count_from, dimension_info};
use crate::{Store, StoreError};

/// Session metadata selected before the sums; the sums start at [`SUMS_START`].
const SUMMARY_COLUMNS: &str = "e.session_id, MIN(e.occurred_at), MAX(e.occurred_at), s.key, \
     s.parent_session_id, s.is_subagent, MAX(e.app_id), MAX(p.key), \
     GROUP_CONCAT(DISTINCT COALESCE(e.model_id, e.raw_model))";
const SUMS_START: usize = 9;

fn read_error(error: &StoreError) -> ReadError {
    ReadError(error.to_string())
}

/// Clauses selecting events (alias `e`) in `[from, until)` that match the filters. The
/// bucket range uses the index; the timestamp range makes unaligned bounds exact.
pub(crate) fn event_clauses(
    from: UtcMillis,
    until: UtcMillis,
    filters: &Filters,
    values: &mut Vec<Value>,
) -> Vec<String> {
    let bucket_floor = from.bucket_start().0;
    values.extend([
        Value::Integer(bucket_floor),
        Value::Integer(until.0),
        Value::Integer(from.0),
    ]);
    let n = values.len();
    let mut clauses = vec![
        format!("e.bucket_start >= ?{}", n - 2),
        format!("e.bucket_start < ?{}", n - 1),
        format!("e.occurred_at >= ?{n}"),
        format!("e.occurred_at < ?{}", n - 1),
        "e.session_id IS NOT NULL".to_owned(),
    ];
    for (dimension, allowed) in filters.iter() {
        let (_, column, integer) = dimension_info(dimension);
        if let Some(clause) = filter_clause(&format!("e.{column}"), integer, allowed, values) {
            clauses.push(clause);
        }
    }
    clauses
}

fn summary_from(row: &Row<'_>) -> Result<SessionSummary, StoreError> {
    let mut models: Vec<String> = row
        .get::<_, Option<String>>(8)?
        .map(|joined| joined.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    models.sort();
    Ok(SessionSummary {
        id: row.get(0)?,
        first_at: UtcMillis(row.get(1)?),
        last_at: UtcMillis(row.get(2)?),
        key: row.get(3)?,
        parent_id: row.get(4)?,
        is_subagent: row.get::<_, i64>(5)? != 0,
        app: row.get(6)?,
        project: row.get(7)?,
        models,
        sums: sums_from(row, SUMS_START)?,
    })
}

/// Session summaries for events matching `where_sql`, then `tail` (ordering, paging).
fn summaries(
    connection: &Connection,
    where_sql: &str,
    values: Vec<Value>,
    tail: &str,
) -> Result<Vec<SessionSummary>, StoreError> {
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS}, {sums} FROM usage_events e \
         LEFT JOIN event_costs c ON c.event_id = e.id \
         JOIN sessions s ON s.id = e.session_id \
         LEFT JOIN projects p ON p.id = e.project_id \
         WHERE {where_sql} GROUP BY e.session_id {tail}",
        sums = *EVENT_SUMS_SELECT,
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| Ok(summary_from(row)))?;
    rows.map(|row| row.map_err(StoreError::from).and_then(|summary| summary))
        .collect()
}

fn order_expression(sort: SessionSort) -> String {
    match sort {
        SessionSort::LastActive => "MAX(e.occurred_at)".to_owned(),
        SessionSort::Tokens => format!("COALESCE(SUM({EVENT_TOTAL_TOKENS}), 0)"),
        SessionSort::Cost => "COALESCE(SUM(c.total_nusd), 0)".to_owned(),
        SessionSort::Events => "COUNT(*)".to_owned(),
        SessionSort::CacheWrites => "COALESCE(SUM(COALESCE(e.cache_write_short, 0) \
             + COALESCE(e.cache_write_long, 0) + COALESCE(e.cache_write_unspecified, 0)), 0)"
            .to_owned(),
        SessionSort::Duration => "MAX(e.occurred_at) - MIN(e.occurred_at)".to_owned(),
    }
}

fn page(connection: &Connection, query: &SessionQuery) -> Result<SessionPage, StoreError> {
    let mut values = Vec::new();
    let where_sql =
        event_clauses(query.from, query.until, &query.filters, &mut values).join(" AND ");
    let total: i64 = connection.query_row(
        &format!("SELECT COUNT(DISTINCT e.session_id) FROM usage_events e WHERE {where_sql}"),
        params_from_iter(values.iter()),
        |row| row.get(0),
    )?;
    values.extend([
        Value::Integer(i64::from(query.limit)),
        Value::Integer(i64::from(query.offset)),
    ]);
    let n = values.len();
    let tail = format!(
        "ORDER BY {} DESC, e.session_id DESC LIMIT ?{} OFFSET ?{n}",
        order_expression(query.sort),
        n - 1
    );
    Ok(SessionPage {
        total: count_from(Some(total), "session count")?.unwrap_or(0),
        sessions: summaries(connection, &where_sql, values, &tail)?,
    })
}

fn event_from(row: &Row<'_>) -> Result<(i64, SessionEvent), StoreError> {
    let count = |index: usize, what: &str| count_from(row.get::<_, Option<i64>>(index)?, what);
    let status: Option<String> = row.get(14)?;
    let priced = matches!(status.as_deref(), Some("priced" | "partial"));
    Ok((
        row.get(0)?,
        SessionEvent {
            at: UtcMillis(row.get(1)?),
            provider: row.get(15)?,
            model: row.get(2)?,
            raw_model: row.get(3)?,
            tokens: TokenCounts {
                input_uncached: count(4, "input tokens")?,
                cache_read: count(5, "cache read tokens")?,
                cache_write_short: count(6, "cache write tokens")?,
                cache_write_long: count(7, "cache write tokens")?,
                cache_write_unspecified: count(8, "cache write tokens")?,
                output_total: count(9, "output tokens")?,
                reasoning: count(10, "reasoning tokens")?,
                tool_tokens: count(11, "tool tokens")?,
            },
            token_accuracy: accuracy(row, 12)?,
            cost: if priced {
                row.get::<_, Option<i64>>(13)?.map(NanoUsd)
            } else {
                None
            },
            tools: Vec::new(),
        },
    ))
}

fn detail(connection: &Connection, id: i64) -> Result<Option<SessionDetail>, StoreError> {
    let Some(summary) = summaries(
        connection,
        "e.session_id = ?1",
        vec![Value::Integer(id)],
        "",
    )?
    .into_iter()
    .next() else {
        return Ok(None);
    };

    let mut tools: HashMap<i64, Vec<ToolUse>> = HashMap::new();
    let mut statement = connection.prepare_cached(
        "SELECT t.event_id, t.tool_name, t.mcp_server, t.is_error FROM tool_calls t \
         JOIN usage_events e ON e.id = t.event_id WHERE e.session_id = ?1 \
         ORDER BY t.event_id, t.tool_use_id",
    )?;
    let rows = statement.query_map([id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            ToolUse {
                name: row.get(1)?,
                mcp_server: row.get(2)?,
                is_error: row.get::<_, Option<i64>>(3)?.map(|v| v != 0),
            },
        ))
    })?;
    for row in rows {
        let (event, tool) = row?;
        tools.entry(event).or_default().push(tool);
    }

    let mut statement = connection.prepare_cached(
        "SELECT e.id, e.occurred_at, e.model_id, e.raw_model, e.input_uncached, e.cache_read, \
         e.cache_write_short, e.cache_write_long, e.cache_write_unspecified, e.output_total, \
         e.reasoning, e.tool_tokens, e.acc_tokens, c.total_nusd, c.status, e.provider_id \
         FROM usage_events e LEFT JOIN event_costs c ON c.event_id = e.id \
         WHERE e.session_id = ?1 ORDER BY e.occurred_at, e.id",
    )?;
    let rows = statement.query_map([id], |row| Ok(event_from(row)))?;
    let mut events = Vec::new();
    for row in rows {
        let (event_id, mut event) = row??;
        event.tools = tools.remove(&event_id).unwrap_or_default();
        events.push(event);
    }

    let subagents = summaries(
        connection,
        "s.parent_session_id = ?1",
        vec![Value::Integer(id)],
        "ORDER BY MIN(e.occurred_at)",
    )?;
    Ok(Some(SessionDetail {
        summary,
        events,
        subagents,
    }))
}

impl SessionReader for Store {
    fn sessions(&self, query: &SessionQuery) -> Result<SessionPage, ReadError> {
        self.with_reader(|connection| page(connection, query))
            .map_err(|e| read_error(&e))
    }

    fn session(&self, id: i64) -> Result<Option<SessionDetail>, ReadError> {
        self.with_reader(|connection| detail(connection, id))
            .map_err(|e| read_error(&e))
    }
}

/// Selected columns of matching events with a session, in the given order; `extra` is
/// one more condition, or empty.
fn timed_events<T>(
    connection: &Connection,
    query: &RangeQuery,
    columns: &str,
    extra: &str,
    order: &str,
    map: impl Fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, StoreError> {
    let mut values = Vec::new();
    let mut clauses = event_clauses(query.from, query.until, &query.filters, &mut values);
    if !extra.is_empty() {
        clauses.push(extra.to_owned());
    }
    let mut statement = connection.prepare(&format!(
        "SELECT {columns} FROM usage_events e WHERE {} ORDER BY {order}",
        clauses.join(" AND ")
    ))?;
    let rows = statement.query_map(params_from_iter(values), map)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

impl ActivityReader for Store {
    fn event_times(&self, query: &RangeQuery) -> Result<Vec<(i64, UtcMillis)>, ReadError> {
        self.with_reader(|connection| {
            timed_events(
                connection,
                query,
                "e.session_id, e.occurred_at",
                "",
                "e.session_id, e.occurred_at",
                |row| Ok((row.get::<_, i64>(0)?, UtcMillis(row.get(1)?))),
            )
        })
        .map_err(|e| read_error(&e))
    }

    fn project_event_times(
        &self,
        query: &RangeQuery,
    ) -> Result<Vec<(i64, i64, UtcMillis)>, ReadError> {
        self.with_reader(|connection| {
            timed_events(
                connection,
                query,
                "e.project_id, e.session_id, e.occurred_at",
                "e.project_id IS NOT NULL",
                "e.project_id, e.session_id, e.occurred_at",
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        UtcMillis(row.get(2)?),
                    ))
                },
            )
        })
        .map_err(|e| read_error(&e))
    }
}

impl LabelReader for Store {
    fn projects(&self) -> Result<Vec<ProjectLabel>, ReadError> {
        self.with_reader(|connection| {
            let mut statement = connection
                .prepare_cached("SELECT id, key, display_name FROM projects ORDER BY key")?;
            let rows = statement.query_map([], |row| {
                Ok(ProjectLabel {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .map_err(|e| read_error(&e))
    }
}
