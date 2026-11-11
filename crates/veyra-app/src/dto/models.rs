use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{AccuracyClass, Labelled, Measures, Scope};

/// Request for [`crate::Api::get_models`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelsRequest {
    pub scope: Scope,
}

/// Everything the Models & Providers page shows for a scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Models {
    /// The weakest accuracy class among the scope's tokens.
    pub accuracy: Option<AccuracyClass>,
    /// Local days of the period, for the share trend and the timeline.
    pub dates: Vec<String>,
    /// Providers with usage, largest first.
    pub providers: Vec<Labelled>,
    /// Each provider's share of each day's tokens, aligned with `providers`.
    pub provider_share: Vec<ShareDay>,
    /// Every model with usage, largest first.
    pub leaderboard: Vec<ModelRow>,
    /// Applications with usage, largest first.
    pub apps: Vec<Labelled>,
    /// Tokens per provider (rows) and application (columns).
    pub provider_by_app: Matrix,
    /// Tokens per model (rows, the largest) and application (columns).
    pub model_by_app: Matrix,
    /// Daily tokens of the largest models, aligned with `dates`.
    pub timeline: Vec<ModelLane>,
    /// What the effective rate means.
    pub rate_definition: String,
}

/// The shares (0–1) of one local day; all zero on a day without tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShareDay {
    pub date: String,
    pub shares: Vec<f64>,
}

/// One model in the leaderboard. Money is API-equivalent nano-USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    /// The canonical model id, or the reported name of an unrecognised model.
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    pub provider_label: String,
    pub measures: Measures,
    /// API-equivalent cost per million output tokens; `null` when not computable.
    #[ts(type = "number | null")]
    pub cost_per_million_output_nano_usd: Option<i64>,
    pub cache_hit_ratio: Option<f64>,
    /// Share of the model's tokens with published prices.
    pub pricing_coverage: Option<f64>,
    /// First and last local day with usage in the period.
    pub first_day: String,
    pub last_day: String,
}

/// Tokens by two dimensions: `values[row][column]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Matrix {
    pub rows: Vec<Labelled>,
    pub columns: Vec<Labelled>,
    #[ts(type = "number[][]")]
    pub values: Vec<Vec<u64>>,
}

/// One model's tokens per local day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelLane {
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    #[ts(type = "number[]")]
    pub tokens: Vec<u64>,
}
