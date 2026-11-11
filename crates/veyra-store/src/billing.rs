//! Billing entries and budgets (ADR-028). Every change runs in one write transaction and
//! marks the data as changed (`Store::change`), so cached views refresh.

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use veyra_domain::billing::{
    BillingEntry, BillingEntryDraft, BillingError, BillingRepository, Budget, BudgetDraft,
    EntryKind, Recurrence,
};
use veyra_domain::source::PortError;
use veyra_domain::time::CivilDate;

use crate::{Store, StoreError};

const ENTRY_COLUMNS: &str =
    "id, kind, label, provider_id, amount_nano, currency, recurrence, starts_on, ends_on";
const BUDGET_COLUMNS: &str = "id, label, limit_nano_usd, warn_percent, project_id";

fn kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Subscription => "subscription",
        EntryKind::Charge => "charge",
    }
}

fn recurrence_name(recurrence: Recurrence) -> &'static str {
    match recurrence {
        Recurrence::Once => "once",
        Recurrence::Monthly => "monthly",
        Recurrence::Yearly => "yearly",
    }
}

/// A stored text the schema does not allow, read as a conversion failure.
fn invalid(column: usize, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        format!("unexpected value `{value}`").into(),
    )
}

fn stored_date(row: &Row<'_>, column: usize) -> rusqlite::Result<Option<CivilDate>> {
    row.get::<_, Option<String>>(column)?
        .map(|text| CivilDate::parse(&text).map_err(|_| invalid(column, &text)))
        .transpose()
}

fn entry_from(row: &Row<'_>) -> rusqlite::Result<BillingEntry> {
    let kind: String = row.get(1)?;
    let recurrence: String = row.get(6)?;
    Ok(BillingEntry {
        id: row.get(0)?,
        entry: BillingEntryDraft {
            kind: match kind.as_str() {
                "subscription" => EntryKind::Subscription,
                "charge" => EntryKind::Charge,
                _ => return Err(invalid(1, &kind)),
            },
            label: row.get(2)?,
            provider: row.get(3)?,
            amount_nano: row.get(4)?,
            currency: row.get(5)?,
            recurrence: match recurrence.as_str() {
                "once" => Recurrence::Once,
                "monthly" => Recurrence::Monthly,
                "yearly" => Recurrence::Yearly,
                _ => return Err(invalid(6, &recurrence)),
            },
            starts_on: stored_date(row, 7)?.ok_or_else(|| invalid(7, ""))?,
            ends_on: stored_date(row, 8)?,
        },
    })
}

fn budget_from(row: &Row<'_>) -> rusqlite::Result<Budget> {
    Ok(Budget {
        id: row.get(0)?,
        budget: BudgetDraft {
            label: row.get(1)?,
            limit_nano_usd: row.get(2)?,
            warn_percent: row.get(3)?,
            project_id: row.get(4)?,
        },
    })
}

fn entry(connection: &Connection, id: i64) -> Result<BillingEntry, PortError> {
    connection
        .query_row(
            &format!("SELECT {ENTRY_COLUMNS} FROM billing_entries WHERE id = ?1"),
            [id],
            entry_from,
        )
        .optional()
        .map_err(StoreError::from)?
        .ok_or_else(|| BillingError::EntryNotFound(id).into())
}

fn budget(connection: &Connection, id: i64) -> Result<Budget, PortError> {
    connection
        .query_row(
            &format!("SELECT {BUDGET_COLUMNS} FROM budgets WHERE id = ?1"),
            [id],
            budget_from,
        )
        .optional()
        .map_err(StoreError::from)?
        .ok_or_else(|| BillingError::BudgetNotFound(id).into())
}

