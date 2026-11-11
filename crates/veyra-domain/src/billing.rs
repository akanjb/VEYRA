//! What the user pays, as they record it: billing entries and budgets (FR-PRICE-08,
//! FR-PRICE-09, ADR-028).
//!
//! A billing entry is a subscription or a charge, once or recurring. Subscriptions are
//! subscription spend and charges are actual charges: two of the three cost kinds that
//! are never added together (ADR-004). A budget is a monthly limit on API-equivalent cost.

use crate::source::PortError;
use crate::time::CivilDate;

/// Which cost kind an entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A plan paid for access (subscription spend).
    Subscription,
    /// An invoice or other payment for usage (actual charge).
    Charge,
}

/// How often an entry repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recurrence {
    /// Paid once, on its start date.
    Once,
    /// Every month from its start date (the same day of the month, or the month's last).
    Monthly,
    /// Every year from its start date.
    Yearly,
}

/// A billing entry as the user describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingEntryDraft {
    pub kind: EntryKind,
    pub label: String,
    /// The provider billed, when the user names one (a registry provider id).
    pub provider: Option<String>,
    /// Billionths of `currency` per occurrence.
    pub amount_nano: i64,
    /// ISO 4217 code, e.g. `USD`.
    pub currency: String,
    pub recurrence: Recurrence,
    /// The date of a one-off entry, or the first day of a recurring one.
    pub starts_on: CivilDate,
    /// The last day a recurring entry applies; `None` while it continues.
    pub ends_on: Option<CivilDate>,
}

/// A stored billing entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingEntry {
    pub id: i64,
    pub entry: BillingEntryDraft,
}

/// A monthly budget on API-equivalent cost, for all usage or for one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDraft {
    pub label: String,
    /// The monthly limit in nano-USD.
    pub limit_nano_usd: i64,
    /// The share of the limit (in percent) at which the budget warns.
    pub warn_percent: u8,
    /// The project the budget covers; `None` for all usage.
    pub project_id: Option<i64>,
}

/// A stored budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    pub id: i64,
    pub budget: BudgetDraft,
}

/// The longest label an entry or budget may have, in characters.
pub const MAX_LABEL_CHARS: usize = 80;

/// The warning threshold of a new budget, in percent of its limit.
pub const DEFAULT_WARN_PERCENT: u8 = 80;

/// Why a billing entry or budget was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BillingError {
    #[error("the label must have 1 to {MAX_LABEL_CHARS} characters")]
    Label,
    #[error("the amount must be greater than zero")]
    Amount,
    #[error("`{0}` is not a three-letter currency code")]
    Currency(String),
    #[error("a one-off entry has no end date")]
    EndOfOneOff,
    #[error("the end date is before the start date")]
    EndBeforeStart,
    #[error("the warning threshold must be between 1% and 100%")]
    WarnPercent,
    #[error("billing entry {0} does not exist")]
    EntryNotFound(i64),
    #[error("budget {0} does not exist")]
    BudgetNotFound(i64),
    #[error("project {0} does not exist")]
    ProjectNotFound(i64),
}

fn label(value: &str) -> Result<String, BillingError> {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    if (1..=MAX_LABEL_CHARS).contains(&length) {
        Ok(trimmed.to_owned())
    } else {
        Err(BillingError::Label)
    }
}

impl BillingEntryDraft {
    /// The entry as stored: label trimmed, currency upper-cased, every rule checked.
    ///
    /// # Errors
    /// The first [`BillingError`] the entry breaks.
    pub fn validated(self) -> Result<Self, BillingError> {
        let currency = self.currency.trim().to_ascii_uppercase();
        if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(BillingError::Currency(self.currency));
        }
        if self.amount_nano <= 0 {
            return Err(BillingError::Amount);
        }
        match (self.recurrence, self.ends_on) {
            (Recurrence::Once, Some(_)) => return Err(BillingError::EndOfOneOff),
            (_, Some(end)) if end < self.starts_on => return Err(BillingError::EndBeforeStart),
            _ => {}
        }
        Ok(Self {
            label: label(&self.label)?,
            provider: self
                .provider
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty()),
            currency,
            ..self
        })
    }
}

impl BudgetDraft {
    /// The budget as stored: label trimmed, every rule checked.
    ///
    /// # Errors
    /// The first [`BillingError`] the budget breaks.
    pub fn validated(self) -> Result<Self, BillingError> {
        if self.limit_nano_usd <= 0 {
            return Err(BillingError::Amount);
        }
        if !(1..=100).contains(&self.warn_percent) {
            return Err(BillingError::WarnPercent);
        }
        Ok(Self {
            label: label(&self.label)?,
            ..self
        })
    }
}

/// Reads and changes billing entries and budgets (implemented by the data layer).
///
/// Saving with `id: None` creates a record; with an id it replaces that record.
pub trait BillingRepository: Send + Sync {
    /// Every billing entry, by start date.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn billing_entries(&self) -> Result<Vec<BillingEntry>, PortError>;

    /// Creates or replaces a validated entry.
    ///
    /// # Errors
    /// [`BillingError::EntryNotFound`] inside [`PortError`] for an unknown id, or a
    /// persistence failure.
    fn save_billing_entry(
        &self,
        id: Option<i64>,
        entry: &BillingEntryDraft,
    ) -> Result<BillingEntry, PortError>;

    /// Deletes an entry.
    ///
    /// # Errors
    /// [`BillingError::EntryNotFound`] inside [`PortError`] for an unknown id, or a
    /// persistence failure.
    fn delete_billing_entry(&self, id: i64) -> Result<(), PortError>;

    /// Every budget, by label.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn budgets(&self) -> Result<Vec<Budget>, PortError>;

    /// Creates or replaces a validated budget.
    ///
    /// # Errors
    /// [`BillingError::BudgetNotFound`] or [`BillingError::ProjectNotFound`] inside
    /// [`PortError`], or a persistence failure.
    fn save_budget(&self, id: Option<i64>, budget: &BudgetDraft) -> Result<Budget, PortError>;

    /// Deletes a budget.
    ///
    /// # Errors
    /// [`BillingError::BudgetNotFound`] inside [`PortError`] for an unknown id, or a
    /// persistence failure.
    fn delete_budget(&self, id: i64) -> Result<(), PortError>;
}

#[cfg(test)]
mod tests;
