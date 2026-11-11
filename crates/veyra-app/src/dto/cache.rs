use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{AccuracyClass, Kpi, Labelled, Scope};

/// Request for [`crate::Api::get_cache`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheRequest {
    pub scope: Scope,
}

/// Everything the Cache page shows for a scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Cache {
    pub kpis: CacheKpis,
    /// The weakest accuracy class among the scope's tokens.
    pub accuracy: Option<AccuracyClass>,
    /// Share of tokens on events with published prices. Savings and overheads cover
    /// only these; `null` without tokens.
    pub pricing_coverage: Option<f64>,
    /// What the coverage means.
    pub coverage_definition: String,
    /// One point per local day.
    pub daily: Vec<CachePoint>,
    /// Hit ratio of the models with the most input tokens, plus "Other".
    pub by_model: Vec<RatioRow>,
    /// Hit ratio of the projects with the most input tokens, plus "Other".
    pub by_project: Vec<RatioRow>,
    /// Cache writes per TTL, and cache reads, of the models that write the most.
    pub writes_by_model: Vec<TtlRow>,
    /// Sessions whose cache writes cost more than their cache reads saved.
    pub inefficient: Vec<CacheSession>,
    /// What makes a session inefficient.
    pub inefficient_definition: String,
}

/// The Cache headline tiles; money is API-equivalent nano-USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheKpis {
    pub net_benefit_nano_usd: Kpi,
    pub gross_saving_nano_usd: Kpi,
    pub write_overhead_nano_usd: Kpi,
    pub hit_ratio: Kpi,
    pub reuse_ratio: Kpi,
}

/// Cache reads and writes on one local day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CachePoint {
    pub date: String,
    #[ts(type = "number")]
    pub reads: u64,
    #[ts(type = "number")]
    pub writes: u64,
}

/// A group's cache hit ratio and the input tokens behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RatioRow {
    /// Filter value; `null` for unknown values and for "Other".
    pub key: Option<String>,
    pub label: String,
    /// Provider of a model row, for its identity colour.
    pub provider: Option<String>,
    /// `null` when the group reported no cache reads or no input.
    pub hit_ratio: Option<f64>,
    #[ts(type = "number")]
    pub input_tokens: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    pub is_other: bool,
}

/// A model's cache writes per time-to-live and its cache reads. Sources do not say
/// which write a read came from, so reads are not split by TTL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TtlRow {
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    /// Written with a 5-minute lifetime.
    #[ts(type = "number")]
    pub short: u64,
    /// Written with a 1-hour lifetime.
    #[ts(type = "number")]
    pub long: u64,
    /// Written without a reported lifetime.
    #[ts(type = "number")]
    pub unspecified: u64,
    #[ts(type = "number")]
    pub reads: u64,
}

/// A session's cache use over the scope's period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheSession {
    #[ts(type = "number")]
    pub id: i64,
    pub app: String,
    pub project: Option<String>,
    pub models: Vec<Labelled>,
    #[ts(type = "number")]
    pub ended_at: i64,
    #[ts(type = "number")]
    pub cache_write: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    pub reuse_ratio: Option<f64>,
    /// Negative: the writes cost more than the reads saved.
    #[ts(type = "number")]
    pub net_benefit_nano_usd: i64,
}
