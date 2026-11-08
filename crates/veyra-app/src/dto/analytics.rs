use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What every analytics view is about: a range of local days, an optional comparison
/// and filters. The dashboard keeps this in the URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    /// IANA time zone for local days and hours, e.g. `Asia/Kolkata`.
    pub time_zone: String,
    /// First local day, inclusive (`YYYY-MM-DD`).
    pub start: String,
    /// Last local day, inclusive (`YYYY-MM-DD`).
    pub end: String,
    pub comparison: Comparison,
    pub filters: ScopeFilters,
}

/// The period compared against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Comparison {
    None,
    /// The same number of days immediately before.
    Previous,
}

/// Allowed values per dimension; an empty list means unrestricted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScopeFilters {
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub apps: Vec<String>,
    pub interfaces: Vec<String>,
    /// Project ids, as listed in rankings.
    pub projects: Vec<String>,
}

/// Accuracy class of a value (PRD §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AccuracyClass {
    Exact,
    ProviderReported,
    Partial,
    Estimated,
    Unsupported,
}

/// Additive measures of a group. Money is API-equivalent nano-USD (an estimate).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Measures {
    #[ts(type = "number")]
    pub tokens: u64,
    #[ts(type = "number")]
    pub cost_nano_usd: i64,
    #[ts(type = "number")]
    pub requests: u64,
}

/// A headline number with its comparison. `null` means not reported, never zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Kpi {
    pub value: Option<f64>,
    pub previous: Option<f64>,
    /// Relative change; `null` without a comparison or when the previous value is zero.
    pub change: Option<f64>,
    /// The weakest accuracy class contributing; `null` for derived measures.
    pub accuracy: Option<AccuracyClass>,
    /// What the number means, from the metric registry (the single source of definitions).
    pub definition: String,
}

/// The Overview headline tiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OverviewKpis {
    pub tokens: Kpi,
    /// API-equivalent estimate in nano-USD.
    pub cost_nano_usd: Kpi,
    pub requests: Kpi,
    pub sessions: Kpi,
    /// Derived: union of request gaps capped at the idle threshold (5 minutes).
    pub active_ms: Kpi,
    pub cache_hit_ratio: Kpi,
}

/// Share of tokens per accuracy class (each 0–1; all zero without tokens).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Composition {
    pub exact: f64,
    pub provider_reported: f64,
    pub partial: f64,
    pub estimated: f64,
    pub unsupported: f64,
    pub weakest: Option<AccuracyClass>,
    /// Share of tokens on priced events; `null` without tokens.
    pub pricing_coverage: Option<f64>,
}

/// One local day of the stacked trend; `values` align with [`Overview::apps`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DailyPoint {
    pub date: String,
    pub values: Vec<Measures>,
}

/// A dimension value and its display name (e.g. `claude-code`, "Claude Code").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Labelled {
    pub key: String,
    pub label: String,
}

/// One row of a ranking. The "Other" row merges every group below the top N exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RankRow {
    /// Dimension value used for filtering; `null` for unknown values and for "Other".
    pub key: Option<String>,
    pub label: String,
    /// Provider of a model row, for its identity colour.
    pub provider: Option<String>,
    pub measures: Measures,
    /// Share of the scope's tokens (0–1).
    pub share: f64,
    /// Groups merged into this row (1 for a single group).
    #[ts(type = "number")]
    pub group_count: u64,
    pub is_other: bool,
}

/// Tokens on one local day of the activity calendar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDay {
    pub date: String,
    #[ts(type = "number")]
    pub tokens: u64,
}

/// Tokens in one local hour of one weekday, summed over the period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HourCell {
    /// 0 = Monday … 6 = Sunday.
    pub weekday: u8,
    pub hour: u8,
    #[ts(type = "number")]
    pub tokens: u64,
}

/// Request for [`crate::Api::get_overview`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OverviewRequest {
    pub scope: Scope,
}

/// Everything the Overview page shows for a scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub kpis: OverviewKpis,
    pub composition: Composition,
    /// Applications in the stacked trend, largest first.
    pub apps: Vec<Labelled>,
    /// Providers with usage in the scope, by name.
    pub providers: Vec<Labelled>,
    pub daily: Vec<DailyPoint>,
    /// Top models by tokens, plus "Other".
    pub models: Vec<RankRow>,
    /// Top projects by tokens, plus "Other".
    pub projects: Vec<RankRow>,
    /// The 365 local days ending on the scope's last day (filters apply).
    pub calendar: Vec<CalendarDay>,
    /// 7 × 24 cells in the scope's time zone.
    pub hours: Vec<HourCell>,
}
