//! 15-minute rollups (ADR-009), rebuilt from raw events for each touched bucket.
//!
//! Rebuilding instead of incrementally adjusting keeps rollups correct by construction:
//! a bucket's rows are always exactly the aggregation of its events. Three levels exist:
//! `rollup_15m` (per dimension combination), `rollup_15m_by_dimension` (per value of each
//! single dimension) and `rollup_15m_totals` (one row per bucket).
//!
//! Rebuilds are set-based: the batch's affected buckets go into a temporary table and
//! each level is rebuilt for all of them with one statement.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use rusqlite::Transaction;

use crate::StoreError;
use crate::sql::{DIMENSIONS, EVENT_TOTAL_TOKENS, ROLLUP_SUM_COLUMNS, rollup_sums_select};

/// The affected buckets of the current batch.
const AFFECTED: &str = "(SELECT bucket_start FROM temp.rollup_affected)";

/// Aggregates the affected buckets' events into dimension rows (numbered by `slot`).
static REBUILD_FULL_SQL: LazyLock<String> = LazyLock::new(|| {
    let total = EVENT_TOTAL_TOKENS;
    let by_accuracy = |class: &str| {
        format!("COALESCE(SUM(CASE WHEN e.acc_tokens = '{class}' THEN {total} ELSE 0 END), 0)")
    };
    format!(
        "INSERT INTO rollup_15m (bucket_start, slot, provider_id, model_id, app_id, interface, \
         gateway_id, project_id, device_id, {sums}) \
         SELECT e.bucket_start, ROW_NUMBER() OVER (PARTITION BY e.bucket_start), e.provider_id, \
         e.model_id, e.app_id, e.interface, e.gateway_id, e.project_id, e.device_id, COUNT(*), \
         SUM(e.request_count), SUM(e.input_uncached), SUM(e.cache_read), \
         SUM(e.cache_write_short), SUM(e.cache_write_long), SUM(e.cache_write_unspecified), \
         SUM(e.output_total), SUM(e.reasoning), SUM(e.tool_tokens), SUM(e.reported_cost_nusd), \
         COALESCE(SUM(c.input_nusd), 0), COALESCE(SUM(c.cache_read_nusd), 0), \
         COALESCE(SUM(c.cache_write_nusd), 0), COALESCE(SUM(c.output_nusd), 0), \
         COALESCE(SUM(c.total_nusd), 0), SUM(c.cache_read_saving_nusd), \
         SUM(c.cache_write_overhead_nusd), COALESCE(SUM(c.status = 'priced'), 0), \
         COALESCE(SUM(c.status = 'partial'), 0), COALESCE(SUM(c.status = 'unpriced-model'), 0), \
         COALESCE(SUM({total}), 0), \
         COALESCE(SUM(CASE WHEN c.status IN ('priced', 'partial') THEN {total} ELSE 0 END), 0), \
         {exact}, {provider}, {partial}, {estimated}, {unsupported} \
         FROM usage_events e LEFT JOIN event_costs c ON c.event_id = e.id \
         WHERE e.bucket_start IN {AFFECTED} \
         GROUP BY e.bucket_start, e.provider_id, e.model_id, e.app_id, e.interface, \
         e.gateway_id, e.project_id, e.device_id",
        sums = ROLLUP_SUM_COLUMNS.join(", "),
        exact = by_accuracy("exact"),
        provider = by_accuracy("provider-reported"),
        partial = by_accuracy("partial"),
        estimated = by_accuracy("estimated"),
        unsupported = by_accuracy("unsupported"),
    )
});

/// Aggregates the affected buckets' dimension rows into one totals row per bucket.
static REBUILD_TOTALS_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "INSERT INTO rollup_15m_totals (bucket_start, {}) \
         SELECT bucket_start, {} FROM rollup_15m WHERE bucket_start IN {AFFECTED} \
         GROUP BY bucket_start",
        ROLLUP_SUM_COLUMNS.join(", "),
        rollup_sums_select()
    )
});

/// Per dimension: aggregates the affected buckets' dimension rows by that value.
static REBUILD_BY_DIMENSION_SQL: LazyLock<Vec<String>> = LazyLock::new(|| {
    DIMENSIONS
        .iter()
        .map(|(_, name, column, _)| {
            format!(
                "INSERT INTO rollup_15m_by_dimension (dimension, bucket_start, slot, value, {}) \
                 SELECT '{name}', bucket_start, ROW_NUMBER() OVER (PARTITION BY bucket_start), \
                 CAST({column} AS TEXT), {} FROM rollup_15m WHERE bucket_start IN {AFFECTED} \
                 GROUP BY bucket_start, {column}",
                ROLLUP_SUM_COLUMNS.join(", "),
                rollup_sums_select()
            )
        })
        .collect()
});

/// Rebuilds every rollup level for the given buckets. Returns the number of buckets.
pub fn rebuild(
    transaction: &Transaction<'_>,
    buckets: &BTreeSet<i64>,
) -> Result<usize, StoreError> {
    if buckets.is_empty() {
        return Ok(0);
    }
    transaction.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS rollup_affected (bucket_start INTEGER PRIMARY KEY); \
         DELETE FROM temp.rollup_affected;",
    )?;
    let mut add = transaction.prepare_cached("INSERT INTO temp.rollup_affected VALUES (?1)")?;
    for bucket in buckets {
        add.execute([bucket])?;
    }

    for table in ["rollup_15m", "rollup_15m_totals"] {
        transaction
            .prepare_cached(&format!(
                "DELETE FROM {table} WHERE bucket_start IN {AFFECTED}"
            ))?
            .execute([])?;
    }
    // The by-dimension key starts with `dimension`: delete per dimension so each delete
    // is a key-range seek rather than a table scan.
    let mut delete_by_dimension = transaction.prepare_cached(&format!(
        "DELETE FROM rollup_15m_by_dimension WHERE dimension = ?1 AND bucket_start IN {AFFECTED}"
    ))?;
    for (_, name, ..) in &DIMENSIONS {
        delete_by_dimension.execute([name])?;
    }
    transaction.prepare_cached(&REBUILD_FULL_SQL)?.execute([])?;
    transaction
        .prepare_cached(&REBUILD_TOTALS_SQL)?
        .execute([])?;
    for sql in REBUILD_BY_DIMENSION_SQL.iter() {
        transaction.prepare_cached(sql)?.execute([])?;
    }
    Ok(buckets.len())
}
