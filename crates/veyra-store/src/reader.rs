//! Serves analytics base sums from the 15-minute rollups.
//!
//! Each query reads the smallest rollup level that answers it exactly:
//! - no grouping and no filters → `rollup_15m_totals` (one row per bucket);
//! - one dimension, grouped and/or filtered → `rollup_15m_by_dimension`;
//! - anything else → `rollup_15m` (one row per bucket and dimension combination).
//!
//! All levels are clustered by time, so a date range is a sequential read.

use std::collections::BTreeSet;

use rusqlite::types::Value;
use rusqlite::{Row, params_from_iter};
use veyra_analytics::dimension::{Dimension, DimensionValue};
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader, BaseSumsRow, ReadError};
use veyra_analytics::sums::{BaseSums, CostSums};
use veyra_domain::money::NanoUsd;
use veyra_domain::time::UtcMillis;

use crate::sql::{
    ROLLUP_SUM_COLUMNS, count_from, dimension_info, rollup_sum_index, rollup_sums_select,
};
use crate::{Store, StoreError};

impl BaseSumsReader for Store {
    fn read_base_sums(&self, query: &BaseSumsQuery) -> Result<Vec<BaseSumsRow>, ReadError> {
        self.with_reader(|connection| {
            let plan = plan(query);
            let mut statement = connection.prepare_cached(&plan.sql)?;
            let rows = statement.query_map(params_from_iter(plan.values), |row| {
                Ok(map_row(row, query.by_bucket, &plan.integer_keys))
            })?;
            rows.map(|row| row.map_err(StoreError::from).and_then(|r| r))
                .collect()
        })
        .map_err(|e| ReadError(e.to_string()))
    }
}

/// Which rollup level answers a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Totals,
    ByDimension(Dimension),
    Full,
}

fn source(query: &BaseSumsQuery) -> Source {
    let filtered: Vec<Dimension> = query.filters.iter().map(|(d, _)| d).collect();
    match (query.group_by.as_slice(), filtered.as_slice()) {
        ([], []) => Source::Totals,
        ([grouped], []) => Source::ByDimension(*grouped),
        ([grouped], [filter]) if grouped == filter => Source::ByDimension(*grouped),
        ([], [filter]) => Source::ByDimension(*filter),
        _ => Source::Full,
    }
}

/// SQL, parameters, and for each grouping key whether it is read as an integer id.
struct Plan {
    sql: String,
    values: Vec<Value>,
    integer_keys: Vec<bool>,
}

fn plan(query: &BaseSumsQuery) -> Plan {
    let mut values = vec![Value::Integer(query.from.0), Value::Integer(query.until.0)];
    let mut clauses = vec![
        "bucket_start >= ?1".to_owned(),
        "bucket_start < ?2".to_owned(),
    ];
    let source = source(query);
    let (table, key_columns, integer_keys): (&str, Vec<&str>, Vec<bool>) = match source {
        Source::Totals => ("rollup_15m_totals", vec![], vec![]),
        Source::ByDimension(dimension) => {
            let (name, ..) = dimension_info(dimension);
            values.push(Value::Text(name.to_owned()));
            clauses.push(format!("dimension = ?{}", values.len()));
            let keys = if query.group_by.is_empty() {
                vec![]
            } else {
                vec!["value"]
            };
            let integers = vec![false; keys.len()];
            ("rollup_15m_by_dimension", keys, integers)
        }
        Source::Full => (
            "rollup_15m",
            query
                .group_by
                .iter()
                .map(|d| dimension_info(*d).1)
                .collect(),
            query
                .group_by
                .iter()
                .map(|d| dimension_info(*d).2)
                .collect(),
        ),
    };
    for (dimension, allowed) in query.filters.iter() {
        let column = match source {
            Source::ByDimension(_) => "value",
            _ => dimension_info(dimension).1,
        };
        // The by-dimension table stores every value as text.
        let integer = matches!(source, Source::Full) && dimension_info(dimension).2;
        if let Some(clause) = filter_clause(column, integer, allowed, &mut values) {
            clauses.push(clause);
        }
    }
    let filter = clauses.join(" AND ");

    if source == Source::Totals && query.by_bucket {
        // Already one row per bucket: read the columns as stored, no aggregation.
        let sql = format!(
            "SELECT bucket_start, {} FROM {table} WHERE {filter} ORDER BY bucket_start",
            ROLLUP_SUM_COLUMNS.join(", ")
        );
        return Plan {
            sql,
            values,
            integer_keys,
        };
    }
    let mut keys = Vec::new();
    if query.by_bucket {
        keys.push("bucket_start");
    }
    keys.extend(key_columns);
    let select_keys: String = keys.iter().flat_map(|k| [*k, ", "]).collect();
    let group = if keys.is_empty() {
        String::new()
    } else {
        format!(" GROUP BY {}", keys.join(", "))
    };
    let sql = format!(
        "SELECT {select_keys}{} FROM {table} WHERE {filter}{group} HAVING SUM(events) > 0",
        rollup_sums_select(),
    );
    Plan {
        sql,
        values,
        integer_keys,
    }
}

