//! Billing entries and budgets on a real SQLite file: round trips, replacement,
//! deletion and refused changes.

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use veyra_domain::billing::{
    BillingEntryDraft, BillingError, BillingRepository, BudgetDraft, EntryKind, Recurrence,
};
use veyra_domain::project::ProjectRepository;
use veyra_domain::source::PortError;
use veyra_domain::time::{CivilDate, UtcMillis};

use support::{NOV_8, TempDb, bundled_catalog, event, priced, tokens};

fn subscription() -> BillingEntryDraft {
    BillingEntryDraft {
        kind: EntryKind::Subscription,
        label: "Team plan".to_owned(),
        provider: Some("anthropic".to_owned()),
        amount_nano: 20_000_000_000,
        currency: "USD".to_owned(),
        recurrence: Recurrence::Monthly,
        starts_on: CivilDate::parse("2026-11-01").unwrap(),
        ends_on: Some(CivilDate::parse("2027-10-31").unwrap()),
    }
}

fn budget(project_id: Option<i64>) -> BudgetDraft {
    BudgetDraft {
        label: "Monthly".to_owned(),
        limit_nano_usd: 50_000_000_000,
        warn_percent: 80,
        project_id,
    }
}

fn refusal(error: &PortError) -> Option<&BillingError> {
    error.downcast_ref::<BillingError>()
}

#[test]
fn entries_round_trip_and_can_be_replaced_and_deleted() {
    let db = TempDb::new();
    let version = db.store.data_version();
    let saved = db.store.save_billing_entry(None, &subscription()).unwrap();
    assert_eq!(saved.entry, subscription());
    assert!(db.store.data_version() > version, "views refresh");

    let charge = BillingEntryDraft {
        kind: EntryKind::Charge,
        recurrence: Recurrence::Once,
        ends_on: None,
        provider: None,
        ..subscription()
    };
    let replaced = db
        .store
        .save_billing_entry(Some(saved.id), &charge)
        .unwrap();
    assert_eq!(replaced.id, saved.id);
    assert_eq!(db.store.billing_entries().unwrap(), vec![replaced]);

    db.store.delete_billing_entry(saved.id).unwrap();
    assert_eq!(db.store.billing_entries().unwrap().len(), 0);
}

#[test]
fn entries_are_listed_by_start_date() {
    let db = TempDb::new();
    let later = db.store.save_billing_entry(None, &subscription()).unwrap();
    let earlier = BillingEntryDraft {
        starts_on: CivilDate::parse("2026-01-15").unwrap(),
        ..subscription()
    };
    let earlier = db.store.save_billing_entry(None, &earlier).unwrap();
    let ids: Vec<i64> = db
        .store
        .billing_entries()
        .unwrap()
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, [earlier.id, later.id]);
}

#[test]
fn unknown_entries_and_budgets_are_refused() {
    let db = TempDb::new();
    let error = db
        .store
        .save_billing_entry(Some(7), &subscription())
        .unwrap_err();
    assert_eq!(refusal(&error), Some(&BillingError::EntryNotFound(7)));
    let error = db.store.delete_billing_entry(7).unwrap_err();
    assert_eq!(refusal(&error), Some(&BillingError::EntryNotFound(7)));
    let error = db.store.save_budget(Some(3), &budget(None)).unwrap_err();
    assert_eq!(refusal(&error), Some(&BillingError::BudgetNotFound(3)));
    let error = db.store.delete_budget(3).unwrap_err();
    assert_eq!(refusal(&error), Some(&BillingError::BudgetNotFound(3)));
    let error = db.store.save_budget(None, &budget(Some(999))).unwrap_err();
    assert_eq!(refusal(&error), Some(&BillingError::ProjectNotFound(999)));
    assert_eq!(db.store.budgets().unwrap().len(), 0, "nothing was written");
}

#[test]
fn budgets_round_trip_for_all_usage_and_for_a_project() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    let recorded = priced(event("req-1", NOV_8, tokens(100, 10)), &catalog);
    db.store
        .write_events(&[recorded], UtcMillis(NOV_8 + 60_000))
        .unwrap();
    let project = db.store.list_projects().unwrap()[0].id;

    let overall = db.store.save_budget(None, &budget(None)).unwrap();
    let scoped = BudgetDraft {
        label: "Veyra".to_owned(),
        ..budget(Some(project))
    };
    let scoped = db.store.save_budget(None, &scoped).unwrap();
    assert_eq!(db.store.budgets().unwrap(), vec![overall.clone(), scoped]);

    let raised = BudgetDraft {
        limit_nano_usd: 80_000_000_000,
        ..budget(None)
    };
    let replaced = db.store.save_budget(Some(overall.id), &raised).unwrap();
    assert_eq!(replaced.budget, raised);
    db.store.delete_budget(overall.id).unwrap();
    assert_eq!(db.store.budgets().unwrap().len(), 1);
}
