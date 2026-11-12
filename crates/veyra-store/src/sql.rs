//! SQL fragments and value conversions shared by writes, rollups and reads.

use std::sync::LazyLock;

use rusqlite::types::Value;

use crate::StoreError;

/// The additive sums of a group of events (alias `e`, costs `c`), in
/// [`ROLLUP_SUM_COLUMNS`] order. Shared by the rollup rebuild and the session reads, so a
/// session's totals are computed exactly like a bucket's.
pub static EVENT_SUMS_SELECT: LazyLock<String> = LazyLock::new(|| {
    let total = EVENT_TOTAL_TOKENS;
    let by_accuracy = |class: &str| {
        format!("COALESCE(SUM(CASE WHEN e.acc_tokens = '{class}' THEN {total} ELSE 0 END), 0)")
    };
    format!(
        "COUNT(*), SUM(e.request_count), SUM(e.input_uncached), SUM(e.cache_read), \
         SUM(e.cache_write_short), SUM(e.cache_write_long), SUM(e.cache_write_unspecified), \
         SUM(e.output_total), SUM(e.reasoning), SUM(e.tool_tokens), SUM(e.reported_cost_nusd), \
         COALESCE(SUM(c.input_nusd), 0), COALESCE(SUM(c.cache_read_nusd), 0), \
         COALESCE(SUM(c.cache_write_nusd), 0), COALESCE(SUM(c.output_nusd), 0), \
         COALESCE(SUM(c.total_nusd), 0), SUM(c.cache_read_saving_nusd), \
         SUM(c.cache_write_overhead_nusd), COALESCE(SUM(c.status = 'priced'), 0), \
         COALESCE(SUM(c.status = 'partial'), 0), COALESCE(SUM(c.status = 'unpriced-model'), 0), \
         COALESCE(SUM({total}), 0), \
         COALESCE(SUM(CASE WHEN c.status IN ('priced', 'partial') THEN {total} ELSE 0 END), 0), \
         {exact}, {provider}, {partial}, {estimated}, {unsupported}",
        exact = by_accuracy("exact"),
        provider = by_accuracy("provider-reported"),
        partial = by_accuracy("partial"),
        estimated = by_accuracy("estimated"),
        unsupported = by_accuracy("unsupported"),
    )
});

/// Every stored column of `usage_events` except `id` and `dedup_key`, in binding order.
pub const EVENT_COLUMNS: [&str; 37] = [
    "fingerprint",
    "occurred_at",
    "bucket_start",
    "connector_id",
    "parser_version",
    "tool_version",
    "session_id",
    "project_id",
    "device_id",
    "is_subagent",
    "provider_id",
    "model_id",
    "raw_model",
    "app_id",
    "interface",
    "gateway_id",
    "granularity",
    "service_tier",
    "inference_geo_us",
    "request_count",
    "input_uncached",
    "cache_read",
    "cache_write_short",
    "cache_write_long",
    "cache_write_unspecified",
    "output_total",
    "reasoning",
    "tool_tokens",
    "reported_cost_nusd",
    "acc_tokens",
    "acc_cache",
    "acc_reasoning",
    "acc_requests",
    "acc_cost",
    "ingested_at",
    "precedence",
    "source_project_id",
];

/// Each analytics dimension: its stable name in `rollup_15m_by_dimension`, its column in
/// `usage_events`/`rollup_15m`, and whether that column holds an integer id.
pub const DIMENSIONS: [(veyra_analytics::dimension::Dimension, &str, &str, bool); 7] = {
    use veyra_analytics::dimension::Dimension as D;
    [
        (D::Provider, "provider", "provider_id", false),
        (D::Model, "model", "model_id", false),
        (D::App, "app", "app_id", false),
        (D::Interface, "interface", "interface", false),
        (D::Gateway, "gateway", "gateway_id", false),
        (D::Project, "project", "project_id", true),
        (D::Device, "device", "device_id", true),
    ]
};

/// `(name, column, integer)` for a dimension.
pub fn dimension_info(
    dimension: veyra_analytics::dimension::Dimension,
) -> (&'static str, &'static str, bool) {
    DIMENSIONS
        .iter()
        .find(|(d, ..)| *d == dimension)
        .map_or_else(
            || unreachable!("every dimension is listed"),
            |(_, name, column, integer)| (*name, *column, *integer),
        )
}

/// Additive measures stored in every rollup table, in this order. Every analytics read
/// aggregates exactly these with `SUM`.
pub const ROLLUP_SUM_COLUMNS: [&str; 28] = [
    "events",
    "requests",
    "input_uncached",
    "cache_read",
    "cache_write_short",
    "cache_write_long",
    "cache_write_unspecified",
    "output_total",
    "reasoning",
    "tool_tokens",
    "reported_cost_nusd",
    "cost_input_nusd",
    "cost_cache_read_nusd",
    "cost_cache_write_nusd",
    "cost_output_nusd",
    "cost_total_nusd",
    "cache_read_saving_nusd",
    "cache_write_overhead_nusd",
    "events_priced",
    "events_partial",
    "events_unpriced",
    "tokens_total",
    "tokens_priced",
    "tokens_exact",
    "tokens_provider_reported",
    "tokens_partial",
    "tokens_estimated",
    "tokens_unsupported",
];

/// `SUM(c1), SUM(c2), …` over [`ROLLUP_SUM_COLUMNS`].
pub fn rollup_sums_select() -> String {
    ROLLUP_SUM_COLUMNS
        .iter()
        .map(|c| format!("SUM({c})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Position of a column within [`ROLLUP_SUM_COLUMNS`]. A `const fn`, so a read evaluates
/// it once at compile time rather than once per column of every row.
pub const fn rollup_sum_index(column: &str) -> usize {
    let mut index = 0;
    while index < ROLLUP_SUM_COLUMNS.len() {
        if same_bytes(ROLLUP_SUM_COLUMNS[index].as_bytes(), column.as_bytes()) {
            return index;
        }
        index += 1;
    }
    panic!("unknown rollup column");
}

/// Byte-wise equality usable in a `const fn`.
const fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Total tokens of an event (alias `e`): prompt components plus output, matching
/// `TokenCounts::total` (reasoning is inside output and not added again).
pub const EVENT_TOTAL_TOKENS: &str = "(COALESCE(e.input_uncached, 0) + COALESCE(e.cache_read, 0) \
     + COALESCE(e.cache_write_short, 0) + COALESCE(e.cache_write_long, 0) \
     + COALESCE(e.cache_write_unspecified, 0) + COALESCE(e.output_total, 0))";

pub fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

pub fn opt_text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, text)
}

pub fn opt_int(value: Option<i64>) -> Value {
    value.map_or(Value::Null, Value::Integer)
}

/// Converts an unsigned count for storage; SQLite integers are signed 64-bit.
pub fn opt_count(value: Option<u64>, what: &'static str) -> Result<Value, StoreError> {
    value.map_or(Ok(Value::Null), |v| {
        i64::try_from(v)
            .map(Value::Integer)
            .map_err(|_| StoreError::OutOfRange(what))
    })
}

/// Reads back a stored count, rejecting negative values as corruption.
pub fn count_from(value: Option<i64>, what: &str) -> Result<Option<u64>, StoreError> {
    value
        .map(|v| u64::try_from(v).map_err(|_| StoreError::Corrupt(format!("negative {what}"))))
        .transpose()
}
