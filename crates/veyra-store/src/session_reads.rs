//! Session-level reads: activity spans, the session explorer, timelines and project
//! labels. Rows come from `rollup_15m_sessions` when it answers a query exactly and from
//! the events otherwise (ADR-030); both sum like the rollups, so a session's numbers
//! always add up to the dashboard's.

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};
use veyra_analytics::activity::{DEFAULT_IDLE_THRESHOLD_MS, Span, spans_of};
use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{ActivityReader, LabelReader, ProjectLabel, RangeQuery, ReadError};
use veyra_analytics::sessions::{
    SessionDetail, SessionEvent, SessionPage, SessionQuery, SessionReader, SessionSummary, ToolUse,
};
use veyra_domain::money::NanoUsd;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;

use crate::mapping::accuracy;
use crate::reader::{filter_clause, sums_from};
use crate::session_source::{SessionSource, event_source, session_source};
use crate::sql::{count_from, dimension_info};
use crate::{Store, StoreError, spans};

/// The column of the models list in a summary row; the sums start at [`SUMS_START`].
const MODELS_COLUMN: usize = 8;
const SUMS_START: usize = 9;

fn read_error(error: &StoreError) -> ReadError {
    ReadError(error.to_string())
}

/// Clauses selecting events (alias `e`) in `[from, until)` that match the filters. The
/// bucket range uses the index; the timestamp range makes unaligned bounds exact.
pub(crate) fn range_clauses(
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
    ];
    for (dimension, allowed) in filters.iter() {
        let (_, column, integer) = dimension_info(dimension);
        if let Some(clause) = filter_clause(&format!("e.{column}"), integer, allowed, values) {
            clauses.push(clause);
        }
    }
    clauses
}

/// [`range_clauses`] for events that belong to a session.
pub(crate) fn event_clauses(
    from: UtcMillis,
    until: UtcMillis,
    filters: &Filters,
    values: &mut Vec<Value>,
) -> Vec<String> {
    let mut clauses = range_clauses(from, until, filters, values);
    clauses.push("e.session_id IS NOT NULL".to_owned());
    clauses
}

