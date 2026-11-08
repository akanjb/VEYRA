//! SQL fragments and value conversions shared by writes, rollups and reads.

use rusqlite::types::Value;

use crate::StoreError;

/// Every stored column of `usage_events` except `id` and `dedup_key`, in binding order.
pub const EVENT_COLUMNS: [&str; 35] = [
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

/// Position of a column within [`ROLLUP_SUM_COLUMNS`].
pub fn rollup_sum_index(column: &str) -> usize {
    ROLLUP_SUM_COLUMNS
        .iter()
        .position(|c| *c == column)
        .unwrap_or_else(|| unreachable!("unknown rollup column {column}"))
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
