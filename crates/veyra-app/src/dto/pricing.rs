//! The cost assumptions panel: how a scope's API-equivalent cost was estimated. Every
//! money field is API-equivalent; prices are in millionths of a US dollar per million
//! tokens, multipliers in basis points (10 000 is 1.0×).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{Labelled, Scope};

/// Request for [`crate::Api::get_cost_assumptions`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostAssumptionsRequest {
    pub scope: Scope,
}

/// How the scope's events were priced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostAssumptions {
    /// The version of the price catalogue in use.
    pub catalog_version: String,
    /// A price checked longer ago than this is stale.
    pub stale_after_days: u32,
    /// Events per pricing outcome; outcomes that did not occur are left out.
    pub statuses: Vec<StatusUse>,
    /// Events per assumption made; an event with several counts under each.
    pub assumptions: Vec<AssumptionUse>,
    /// The price rules applied, highest cost first.
    pub rules: Vec<RuleUse>,
    /// Models with no published price, most tokens first.
    pub unpriced: Vec<UnpricedModel>,
    pub definition: String,
}

/// Events, tokens and API-equivalent cost of a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PricingTally {
    #[ts(type = "number")]
    pub events: u64,
    #[ts(type = "number")]
    pub tokens: u64,
    #[ts(type = "number")]
    pub api_equivalent_nano_usd: i64,
}

/// The outcome of pricing an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum PricingStatus {
    /// Every reported component was priced with published rates.
    Priced,
    /// Priced, but something had to be assumed or left out.
    Partial,
    /// No published price covers the model.
    UnpricedModel,
    /// The event reports no tokens.
    NoUsage,
}

/// How many events had one pricing outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StatusUse {
    pub status: PricingStatus,
    pub tally: PricingTally,
}

/// Something the pricing engine had to assume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AssumptionKind {
    UnspecifiedCacheWriteAtShortRate,
    PromptSizeUnknownBaseTierUsed,
    ServiceTierRateMissing,
    InferenceGeoRateMissing,
    ComponentRateMissing,
    HistoryAssumed,
}

/// How many events one assumption was made for, and what it means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssumptionUse {
    pub kind: AssumptionKind,
    pub description: String,
    /// Whether the assumption makes the cost of its events inexact.
    pub makes_partial: bool,
    pub tally: PricingTally,
}

/// Published prices per million tokens; `None` where no rate is published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Rates {
    #[ts(type = "number")]
    pub input: u64,
    #[ts(type = "number")]
    pub output: u64,
    #[ts(type = "number | null")]
    pub cache_read: Option<u64>,
    #[ts(type = "number | null")]
    pub cache_write_short: Option<u64>,
    #[ts(type = "number | null")]
    pub cache_write_long: Option<u64>,
    #[ts(type = "number | null")]
    pub reasoning: Option<u64>,
}

/// Rates that apply when a prompt is longer than a number of tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PriceTier {
    #[ts(type = "number")]
    pub above_prompt_tokens: u64,
    pub rates: Rates,
}

/// What a price modifier applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ModifierKind {
    Batch,
    Flex,
    Priority,
    Fast,
    Ultrafast,
    InferenceGeoUs,
}

/// A published multiplier on the base rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PriceModifier {
    pub kind: ModifierKind,
    pub factor_bps: u32,
    /// A different multiplier for cache reads, where one is published.
    pub cache_read_factor_bps: Option<u32>,
}

/// A price rule applied in the scope, with what it priced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuleUse {
    pub id: String,
    pub provider: Labelled,
    /// The model id, or the prefix of the model ids, it covers.
    pub models: String,
    /// Whether `models` is a prefix of model ids rather than one model.
    pub is_prefix: bool,
    /// First and last UTC dates the rule is in effect; `None` when open-ended.
    pub effective_from: Option<String>,
    pub effective_until: Option<String>,
    pub base: Rates,
    pub tiers: Vec<PriceTier>,
    pub modifiers: Vec<PriceModifier>,
    pub source_url: String,
    /// `YYYY-MM-DD`: when the rates were last checked against the source.
    pub verified_at: String,
    pub is_stale: bool,
    /// Whether earlier requests are priced as if the rates had not changed.
    pub history_assumed: bool,
    pub tally: PricingTally,
}

/// A model with no published price; its tokens add nothing to the cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnpricedModel {
    pub provider: Labelled,
    /// The model as reported; `None` when the source did not name one.
    pub model: Option<Labelled>,
    pub tally: PricingTally,
}