fn summary_from(row: &Row<'_>) -> Result<SessionSummary, StoreError> {
    // Rollup rows list their own models, so a session's list may repeat names.
    let mut models: Vec<String> = row
        .get::<_, Option<String>>(MODELS_COLUMN)?
        .map(|joined| joined.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    models.sort();
    models.dedup();
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

/// Session summaries from `source` for rows matching `where_sql`, then `tail` (ordering).
fn summaries(
    connection: &Connection,
    source: &SessionSource,
    where_sql: &str,
    values: Vec<Value>,
    tail: &str,
) -> Result<Vec<SessionSummary>, StoreError> {
    let sql = format!(
        "SELECT e.session_id, {first}, {last}, s.key, s.parent_session_id, s.is_subagent, \
         MAX(e.app_id), MAX(p.key), {models}, {sums} FROM {from} \
         JOIN sessions s ON s.id = e.session_id \
         LEFT JOIN projects p ON p.id = e.project_id \
         WHERE {where_sql} GROUP BY e.session_id {tail}",
        first = source.first_at,
        last = source.last_at,
        models = source.models,
        sums = source.sums,
        from = source.from,
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| Ok(summary_from(row)))?;
    rows.map(|row| row.map_err(StoreError::from).and_then(|summary| summary))
        .collect()
}

/// The ids of one page of sessions, in display order. Ranking needs only each
/// session's sort key, so the full sums are computed for the page's sessions alone.
fn page_ids(
    connection: &Connection,
    source: &SessionSource,
    where_sql: &str,
    values: &[Value],
    query: &SessionQuery,
) -> Result<Vec<i64>, StoreError> {
    let n = values.len();
    let mut statement = connection.prepare(&format!(
        "SELECT e.session_id FROM {} WHERE {where_sql} \
         GROUP BY e.session_id ORDER BY {} DESC, e.session_id DESC LIMIT ?{} OFFSET ?{}",
        source.from,
        source.order(query.sort),
        n + 1,
        n + 2
    ))?;
    let paging = [
        Value::Integer(i64::from(query.limit)),
        Value::Integer(i64::from(query.offset)),
    ];
    let rows = statement.query_map(params_from_iter(values.iter().chain(&paging)), |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn page(connection: &Connection, query: &SessionQuery) -> Result<SessionPage, StoreError> {
    let mut values = Vec::new();
    let (source, clauses) = session_source(query.from, query.until, &query.filters, &mut values);
    let where_sql = clauses.join(" AND ");
    let total: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(DISTINCT e.session_id) FROM {} WHERE {where_sql}",
            source.from
        ),
        params_from_iter(values.iter()),
        |row| row.get(0),
    )?;
    let ids = page_ids(connection, &source, &where_sql, &values, query)?;
    let mut sessions = if ids.is_empty() {
        Vec::new()
    } else {
        summaries(
            connection,
            &source,
            &format!("{where_sql} AND {}", sessions_in(&ids)),
            values,
            "",
        )?
    };
    let position: HashMap<i64, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    sessions.sort_by_key(|summary| position.get(&summary.id).copied());
    Ok(SessionPage {
        total: count_from(Some(total), "session count")?.unwrap_or(0),
        sessions,
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
        &event_source(),
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
        &event_source(),
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

/// The SQL condition selecting the given sessions. Ids are integers read from the
/// database, so they are written into the SQL directly.
pub(crate) fn sessions_in(sessions: &[i64]) -> String {
    let ids: Vec<String> = sessions.iter().map(i64::to_string).collect();
    format!("e.session_id IN ({})", ids.join(", "))
}

/// Which spans to read: per project or not, and for some sessions only.
#[derive(Clone, Copy)]
struct SpanRequest<'a> {
    by_project: bool,
    sessions: Option<&'a [i64]>,
}

impl SpanRequest<'_> {
    /// The project column, or a constant when spans are not split by project.
    fn project(self) -> &'static str {
        if self.by_project {
            "e.project_id"
        } else {
            "NULL"
        }
    }

    fn conditions(self) -> Vec<String> {
        let mut conditions = Vec::new();
        if self.by_project {
            conditions.push("e.project_id IS NOT NULL".to_owned());
        }
        if let Some(sessions) = self.sessions {
            conditions.push(sessions_in(sessions));
        }
        conditions
    }
}

/// `(project, span)` from raw events matching the query, `request` and `extra`.
fn event_spans(
    connection: &Connection,
    query: &RangeQuery,
    request: SpanRequest<'_>,
    extra: &[String],
) -> Result<Vec<(Option<i64>, Span)>, StoreError> {
    let mut values = Vec::new();
    let mut clauses = event_clauses(query.from, query.until, &query.filters, &mut values);
    clauses.extend(request.conditions());
    clauses.extend_from_slice(extra);
    let mut statement = connection.prepare(&format!(
        "SELECT {}, e.session_id, e.occurred_at FROM usage_events e WHERE {} ORDER BY 1, 2, 3",
        request.project(),
        clauses.join(" AND ")
    ))?;
    let rows = statement
        .query_map(params_from_iter(values), |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                (row.get::<_, i64>(1)?, UtcMillis(row.get(2)?)),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .chunk_by(|a, b| a.0 == b.0)
        .flat_map(|group| {
            let events: Vec<(i64, UtcMillis)> = group.iter().map(|(_, event)| *event).collect();
            spans_of(&events, DEFAULT_IDLE_THRESHOLD_MS)
                .into_iter()
                .map(move |span| (group[0].0, span))
        })
        .collect())
}

/// One row of `rollup_15m_sessions`: project, session, bucket, first, last, active.
type SpanRow = (Option<i64>, i64, i64, i64, i64, String);

/// `(project, span)` from `rollup_15m_sessions`. A session's bucket usually has one row;
/// when it has several (the session changed project, application or interface within
/// the bucket), its span is rebuilt from that bucket's events so it stays exact.
fn rollup_spans(
    connection: &Connection,
    query: &RangeQuery,
    request: SpanRequest<'_>,
    mut values: Vec<Value>,
    mut clauses: Vec<String>,
) -> Result<Vec<(Option<i64>, Span)>, StoreError> {
    clauses.extend(request.conditions());
    let mut statement = connection.prepare(&format!(
        "SELECT {}, e.session_id, e.bucket_start, e.first_at, e.last_at, e.active \
         FROM rollup_15m_sessions e WHERE {} ORDER BY 1, 2, 3, 4",
        request.project(),
        clauses.join(" AND ")
    ))?;
    let rows: Vec<SpanRow> = statement
        .query_map(params_from_iter(values.drain(..)), |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut spans = Vec::with_capacity(rows.len());
    for group in rows.chunk_by(|a, b| (a.0, a.1, a.2) == (b.0, b.1, b.2)) {
        let (project, session, bucket, first, last, active) = &group[0];
        if group.len() == 1 {
            let span = Span {
                session: *session,
                first: *first,
                last: *last,
                active: spans::decode(active)?,
            };
            spans.push((*project, span));
        } else {
            let mut extra = vec![
                format!("e.session_id = {session}"),
                format!("e.bucket_start = {bucket}"),
            ];
            if let Some(project) = project {
                extra.push(format!("e.project_id = {project}"));
            }
            spans.extend(event_spans(connection, query, request, &extra)?);
        }
    }
    Ok(spans)
}

/// `(project, span)` of matching sessions, from the rollup when it answers exactly.
fn read_spans(
    connection: &Connection,
    query: &RangeQuery,
    request: SpanRequest<'_>,
) -> Result<Vec<(Option<i64>, Span)>, StoreError> {
    let mut values = Vec::new();
    let (source, clauses) = session_source(query.from, query.until, &query.filters, &mut values);
    if source.is_rollup {
        rollup_spans(connection, query, request, values, clauses)
    } else {
        event_spans(connection, query, request, &[])
    }
}

fn without_projects(spans: Vec<(Option<i64>, Span)>) -> Vec<Span> {
    spans.into_iter().map(|(_, span)| span).collect()
}

impl ActivityReader for Store {
    fn spans(&self, query: &RangeQuery) -> Result<Vec<Span>, ReadError> {
        let request = SpanRequest {
            by_project: false,
            sessions: None,
        };
        self.with_reader(|connection| read_spans(connection, query, request).map(without_projects))
            .map_err(|e| read_error(&e))
    }

    fn session_spans(&self, query: &RangeQuery, sessions: &[i64]) -> Result<Vec<Span>, ReadError> {
        if sessions.is_empty() {
            return Ok(Vec::new());
        }
        let request = SpanRequest {
            by_project: false,
            sessions: Some(sessions),
        };
        self.with_reader(|connection| read_spans(connection, query, request).map(without_projects))
            .map_err(|e| read_error(&e))
    }

    fn project_spans(&self, query: &RangeQuery) -> Result<Vec<(i64, Span)>, ReadError> {
        let request = SpanRequest {
            by_project: true,
            sessions: None,
        };
        self.with_reader(|connection| {
            let spans = read_spans(connection, query, request)?;
            Ok(spans
                .into_iter()
                .filter_map(|(project, span)| Some((project?, span)))
                .collect())
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
