//! Serves analytics base sums from the 15-minute rollups.
//!
//! Each query reads the smallest rollup level that answers it exactly:
//! - no grouping and no filters → `rollup_15m_totals` (one row per bucket);
//! - one dimension, grouped and/or filtered → `rollup_15m_by_dimension`;
//! - only provider, model, application and interface → `rollup_15m_services` (ADR-030);
//! - anything else → `rollup_15m` (one row per bucket and dimension combination).
//!
//! All levels are clustered by time, so a date range is a sequential read. A read split by
//! local period seeks each period's buckets in turn and sums them in SQL, so it returns one
//! row per period rather than one per 15-minute bucket.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::types::Value;
use rusqlite::{Row, params_from_iter};
use veyra_analytics::dimension::{Dimension, DimensionValue};
use veyra_analytics::port::{
    BaseSumsQuery, BaseSumsReader, BaseSumsRow, CostRow, ReadError, TokenRow, in_periods,
};
use veyra_analytics::sums::{BaseSums, CostSums};
use veyra_domain::money::NanoUsd;
use veyra_domain::time::UtcMillis;

use crate::sql::{
    ROLLUP_SUM_COLUMNS, count_from, dimension_info, rollup_sum_index, rollup_sums_select,
};
use crate::{Store, StoreError};

impl BaseSumsReader for Store {
    fn read_base_sums(&self, query: &BaseSumsQuery) -> Result<Vec<BaseSumsRow>, ReadError> {
        read(self, query, Split::of(query), Measure::Sums, sums_row)
    }

    fn read_base_sums_by_period(
        &self,
        query: &BaseSumsQuery,
        starts: &[UtcMillis],
    ) -> Result<Vec<BaseSumsRow>, ReadError> {
        if source(query) == Source::Totals {
            // Already one row per bucket: reading the rows as stored is cheaper than
            // summing every column in SQL.
            let rows = read(self, query, Split::Buckets, Measure::Sums, sums_row)?;
            return Ok(in_periods(rows, starts));
        }
        read(self, query, Split::Periods(starts), Measure::Sums, sums_row)
    }

    fn read_tokens(&self, query: &BaseSumsQuery) -> Result<Vec<TokenRow>, ReadError> {
        single_sums(self, query, "tokens_total")?
            .into_iter()
            .map(|((bucket_start, key), tokens)| {
                Ok(TokenRow {
                    bucket_start,
                    key,
                    tokens: count_from(Some(tokens), "tokens")
                        .map_err(|e| ReadError(e.to_string()))?
                        .unwrap_or(0),
                })
            })
            .collect()
    }

    fn read_costs(&self, query: &BaseSumsQuery) -> Result<Vec<CostRow>, ReadError> {
        Ok(single_sums(self, query, "cost_total_nusd")?
            .into_iter()
            .map(|((bucket_start, key), cost)| CostRow {
                bucket_start,
                key,
                cost: NanoUsd(cost),
            })
            .collect())
    }
}

/// A group of a single-sum read: its bucket (when split by bucket) and key.
type SingleKey = (Option<UtcMillis>, Vec<DimensionValue>);

/// One rollup column summed per group. The rows are read as stored and added up here
/// rather than by SQL's `GROUP BY`, which sorts every row: for one column over many rows
/// and few groups, the sort costs several times more than reading the rows.
fn single_sums(
    store: &Store,
    query: &BaseSumsQuery,
    column: &'static str,
) -> Result<BTreeMap<SingleKey, i64>, ReadError> {
    let mut sums: BTreeMap<SingleKey, i64> = BTreeMap::new();
    read(
        store,
        query,
        Split::of(query),
        Measure::Single(column),
        |row, key| {
            let value: i64 = row.get(key.next_column)?;
            let sum = sums.entry((key.bucket_start, key.values)).or_default();
            // Fails like SQL's SUM rather than wrapping.
            *sum = sum
                .checked_add(value)
                .ok_or(StoreError::OutOfRange(column))?;
            Ok(())
        },
    )?;
    Ok(sums)
}

fn sums_row(row: &Row<'_>, key: RowKey) -> Result<BaseSumsRow, StoreError> {
    Ok(BaseSumsRow {
        bucket_start: key.bucket_start,
        key: key.values,
        sums: sums_from(row, key.next_column)?,
    })
}

/// How a read splits its range in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Split<'a> {
    /// One row per group over the whole range.
    Whole,
    /// One row per group and 15-minute bucket.
    Buckets,
    /// One row per group and local period, each starting at one of these instants
    /// (ascending, bucket-aligned, the first at or before the range's start).
    Periods(&'a [UtcMillis]),
}

impl Split<'_> {
    const fn of(query: &BaseSumsQuery) -> Self {
        if query.by_bucket {
            Self::Buckets
        } else {
            Self::Whole
        }
    }

    /// Whether result rows start with the bucket or period they cover.
    fn in_time(self) -> bool {
        self != Self::Whole
    }
}

/// What a read selects for each group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    /// Every rollup sum, in [`ROLLUP_SUM_COLUMNS`] order.
    Sums,
    /// One rollup column of every stored row, summed per group by [`single_sums`].
    Single(&'static str),
}

