//! Billing entries and budgets: conversions between the contract and the domain, and the
//! editing use cases (ADR-028).

use veyra_domain::billing::{
    BillingEntry, BillingEntryDraft, BillingError, Budget, BudgetDraft, DEFAULT_WARN_PERCENT,
    EntryKind, Recurrence,
};
use veyra_domain::source::PortError;
use veyra_domain::time::CivilDate;

use super::internal;
use crate::DataServices;
use crate::dto::{
    BillingEntryInfo, BillingEntryInput, BillingKind, BillingRecurrence, BillingSettings,
    BudgetInfo, BudgetInput,
};
use crate::error::ApiError;

/// A refused or failed billing change as an API error.
pub(crate) fn billing_error(error: &PortError) -> ApiError {
    match error.downcast_ref::<BillingError>() {
        Some(
            BillingError::EntryNotFound(_)
            | BillingError::BudgetNotFound(_)
            | BillingError::ProjectNotFound(_),
        ) => ApiError::NotFound {
            message: error.to_string(),
        },
        Some(_) => ApiError::Validation {
            message: error.to_string(),
        },
        None => internal(error),
    }
}

fn validation(error: &dyn std::fmt::Display) -> ApiError {
    ApiError::Validation {
        message: error.to_string(),
    }
}

fn date(value: &str) -> Result<CivilDate, ApiError> {
    CivilDate::parse(value).map_err(|e| validation(&e))
}

const fn kind(kind: BillingKind) -> EntryKind {
    match kind {
        BillingKind::Subscription => EntryKind::Subscription,
        BillingKind::Charge => EntryKind::Charge,
    }
}

const fn recurrence(recurrence: BillingRecurrence) -> Recurrence {
    match recurrence {
        BillingRecurrence::Once => Recurrence::Once,
        BillingRecurrence::Monthly => Recurrence::Monthly,
        BillingRecurrence::Yearly => Recurrence::Yearly,
    }
}

/// An entry from the contract, validated.
fn entry_draft(input: BillingEntryInput) -> Result<BillingEntryDraft, ApiError> {
    BillingEntryDraft {
        kind: kind(input.kind),
        label: input.label,
        provider: input.provider,
        amount_nano: input.amount_nano,
        currency: input.currency,
        recurrence: recurrence(input.recurrence),
        starts_on: date(&input.starts_on)?,
        ends_on: input.ends_on.as_deref().map(date).transpose()?,
    }
    .validated()
    .map_err(|e| validation(&e))
}

/// A budget from the contract, validated.
fn budget_draft(input: BudgetInput) -> Result<BudgetDraft, ApiError> {
    BudgetDraft {
        label: input.label,
        limit_nano_usd: input.limit_nano_usd,
        warn_percent: input.warn_percent,
        project_id: input.project_id,
    }
    .validated()
    .map_err(|e| validation(&e))
}

fn entry_info(stored: BillingEntry) -> BillingEntryInfo {
    let entry = stored.entry;
    BillingEntryInfo {
        id: stored.id,
        entry: BillingEntryInput {
            kind: match entry.kind {
                EntryKind::Subscription => BillingKind::Subscription,
                EntryKind::Charge => BillingKind::Charge,
            },
            label: entry.label,
            provider: entry.provider,
            amount_nano: entry.amount_nano,
            currency: entry.currency,
            recurrence: match entry.recurrence {
                Recurrence::Once => BillingRecurrence::Once,
                Recurrence::Monthly => BillingRecurrence::Monthly,
                Recurrence::Yearly => BillingRecurrence::Yearly,
            },
            starts_on: entry.starts_on.to_string(),
            ends_on: entry.ends_on.map(|d| d.to_string()),
        },
    }
}

fn budget_info(stored: Budget) -> BudgetInfo {
    let budget = stored.budget;
    BudgetInfo {
        id: stored.id,
        budget: BudgetInput {
            label: budget.label,
            limit_nano_usd: budget.limit_nano_usd,
            warn_percent: budget.warn_percent,
            project_id: budget.project_id,
        },
    }
}

/// Every entry and budget.
pub(crate) fn settings(data: &DataServices) -> Result<BillingSettings, ApiError> {
    let entries = data.billing.billing_entries().map_err(|e| internal(&e))?;
    let budgets = data.billing.budgets().map_err(|e| internal(&e))?;
    Ok(BillingSettings {
        entries: entries.into_iter().map(entry_info).collect(),
        budgets: budgets.into_iter().map(budget_info).collect(),
        default_warn_percent: DEFAULT_WARN_PERCENT,
    })
}

/// Creates or replaces an entry.
pub(crate) fn save_entry(
    data: &DataServices,
    id: Option<i64>,
    input: BillingEntryInput,
) -> Result<BillingEntryInfo, ApiError> {
    let draft = entry_draft(input)?;
    data.billing
        .save_billing_entry(id, &draft)
        .map(entry_info)
        .map_err(|e| billing_error(&e))
}

/// Creates or replaces a budget.
pub(crate) fn save_budget(
    data: &DataServices,
    id: Option<i64>,
    input: BudgetInput,
) -> Result<BudgetInfo, ApiError> {
    let draft = budget_draft(input)?;
    data.billing
        .save_budget(id, &draft)
        .map(budget_info)
        .map_err(|e| billing_error(&e))
}
