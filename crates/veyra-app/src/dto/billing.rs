use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which cost kind a billing entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BillingKind {
    /// Subscription spend: a plan paid for access.
    Subscription,
    /// An actual charge: an invoice or other payment for usage.
    Charge,
}

/// How often a billing entry repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BillingRecurrence {
    Once,
    Monthly,
    Yearly,
}

/// A billing entry as the user enters it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BillingEntryInput {
    pub kind: BillingKind,
    pub label: String,
    /// A provider id from the registry, if the user names one.
    pub provider: Option<String>,
    /// Billionths of `currency` per occurrence.
    #[ts(type = "number")]
    pub amount_nano: i64,
    /// ISO 4217 code; only USD entries are added up.
    pub currency: String,
    pub recurrence: BillingRecurrence,
    /// `YYYY-MM-DD`: the date of a one-off entry, or the first day of a recurring one.
    pub starts_on: String,
    /// `YYYY-MM-DD`, inclusive; `null` while a recurring entry continues.
    pub ends_on: Option<String>,
}

/// A stored billing entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BillingEntryInfo {
    #[ts(type = "number")]
    pub id: i64,
    pub entry: BillingEntryInput,
}

/// A monthly budget on API-equivalent cost as the user enters it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetInput {
    pub label: String,
    #[ts(type = "number")]
    pub limit_nano_usd: i64,
    /// Percent of the limit at which the budget warns (1–100).
    pub warn_percent: u8,
    /// The project it covers; `null` for all usage.
    #[ts(type = "number | null")]
    pub project_id: Option<i64>,
}

/// A stored budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetInfo {
    #[ts(type = "number")]
    pub id: i64,
    pub budget: BudgetInput,
}

/// Request for [`crate::Api::get_billing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BillingRequest {}

/// Every billing entry and budget, for editing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BillingSettings {
    /// By start date.
    pub entries: Vec<BillingEntryInfo>,
    /// By label.
    pub budgets: Vec<BudgetInfo>,
    /// The warning threshold a new budget starts with.
    pub default_warn_percent: u8,
}

/// Request for [`crate::Api::save_billing_entry`]: a new entry, or a replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveBillingEntryRequest {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub entry: BillingEntryInput,
}

/// Request for [`crate::Api::delete_billing_entry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBillingEntryRequest {
    #[ts(type = "number")]
    pub id: i64,
}

/// Request for [`crate::Api::save_budget`]: a new budget, or a replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveBudgetRequest {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub budget: BudgetInput,
}

/// Request for [`crate::Api::delete_budget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBudgetRequest {
    #[ts(type = "number")]
    pub id: i64,
}

/// Acknowledges a deletion; views refresh through the data version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BillingChanged {}