impl Measure {
    /// The selected columns: summed per group, or as stored.
    fn select(self, aggregated: bool) -> String {
        match (self, aggregated) {
            (Self::Sums, true) => rollup_sums_select(),
            (Self::Sums, false) => ROLLUP_SUM_COLUMNS.join(", "),
            (Self::Single(column), _) => column.to_owned(),
        }
    }
}

/// The bucket and grouping key of a result row, and the column its measure starts at.
struct RowKey {
    bucket_start: Option<UtcMillis>,
    values: Vec<DimensionValue>,
    next_column: usize,
}

/// Runs a query split and planned for `measure`, mapping each row with its key.
fn read<T>(
    store: &Store,
    query: &BaseSumsQuery,
    split: Split<'_>,
    measure: Measure,
    mut map: impl FnMut(&Row<'_>, RowKey) -> Result<T, StoreError>,
) -> Result<Vec<T>, ReadError> {
    store
        .with_reader(|connection| {
            let plan = plan(query, split, measure);
            let mut statement = connection.prepare_cached(&plan.sql)?;
            let mut rows = statement.query(params_from_iter(plan.values))?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let key = row_key(row, split.in_time(), &plan.integer_keys)?;
                out.push(map(row, key)?);
            }
            Ok(out)
        })
        .map_err(|e| ReadError(e.to_string()))
}

/// Which rollup level answers a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Totals,
    ByDimension(Dimension),
    /// `rollup_15m_services`: keyed like `rollup_15m` by [`SERVICE_DIMENSIONS`] only.
    Services,
    Full,
}

/// The dimensions `rollup_15m_services` is keyed by.
const SERVICE_DIMENSIONS: [Dimension; 4] = [
    Dimension::Provider,
    Dimension::Model,
    Dimension::App,
    Dimension::Interface,
];

fn source(query: &BaseSumsQuery) -> Source {
    let filtered: Vec<Dimension> = query.filters.iter().map(|(d, _)| d).collect();
    match (query.group_by.as_slice(), filtered.as_slice()) {
        ([], []) => Source::Totals,
        ([grouped], []) => Source::ByDimension(*grouped),
        ([grouped], [filter]) if grouped == filter => Source::ByDimension(*grouped),
        ([], [filter]) => Source::ByDimension(*filter),
        (grouped, filtered)
            if grouped
                .iter()
                .chain(filtered)
                .all(|d| SERVICE_DIMENSIONS.contains(d)) =>
        {
            Source::Services
        }
        _ => Source::Full,
    }
}

/// SQL, parameters, and for each grouping key whether it is read as an integer id.
struct Plan {
    sql: String,
    values: Vec<Value>,
    integer_keys: Vec<bool>,
}

/// The periods of a [`Split::Periods`] read (`?1`, a JSON array of `[start, from, until]`
/// triples), each joined to its own key-range seek of the rollup over `[from, until)`.
const PERIODS: &str = "(SELECT value ->> 0 AS period_start, value ->> 1 AS period_from, \
     value ->> 2 AS period_until FROM json_each(?1)) AS periods";

/// `[[start, from, until], …]`: each period of `starts` with the part of it inside the
/// query's range; periods with none are left out. Rows are labelled with `start` even when
/// the range begins inside the period.
fn period_bounds(query: &BaseSumsQuery, starts: &[UtcMillis]) -> String {
    let ends = starts
        .iter()
        .skip(1)
        .map(|end| end.0)
        .chain([query.until.0]);
    let periods: Vec<String> = starts
        .iter()
        .zip(ends)
        .map(|(start, end)| (start.0, start.0.max(query.from.0), end.min(query.until.0)))
        .filter(|(_, from, until)| from < until)
        .map(|(start, from, until)| format!("[{start},{from},{until}]"))
        .collect();
    format!("[{}]", periods.join(","))
}

fn plan(query: &BaseSumsQuery, split: Split<'_>, measure: Measure) -> Plan {
    let (mut values, mut clauses) = if let Split::Periods(starts) = split {
        // The range is in the periods, so each period seeks only its own buckets.
        (vec![Value::Text(period_bounds(query, starts))], vec![])
    } else {
        (
            vec![Value::Integer(query.from.0), Value::Integer(query.until.0)],
            vec![
                "bucket_start >= ?1".to_owned(),
                "bucket_start < ?2".to_owned(),
            ],
        )
    };
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
        Source::Services | Source::Full => (
            if source == Source::Services {
                "rollup_15m_services"
            } else {
                "rollup_15m"
            },
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
        let integer =
            matches!(source, Source::Services | Source::Full) && dimension_info(dimension).2;
        if let Some(clause) = filter_clause(column, integer, allowed, &mut values) {
            clauses.push(clause);
        }
    }
    let filter = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    Plan {
        sql: statement(source, split, measure, table, key_columns, &filter),
        values,
        integer_keys,
    }
}