/// `(column IN (…) OR column IS NULL)` for the allowed values; binds into `values`.
pub(crate) fn filter_clause(
    column: &str,
    integer: bool,
    allowed: &BTreeSet<DimensionValue>,
    values: &mut Vec<Value>,
) -> Option<String> {
    let mut alternatives = Vec::new();
    let mut placeholders = Vec::new();
    for value in allowed {
        match value {
            None => alternatives.push(format!("{column} IS NULL")),
            Some(v) => {
                values.push(if integer {
                    v.parse::<i64>().map_or(Value::Null, Value::Integer)
                } else {
                    Value::Text(v.clone())
                });
                placeholders.push(format!("?{}", values.len()));
            }
        }
    }
    if !placeholders.is_empty() {
        alternatives.push(format!("{column} IN ({})", placeholders.join(", ")));
    }
    (!alternatives.is_empty()).then(|| format!("({})", alternatives.join(" OR ")))
}

fn map_row(
    row: &Row<'_>,
    by_bucket: bool,
    integer_keys: &[bool],
) -> Result<BaseSumsRow, StoreError> {
    let mut index = 0;
    let bucket_start = if by_bucket {
        index += 1;
        Some(UtcMillis(row.get(0)?))
    } else {
        None
    };
    let mut key: Vec<DimensionValue> = Vec::with_capacity(integer_keys.len());
    for integer in integer_keys {
        let value = if *integer {
            row.get::<_, Option<i64>>(index)?.map(|id| id.to_string())
        } else {
            row.get::<_, Option<String>>(index)?
        };
        key.push(value);
        index += 1;
    }
    Ok(BaseSumsRow {
        bucket_start,
        key,
        sums: sums_from(row, index)?,
    })
}

/// Reads the rollup sums, which start at column `start`.
pub(crate) fn sums_from(row: &Row<'_>, start: usize) -> Result<BaseSums, StoreError> {
    let at = |column: &str| start + rollup_sum_index(column);
    let opt = |column: &str| -> Result<Option<u64>, StoreError> {
        count_from(row.get::<_, Option<i64>>(at(column))?, column)
    };
    let req = |column: &str| -> Result<u64, StoreError> { Ok(opt(column)?.unwrap_or(0)) };
    let money = |column: &str| -> Result<Option<NanoUsd>, StoreError> {
        Ok(row.get::<_, Option<i64>>(at(column))?.map(NanoUsd))
    };

    let mut sums = BaseSums {
        events: req("events")?,
        requests: opt("requests")?,
        reported_cost: money("reported_cost_nusd")?,
        cost: CostSums {
            input: money("cost_input_nusd")?.unwrap_or_default(),
            cache_read: money("cost_cache_read_nusd")?.unwrap_or_default(),
            cache_write: money("cost_cache_write_nusd")?.unwrap_or_default(),
            output: money("cost_output_nusd")?.unwrap_or_default(),
            total: money("cost_total_nusd")?.unwrap_or_default(),
            cache_read_saving: money("cache_read_saving_nusd")?,
            cache_write_overhead: money("cache_write_overhead_nusd")?,
        },
        events_priced: req("events_priced")?,
        events_partial: req("events_partial")?,
        events_unpriced: req("events_unpriced")?,
        tokens_priced: req("tokens_priced")?,
        ..BaseSums::default()
    };
    let t = &mut sums.tokens;
    t.input_uncached = opt("input_uncached")?;
    t.cache_read = opt("cache_read")?;
    t.cache_write_short = opt("cache_write_short")?;
    t.cache_write_long = opt("cache_write_long")?;
    t.cache_write_unspecified = opt("cache_write_unspecified")?;
    t.output_total = opt("output_total")?;
    t.reasoning = opt("reasoning")?;
    t.tool_tokens = opt("tool_tokens")?;
    let classes = [
        "tokens_exact",
        "tokens_provider_reported",
        "tokens_partial",
        "tokens_estimated",
        "tokens_unsupported",
    ];
    for (slot, column) in sums.tokens_by_accuracy.iter_mut().zip(classes) {
        *slot = req(column)?;
    }
    Ok(sums)
}
