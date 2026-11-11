use jiff::civil::{Date, date};
use veyra_domain::billing::{BillingEntryDraft, EntryKind, Recurrence};
use veyra_domain::money::NanoUsd;
use veyra_domain::time::CivilDate;

use super::{daily_amounts, daily_billing};

const USD: i64 = 1_000_000_000;

fn civil(value: &str) -> CivilDate {
    CivilDate::parse(value).unwrap()
}

fn entry(recurrence: Recurrence, starts: &str, amount: i64) -> BillingEntryDraft {
    BillingEntryDraft {
        kind: EntryKind::Subscription,
        label: "Plan".to_owned(),
        provider: None,
        amount_nano: amount,
        currency: "USD".to_owned(),
        recurrence,
        starts_on: civil(starts),
        ends_on: None,
    }
}

fn days_of(first: Date, last: Date) -> usize {
    usize::try_from(first.until(last).unwrap().get_days() + 1).unwrap()
}

#[test]
fn a_one_off_entry_counts_on_its_date_only() {
    let one_off = entry(Recurrence::Once, "2026-11-03", 50 * USD);
    let amounts = daily_amounts(&one_off, date(2026, 11, 1), 5);
    assert_eq!(amounts, vec![0, 0, 50 * USD, 0, 0]);
    assert_eq!(daily_amounts(&one_off, date(2026, 11, 4), 5), vec![0; 5]);
}

#[test]
fn a_month_of_days_adds_up_to_one_payment() {
    let plan = entry(Recurrence::Monthly, "2026-11-01", 20 * USD);
    let november = daily_amounts(&plan, date(2026, 11, 1), 30);
    assert_eq!(november.iter().sum::<i64>(), 20 * USD);
    // Spread evenly: no day differs from another by more than one nano-dollar.
    let (low, high) = (
        november.iter().min().unwrap(),
        november.iter().max().unwrap(),
    );
    assert!(high - low <= 1);
}

#[test]
fn cycles_follow_the_start_day_and_short_months() {
    // Starting on the 31st: February's cycle starts on its last day.
    let plan = entry(Recurrence::Monthly, "2026-01-31", 28 * USD);
    let first = date(2026, 1, 31);
    let last = date(2026, 3, 30);
    let amounts = daily_amounts(&plan, first, days_of(first, last));
    // Cycles: Jan 31 – Feb 27 (28 days), Feb 28 – Mar 30 (31 days).
    let first_cycle: i64 = amounts[..28].iter().sum();
    let second_cycle: i64 = amounts[28..].iter().sum();
    assert_eq!(first_cycle, 28 * USD);
    assert_eq!(second_cycle, 28 * USD);
}

#[test]
fn any_split_of_a_range_adds_up_to_the_whole() {
    let plan = entry(Recurrence::Monthly, "2026-03-17", 7 * USD + 3);
    let first = date(2026, 4, 1);
    let whole: i64 = daily_amounts(&plan, first, 120).iter().sum();
    let parts: i64 = (0..4)
        .map(|i| {
            let start = first.checked_add(jiff::Span::new().days(i * 30)).unwrap();
            daily_amounts(&plan, start, 30).iter().sum::<i64>()
        })
        .sum();
    assert_eq!(whole, parts);
}

#[test]
fn a_year_of_days_adds_up_to_one_yearly_payment() {
    let plan = entry(Recurrence::Yearly, "2026-06-15", 240 * USD);
    let first = date(2026, 6, 15);
    let amounts = daily_amounts(&plan, first, 365);
    assert_eq!(amounts.iter().sum::<i64>(), 240 * USD);
}

#[test]
fn nothing_accrues_before_the_start_or_after_the_end() {
    let mut plan = entry(Recurrence::Monthly, "2026-11-10", 30 * USD);
    plan.ends_on = Some(civil("2026-11-19"));
    let amounts = daily_amounts(&plan, date(2026, 11, 1), 30);
    assert!(amounts[..9].iter().all(|a| *a == 0), "before the start");
    assert!(amounts[19..].iter().all(|a| *a == 0), "after the end");
    // Ten of the cycle's thirty days.
    assert_eq!(amounts.iter().sum::<i64>(), 10 * USD);
}

#[test]
fn kinds_stay_apart_and_other_currencies_are_not_added() {
    let plan = entry(Recurrence::Once, "2026-11-02", 10 * USD);
    let charge = BillingEntryDraft {
        kind: EntryKind::Charge,
        amount_nano: 4 * USD,
        ..plan.clone()
    };
    let euros = BillingEntryDraft {
        currency: "EUR".to_owned(),
        ..plan.clone()
    };
    let billing = daily_billing(&[plan, charge, euros], date(2026, 11, 1), 3);
    assert_eq!(billing.subscription_total().amount(), NanoUsd(10 * USD));
    assert_eq!(billing.charge_total().amount(), NanoUsd(4 * USD));
    assert_eq!(billing.other_currency_entries, 1);
    assert_eq!(billing.subscriptions[1].amount(), NanoUsd(10 * USD));
}