/// The SQL of a planned read from `table`, with its `WHERE` clause already built.
fn statement(
    source: Source,
    split: Split<'_>,
    measure: Measure,
    table: &str,
    key_columns: Vec<&str>,
    filter: &str,
) -> String {
    if source == Source::Totals && split == Split::Buckets {
        // Already one row per bucket: read the columns as stored, no aggregation.
        return format!(
            "SELECT bucket_start, {} FROM {table}{filter} ORDER BY bucket_start",
            measure.select(false)
        );
    }
    let mut keys = Vec::new();
    let from = match split {
        Split::Whole => table.to_owned(),
        Split::Buckets => {
            keys.push("bucket_start");
            table.to_owned()
        }
        Split::Periods(_) => {
            keys.push("periods.period_start");
            // CROSS JOIN keeps the periods as the outer loop.
            format!(
                "{PERIODS} CROSS JOIN {table} ON bucket_start >= periods.period_from \
                 AND bucket_start < periods.period_until"
            )
        }
    };
    keys.extend(key_columns);
    let select_keys: String = keys.iter().flat_map(|k| [*k, ", "]).collect();
    if let Measure::Single(_) = measure {
        // Every stored row, summed per group by the caller. Each stored row holds at least
        // one event, so every group it finds has events, as `HAVING` requires below.
        return format!(
            "SELECT {select_keys}{} FROM {from}{filter}",
            measure.select(false)
        );
    }
    let group = if keys.is_empty() {
        String::new()
    } else {
        format!(" GROUP BY {}", keys.join(", "))
    };
    format!(
        "SELECT {select_keys}{} FROM {from}{filter}{group} HAVING SUM(events) > 0",
        measure.select(true),
    )
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

/// Reads a row's bucket or period (when split in time) and grouping key, in plan order.
fn row_key(row: &Row<'_>, in_time: bool, integer_keys: &[bool]) -> Result<RowKey, StoreError> {
    let mut index = 0;
    let bucket_start = if in_time {
        index += 1;
        Some(UtcMillis(row.get(0)?))
    } else {
        None
    };
    let mut values: Vec<DimensionValue> = Vec::with_capacity(integer_keys.len());
    for integer in integer_keys {
        let value = if *integer {
            row.get::<_, Option<i64>>(index)?.map(|id| id.to_string())
        } else {
            row.get::<_, Option<String>>(index)?
        };
        values.push(value);
        index += 1;
    }
    Ok(RowKey {
        bucket_start,
        values,
        next_column: index,
    })
}

/// Reads the rollup sums, which start at column `start`.
pub(crate) fn sums_from(row: &Row<'_>, start: usize) -> Result<BaseSums, StoreError> {
    /// A rollup column's name and its position in the row, found at compile time.
    macro_rules! column {
        ($name:literal) => {
            (start + const { rollup_sum_index($name) }, $name)
        };
    }
    let opt = |(at, column): (usize, &str)| -> Result<Option<u64>, StoreError> {
        count_from(row.get::<_, Option<i64>>(at)?, column)
    };
    let req = |column: (usize, &str)| -> Result<u64, StoreError> { Ok(opt(column)?.unwrap_or(0)) };
    let money = |(at, _): (usize, &str)| -> Result<Option<NanoUsd>, StoreError> {
        Ok(row.get::<_, Option<i64>>(at)?.map(NanoUsd))
    };

    let mut sums = BaseSums {
        events: req(column!("events"))?,
        requests: opt(column!("requests"))?,
        reported_cost: money(column!("reported_cost_nusd"))?,
        cost: CostSums {
            input: money(column!("cost_input_nusd"))?.unwrap_or_default(),
            cache_read: money(column!("cost_cache_read_nusd"))?.unwrap_or_default(),
            cache_write: money(column!("cost_cache_write_nusd"))?.unwrap_or_default(),
            output: money(column!("cost_output_nusd"))?.unwrap_or_default(),
            total: money(column!("cost_total_nusd"))?.unwrap_or_default(),
            cache_read_saving: money(column!("cache_read_saving_nusd"))?,
            cache_write_overhead: money(column!("cache_write_overhead_nusd"))?,
        },
        events_priced: req(column!("events_priced"))?,
        events_partial: req(column!("events_partial"))?,
        events_unpriced: req(column!("events_unpriced"))?,
        tokens_priced: req(column!("tokens_priced"))?,
        ..BaseSums::default()
    };
    let t = &mut sums.tokens;
    t.input_uncached = opt(column!("input_uncached"))?;
    t.cache_read = opt(column!("cache_read"))?;
    t.cache_write_short = opt(column!("cache_write_short"))?;
    t.cache_write_long = opt(column!("cache_write_long"))?;
    t.cache_write_unspecified = opt(column!("cache_write_unspecified"))?;
    t.output_total = opt(column!("output_total"))?;
    t.reasoning = opt(column!("reasoning"))?;
    t.tool_tokens = opt(column!("tool_tokens"))?;
    let classes = [
        column!("tokens_exact"),
        column!("tokens_provider_reported"),
        column!("tokens_partial"),
        column!("tokens_estimated"),
        column!("tokens_unsupported"),
    ];
    for (slot, column) in sums.tokens_by_accuracy.iter_mut().zip(classes) {
        *slot = req(column)?;
    }
    Ok(sums)
}
