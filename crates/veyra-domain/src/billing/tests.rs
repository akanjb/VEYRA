use super::{BillingEntryDraft, BillingError, BudgetDraft, EntryKind, Recurrence};
use crate::time::CivilDate;

fn date(value: &str) -> CivilDate {
    CivilDate::parse(value).unwrap()
}

fn entry() -> BillingEntryDraft {
    BillingEntryDraft {
        kind: EntryKind::Subscription,
        label: "  Team plan ".to_owned(),
        provider: Some(" anthropic ".to_owned()),
        amount_nano: 20_000_000_000,
        currency: "usd".to_owned(),
        recurrence: Recurrence::Monthly,
        starts_on: date("2026-11-01"),
        ends_on: None,
    }
}

fn budget() -> BudgetDraft {
    BudgetDraft {
        label: "Monthly".to_owned(),
        limit_nano_usd: 100_000_000_000,
        warn_percent: 80,
        project_id: None,
    }
}

#[test]
fn entries_are_normalised() {
    let valid = entry().validated().unwrap();
    assert_eq!(valid.label, "Team plan");
    assert_eq!(valid.provider.as_deref(), Some("anthropic"));
    assert_eq!(valid.currency, "USD");
}

#[test]
fn a_blank_provider_is_no_provider() {
    let valid = BillingEntryDraft {
        provider: Some("  ".to_owned()),
        ..entry()
    }
    .validated()
    .unwrap();
    assert_eq!(valid.provider, None);
}

#[test]
fn invalid_entries_are_refused() {
    let cases = [
        (
            BillingEntryDraft {
                label: "   ".to_owned(),
                ..entry()
            },
            BillingError::Label,
        ),
        (
            BillingEntryDraft {
                label: "x".repeat(81),
                ..entry()
            },
            BillingError::Label,
        ),
        (
            BillingEntryDraft {
                amount_nano: 0,
                ..entry()
            },
            BillingError::Amount,
        ),
        (
            BillingEntryDraft {
                currency: "US$".to_owned(),
                ..entry()
            },
            BillingError::Currency("US$".to_owned()),
        ),
        (
            BillingEntryDraft {
                recurrence: Recurrence::Once,
                ends_on: Some(date("2026-12-01")),
                ..entry()
            },
            BillingError::EndOfOneOff,
        ),
        (
            BillingEntryDraft {
                ends_on: Some(date("2026-10-31")),
                ..entry()
            },
            BillingError::EndBeforeStart,
        ),
    ];
    for (draft, error) in cases {
        assert_eq!(draft.validated(), Err(error));
    }
}

#[test]
fn an_entry_may_end_on_its_start_day() {
    let draft = BillingEntryDraft {
        ends_on: Some(date("2026-11-01")),
        ..entry()
    };
    assert!(draft.validated().is_ok());
}

#[test]
fn budgets_are_checked() {
    assert_eq!(budget().validated().unwrap().label, "Monthly");
    for (draft, error) in [
        (
            BudgetDraft {
                limit_nano_usd: -1,
                ..budget()
            },
            BillingError::Amount,
        ),
        (
            BudgetDraft {
                warn_percent: 0,
                ..budget()
            },
            BillingError::WarnPercent,
        ),
        (
            BudgetDraft {
                warn_percent: 101,
                ..budget()
            },
            BillingError::WarnPercent,
        ),
        (
            BudgetDraft {
                label: String::new(),
                ..budget()
            },
            BillingError::Label,
        ),
    ] {
        assert_eq!(draft.validated(), Err(error));
    }
}