fn project_exists(connection: &Connection, id: i64) -> Result<bool, StoreError> {
    Ok(connection
        .query_row("SELECT 1 FROM projects WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Inserts a row, or replaces row `id` after checking it exists; returns the row's id.
fn upsert(
    transaction: &Transaction<'_>,
    id: Option<i64>,
    exists: impl Fn(&Connection, i64) -> Result<(), PortError>,
    insert: &str,
    update: &str,
    values: &[&dyn rusqlite::ToSql],
) -> Result<i64, PortError> {
    if let Some(id) = id {
        exists(transaction, id)?;
        let mut all: Vec<&dyn rusqlite::ToSql> = values.to_vec();
        all.push(&id);
        transaction
            .execute(update, all.as_slice())
            .map_err(StoreError::from)?;
        Ok(id)
    } else {
        transaction
            .execute(insert, values)
            .map_err(StoreError::from)?;
        Ok(transaction.last_insert_rowid())
    }
}

impl BillingRepository for Store {
    fn billing_entries(&self) -> Result<Vec<BillingEntry>, PortError> {
        self.read_all(
            &format!("SELECT {ENTRY_COLUMNS} FROM billing_entries ORDER BY starts_on, id"),
            entry_from,
        )
    }

    fn save_billing_entry(
        &self,
        id: Option<i64>,
        draft: &BillingEntryDraft,
    ) -> Result<BillingEntry, PortError> {
        let starts_on = draft.starts_on.to_string();
        let ends_on = draft.ends_on.map(|d| d.to_string());
        self.change(|transaction| {
            let saved = upsert(
                transaction,
                id,
                |c, id| entry(c, id).map(|_| ()),
                "INSERT INTO billing_entries (kind, label, provider_id, amount_nano, currency, \
                 recurrence, starts_on, ends_on) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                "UPDATE billing_entries SET kind = ?1, label = ?2, provider_id = ?3, \
                 amount_nano = ?4, currency = ?5, recurrence = ?6, starts_on = ?7, \
                 ends_on = ?8 WHERE id = ?9",
                params![
                    kind_name(draft.kind),
                    draft.label,
                    draft.provider,
                    draft.amount_nano,
                    draft.currency,
                    recurrence_name(draft.recurrence),
                    starts_on,
                    ends_on,
                ],
            )?;
            entry(transaction, saved)
        })
    }

    fn delete_billing_entry(&self, id: i64) -> Result<(), PortError> {
        self.change(|transaction| {
            entry(transaction, id)?;
            transaction
                .execute("DELETE FROM billing_entries WHERE id = ?1", [id])
                .map_err(StoreError::from)?;
            Ok(())
        })
    }

    fn budgets(&self) -> Result<Vec<Budget>, PortError> {
        self.read_all(
            &format!("SELECT {BUDGET_COLUMNS} FROM budgets ORDER BY label, id"),
            budget_from,
        )
    }

    fn save_budget(&self, id: Option<i64>, draft: &BudgetDraft) -> Result<Budget, PortError> {
        self.change(|transaction| {
            if let Some(project) = draft.project_id
                && !project_exists(transaction, project)?
            {
                return Err(BillingError::ProjectNotFound(project).into());
            }
            let saved = upsert(
                transaction,
                id,
                |c, id| budget(c, id).map(|_| ()),
                "INSERT INTO budgets (label, limit_nano_usd, warn_percent, project_id) \
                 VALUES (?1, ?2, ?3, ?4)",
                "UPDATE budgets SET label = ?1, limit_nano_usd = ?2, warn_percent = ?3, \
                 project_id = ?4 WHERE id = ?5",
                params![
                    draft.label,
                    draft.limit_nano_usd,
                    draft.warn_percent,
                    draft.project_id,
                ],
            )?;
            budget(transaction, saved)
        })
    }

    fn delete_budget(&self, id: i64) -> Result<(), PortError> {
        self.change(|transaction| {
            budget(transaction, id)?;
            transaction
                .execute("DELETE FROM budgets WHERE id = ?1", [id])
                .map_err(StoreError::from)?;
            Ok(())
        })
    }
}
