//! Request and session sizes: request counts per size bin from `rollup_15m_size_totals`
//! or `rollup_15m_sizes`, and each session's tokens from `rollup_15m_sessions` or its
//! events (ADR-030).

use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};
use veyra_analytics::port::{RangeQuery, ReadError, SizeReader};
use veyra_analytics::sizes::{SessionSize, SizeRow};
use veyra_domain::time::UtcMillis;

use crate::reader::filter_clause;
use crate::session_source::session_source;
use crate::sql::{count_from, dimension_info};
use crate::{Store, StoreError};

fn request_sizes(connection: &Connection, query: &RangeQuery) -> Result<Vec<SizeRow>, StoreError> {
    let mut values = vec![Value::Integer(query.from.0), Value::Integer(query.until.0)];
    let mut clauses = vec![
        "bucket_start >= ?1".to_owned(),
        "bucket_start < ?2".to_owned(),
    ];
    for (dimension, allowed) in query.filters.iter() {
        let (_, column, integer) = dimension_info(dimension);
        if let Some(clause) = filter_clause(column, integer, allowed, &mut values) {
            clauses.push(clause);
        }
    }
    // Without filters, the totals across every dimension answer from far fewer rows.
    let table = if query.filters.is_empty() {
        "rollup_15m_size_totals"
    } else {
        "rollup_15m_sizes"
    };
    let mut statement = connection.prepare_cached(&format!(
        "SELECT bucket_start, size_bin, SUM(requests) FROM {table} WHERE {} \
         GROUP BY bucket_start, size_bin ORDER BY bucket_start, size_bin",
        clauses.join(" AND ")
    ))?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (bucket_start, bin, requests) = row?;
        Ok(SizeRow {
            bucket_start: UtcMillis(bucket_start),
            bin: u32::try_from(bin).map_err(|_| StoreError::Corrupt(format!("size bin {bin}")))?,
            requests: count_from(Some(requests), "requests")?.unwrap_or(0),
        })
    })
    .collect()
}

fn session_sizes(
    connection: &Connection,
    query: &RangeQuery,
) -> Result<Vec<SessionSize>, StoreError> {
    let mut values = Vec::new();
    let (source, clauses) = session_source(query.from, query.until, &query.filters, &mut values);
    let mut statement = connection.prepare(&format!(
        "SELECT e.session_id, {}, {} FROM {} WHERE {} GROUP BY e.session_id",
        source.first_at,
        source.tokens,
        source.from,
        clauses.join(" AND ")
    ))?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (session, first_at, tokens) = row?;
        Ok(SessionSize {
            session,
            first_at: UtcMillis(first_at),
            tokens: count_from(Some(tokens), "session tokens")?.unwrap_or(0),
        })
    })
    .collect()
}

impl SizeReader for Store {
    fn request_sizes(&self, query: &RangeQuery) -> Result<Vec<SizeRow>, ReadError> {
        self.with_reader(|connection| request_sizes(connection, query))
            .map_err(|e| ReadError(e.to_string()))
    }

    fn session_sizes(&self, query: &RangeQuery) -> Result<Vec<SessionSize>, ReadError> {
        self.with_reader(|connection| session_sizes(connection, query))
            .map_err(|e| ReadError(e.to_string()))
    }
}
