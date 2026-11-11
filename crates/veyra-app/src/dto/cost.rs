//! The Cost page. Three cost kinds are kept in separate, named fields and are never
//! added together (ADR-004, AC-5): `apiEquivalent…` (an estimate from published prices),
//! `actualCharge…` and `subscriptionSpend…` (both recorded by the user). Every other money
//! field is API-equivalent unless its name says otherwise.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{AccuracyClass, Kpi, Labelled, RankRow, Scope};
use super::sessions::SessionRow;
use super::usage::BreakdownDimension;

/// Request for [`crate::Api::get_cost`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostRequest {
    pub scope: Scope,
}

/// Everything the Cost page shows for a scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Cost {
    pub kpis: CostKpis,
    /// The weakest accuracy class among the scope's tokens.
    pub accuracy: Option<AccuracyClass>,
    /// Billing entries left out of the totals because they are not in US dollars.
    pub other_currency_entries: u32,
    /// One point per local day of the period.
    pub daily: Vec<CostDay>,
    /// The month containing the period's last day.
    pub month: MonthOutlook,
    pub budgets: Vec<BudgetStatus>,
    /// Cost per model, provider, application and project.
    pub breakdowns: Vec<CostBreakdown>,
    /// The most expensive sessions in the period.
    pub sessions: Vec<SessionRow>,
    /// Cost per token kind: all models first, then the most expensive models.
    pub composition: Vec<CostMix>,
    /// Effective cost per million output tokens, highest first.
    pub rates: Vec<RateRow>,
    /// Change from the previous period of the same length, by model and by project.
    pub waterfalls: Vec<Waterfall>,
    pub definitions: CostDefinitions,
}

/// The headline tiles. The three cost kinds are separate tiles, never a total.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostKpis {
    pub api_equivalent_nano_usd: Kpi,
    pub pricing_coverage: Kpi,
    pub actual_charge_nano_usd: Kpi,
    pub subscription_spend_nano_usd: Kpi,
    /// API-equivalent.
    pub cost_per_request_nano_usd: Kpi,
    /// API-equivalent.
    pub cost_per_million_output_nano_usd: Kpi,
}

/// One local day: API-equivalent cost, its running total, and the running total of the
/// previous period's day at the same position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostDay {
    pub date: String,
    #[ts(type = "number")]
    pub api_equivalent_nano_usd: i64,
    #[ts(type = "number")]
    pub cumulative_nano_usd: i64,
    #[ts(type = "number")]
    pub previous_cumulative_nano_usd: i64,
}

/// Month-to-date API-equivalent cost, the run rate and the forecast (a projection).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MonthOutlook {
    /// `YYYY-MM`.
    pub month: String,
    /// The last day counted as month to date (`YYYY-MM-DD`): today, or the month's last
    /// day for a past month.
    pub as_of: String,
    /// Every day of the month.
    pub days: Vec<OutlookDay>,
    #[ts(type = "number")]
    pub month_to_date_nano_usd: i64,
    #[ts(type = "number")]
    pub run_rate_nano_usd: i64,
    #[ts(type = "number")]
    pub forecast_nano_usd: i64,
    pub days_left: u32,
}

/// One day of the month: the running total up to `asOf`, the projection after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutlookDay {
    pub date: String,
    #[ts(type = "number | null")]
    pub cumulative_nano_usd: Option<i64>,
    #[ts(type = "number | null")]
    pub projected_nano_usd: Option<i64>,
}

/// Where a cost stands against a budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BudgetLevel {
    Within,
    Warning,
    Over,
}

/// A budget this month. Page filters do not apply: a budget covers all usage or its
/// project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetStatus {
    #[ts(type = "number")]
    pub id: i64,
    pub label: String,
    /// The project it covers (`key` is the project id); `null` for all usage.
    pub project: Option<Labelled>,
    #[ts(type = "number")]
    pub limit_nano_usd: i64,
    pub warn_percent: u8,
    #[ts(type = "number")]
    pub month_to_date_nano_usd: i64,
    #[ts(type = "number")]
    pub forecast_nano_usd: i64,
    pub level: BudgetLevel,
    pub forecast_level: BudgetLevel,
}

/// A ranking by API-equivalent cost; shares are of the scope's cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub dimension: BreakdownDimension,
    pub rows: Vec<RankRow>,
}

/// API-equivalent cost of one group by token kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostMix {
    /// Model id; `null` for all models and for unknown models.
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    #[ts(type = "number")]
    pub input_nano_usd: i64,
    #[ts(type = "number")]
    pub cache_read_nano_usd: i64,
    #[ts(type = "number")]
    pub cache_write_nano_usd: i64,
    #[ts(type = "number")]
    pub output_nano_usd: i64,
}

/// A model's effective API-equivalent cost per million output tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RateRow {
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    #[ts(type = "number")]
    pub cost_per_million_output_nano_usd: i64,
}

/// How API-equivalent cost changed from the previous period, by one dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Waterfall {
    pub dimension: BreakdownDimension,
    #[ts(type = "number")]
    pub previous_nano_usd: i64,
    #[ts(type = "number")]
    pub current_nano_usd: i64,
    /// The largest changes, largest first, then "Other" when groups were merged.
    pub steps: Vec<WaterfallStep>,
}

/// One group's change. The steps add up from the previous total to the current one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WaterfallStep {
    /// Dimension value; `null` for unknown values and for "Other".
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
    #[ts(type = "number")]
    pub change_nano_usd: i64,
    /// Groups merged into this step (1 for a single group).
    #[ts(type = "number")]
    pub group_count: u64,
    pub is_other: bool,
}

/// What the page's derived figures mean, from their single definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CostDefinitions {
    pub run_rate: String,
    pub forecast: String,
    pub budget: String,
    pub waterfall: String,
    pub subscriptions: String,
    pub charges: String,
}
