use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{Labelled, Scope};

/// Request for [`crate::Api::get_what_if`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WhatIfRequest {
    pub scope: Scope,
    /// The model to price the period on; `None` lists the choices only.
    pub model: Option<String>,
}

/// The models a period can be priced on, and the period priced on the one asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WhatIf {
    /// Every model with a current published price, by provider then name.
    pub choices: Vec<WhatIfChoice>,
    /// `None` when no model was asked for.
    pub comparison: Option<WhatIfComparison>,
    pub definition: String,
}

/// A model the period can be priced on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WhatIfChoice {
    pub model: Labelled,
    pub provider: Labelled,
}

/// The period's API-equivalent cost beside the same tokens on another model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WhatIfComparison {
    pub target: WhatIfChoice,
    #[ts(type = "number")]
    pub current_nano_usd: i64,
    #[ts(type = "number")]
    pub repriced_nano_usd: i64,
    /// The relative change from the recorded cost; `None` when that cost is zero.
    pub change: Option<f64>,
    /// Tokens compared on both sides.
    #[ts(type = "number")]
    pub tokens_compared: u64,
    /// Tokens of models with unpriced requests, left out of both sides.
    #[ts(type = "number")]
    pub tokens_left_out: u64,
    /// Each compared model, largest recorded cost first.
    pub rows: Vec<WhatIfRow>,
    /// What had to be assumed to price the tokens on the target, in words.
    pub assumptions: Vec<String>,
}

/// One model's recorded cost and its cost on the target model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WhatIfRow {
    /// The model id; `None` when the source did not name one.
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    #[ts(type = "number")]
    pub tokens: u64,
    #[ts(type = "number")]
    pub current_nano_usd: i64,
    #[ts(type = "number")]
    pub repriced_nano_usd: i64,
    /// The relative change from the recorded cost; `None` when that cost is zero.
    pub change: Option<f64>,
}
