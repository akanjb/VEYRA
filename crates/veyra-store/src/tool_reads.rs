//! Tool calls from `rollup_15m_tools` (ADR-034). The rollup holds every filterable
//! dimension, so each query, filtered or not, reads it alone.

use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};
use veyra_analytics::port::{RangeQuery, ReadError, ToolReader};
use veyra_analytics::tools::{CategoryCalls, ToolCounts, ToolUse};
use veyra_domain::time::UtcMillis;
use veyra_domain::tooling::ToolCategory;

use crate::reader::filter_clause;
use crate::sql::{count_from, dimension_info};
use crate::{Store, StoreError};

/// The clauses selecting the query's rows, binding into the returned values.
fn clauses(query: &RangeQuery) -> (String, Vec<Value>) {
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
    (clauses.join(" AND "), values)
}

fn category(name: &str) -> Result<ToolCategory, StoreError> {
    ToolCategory::from_name(name)
        .ok_or_else(|| StoreError::Corrupt(format!("tool category `{name}`")))
}

fn count(value: i64, what: &'static str) -> Result<u64, StoreError> {
    Ok(count_from(Some(value), what)?.unwrap_or(0))
}

fn tool_uses(connection: &Connection, query: &RangeQuery) -> Result<Vec<ToolUse>, StoreError> {
    let (clauses, values) = clauses(query);
    let mut statement = connection.prepare_cached(&format!(
        "SELECT tool_name, mcp_server, category, SUM(calls), SUM(failed), SUM(with_outcome), \
         SUM(in_subagents) FROM rollup_15m_tools WHERE {clauses} \
         GROUP BY tool_name, mcp_server ORDER BY tool_name, mcp_server"
    ))?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            [
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ],
        ))
    })?;
    rows.map(|row| {
        let (tool, mcp_server, name, [calls, failed, with_outcome, in_subagents]) = row?;
        Ok(ToolUse {
            tool,
            mcp_server,
            category: category(&name)?,
            counts: ToolCounts {
                calls: count(calls, "tool calls")?,
                failed: count(failed, "failed tool calls")?,
                with_outcome: count(with_outcome, "tool outcomes")?,
                in_subagents: count(in_subagents, "subagent tool calls")?,
            },
        })
    })
    .collect()
}

fn category_calls(
    connection: &Connection,
    query: &RangeQuery,
) -> Result<Vec<CategoryCalls>, StoreError> {
    let (clauses, values) = clauses(query);
    let mut statement = connection.prepare_cached(&format!(
        "SELECT bucket_start, category, SUM(calls) FROM rollup_15m_tools WHERE {clauses} \
         GROUP BY bucket_start, category ORDER BY bucket_start, category"
    ))?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (bucket_start, name, calls) = row?;
        Ok(CategoryCalls {
            bucket_start: UtcMillis(bucket_start),
            category: category(&name)?,
            calls: count(calls, "tool calls")?,
        })
    })
    .collect()
}

impl ToolReader for Store {
    fn tool_uses(&self, query: &RangeQuery) -> Result<Vec<ToolUse>, ReadError> {
        self.with_reader(|connection| tool_uses(connection, query))
            .map_err(|e| ReadError(e.to_string()))
    }

    fn category_calls(&self, query: &RangeQuery) -> Result<Vec<CategoryCalls>, ReadError> {
        self.with_reader(|connection| category_calls(connection, query))
            .map_err(|e| ReadError(e.to_string()))
    }
}
