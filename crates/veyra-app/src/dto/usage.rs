use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{AccuracyClass, Kpi, RankRow, Scope};

/// Request for [`crate::Api::get_usage`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageRequest {
    pub scope: Scope,
}

/// Everything the Usage page shows for a scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub kpis: UsageKpis,
    /// The weakest accuracy class among the scope's tokens.
    pub accuracy: Option<AccuracyClass>,
    /// One point per local day.
    pub composition: Vec<TokenMixPoint>,
    /// Whether any source in the scope reports reasoning tokens.
    pub reasoning_reported: bool,
    /// One ranking per dimension, in [`BreakdownDimension`] order.
    pub breakdowns: Vec<Breakdown>,
    /// Tokens per request, per local day.
    pub per_request: Vec<SizePoint>,
    /// Tokens per session, per local day of the session's first request.
    pub per_session: Vec<SizePoint>,
    /// Requests per size bin over the whole period, smallest first.
    pub request_sizes: Vec<SizeBinCount>,
    /// What the request percentiles mean.
    pub request_definition: String,
    /// What the session statistics mean.
    pub session_definition: String,
    /// What the concentration index means.
    pub concentration_definition: String,
}

/// The Usage headline tiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageKpis {
    pub tokens: Kpi,
    pub input_tokens: Kpi,
    pub output_tokens: Kpi,
    pub reasoning_tokens: Kpi,
    pub requests: Kpi,
    pub tokens_per_request: Kpi,
}

/// One local day's tokens by kind. The four kinds add up to the total; reasoning is
/// part of output, shown separately where reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TokenMixPoint {
    pub date: String,
    #[ts(type = "number")]
    pub input_uncached: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    #[ts(type = "number")]
    pub cache_write: u64,
    #[ts(type = "number")]
    pub output: u64,
    /// `null` when no event of the day reports reasoning.
    #[ts(type = "number | null")]
    pub reasoning: Option<u64>,
}

/// The dimensions usage is broken down by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BreakdownDimension {
    Model,
    Provider,
    App,
    Interface,
    Project,
    Device,
}

/// A ranking by tokens with its concentration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Breakdown {
    pub dimension: BreakdownDimension,
    /// The largest groups, plus "Other".
    pub rows: Vec<RankRow>,
    /// The running share of tokens after each row (a Pareto line), aligned with `rows`.
    pub cumulative_share: Vec<f64>,
    /// Herfindahl index over every group (not only the rows shown); `null` without tokens.
    pub concentration: Option<f64>,
}

/// Mean, median and 90th percentile of sizes on one local day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SizePoint {
    pub date: String,
    /// Requests or sessions counted.
    #[ts(type = "number")]
    pub count: u64,
    pub mean: Option<f64>,
    pub p50: Option<f64>,
    pub p90: Option<f64>,
}

/// Requests whose size falls in `[lower, upper)` tokens; `upper` is `null` for the last,
/// open-ended bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SizeBinCount {
    #[ts(type = "number")]
    pub lower: u64,
    #[ts(type = "number | null")]
    pub upper: Option<u64>,
    #[ts(type = "number")]
    pub requests: u64,
}
