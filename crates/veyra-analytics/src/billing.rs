//! Billing entries spread over local days (ADR-028).
//!
//! A one-off entry counts in full on its date. A recurring entry accrues: each cycle's
//! amount is split evenly over the cycle's days, so a month or a year of days adds up to
//! exactly one payment, and a period of any length gets its share. Subscriptions and
//! charges stay separate cost kinds; entries in other currencies are not converted.

use jiff::civil::Date;
use jiff::{ToSpan, Unit};
use veyra_domain::billing::{BillingEntryDraft, EntryKind, Recurrence};
use veyra_domain::money::{ActualCharge, NanoUsd, SubscriptionSpend};
use veyra_domain::time::CivilDate;

/// The only currency added up; others are listed but not converted.
pub const TOTALLED_CURRENCY: &str = "USD";

/// What a subscription's spend in a period means.
pub const SUBSCRIPTION_DEFINITION: &str = "Subscriptions you recorded, spread evenly over the days each payment covers: a monthly plan counts a share of its price on every day of its month. Subscription spend is never added to API-equivalent cost or to charges.";

/// What the actual charges in a period mean.
pub const CHARGES_DEFINITION: &str = "Charges you recorded, such as invoices: a one-off charge counts on its date, a recurring one is spread evenly over the days each payment covers. Never added to API-equivalent cost or to subscription spend.";

/// Recorded billing per local day, one cost kind per field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyBilling {
    pub subscriptions: Vec<SubscriptionSpend>,
    pub charges: Vec<ActualCharge>,
    /// Entries left out because they are not in [`TOTALLED_CURRENCY`].
    pub other_currency_entries: usize,
}

impl DailyBilling {
    /// Subscription spend over all the days.
    #[must_use]
    pub fn subscription_total(&self) -> SubscriptionSpend {
        self.subscriptions.iter().copied().sum()
    }

    /// Actual charges over all the days.
    #[must_use]
    pub fn charge_total(&self) -> ActualCharge {
        self.charges.iter().copied().sum()
    }
}

/// A domain date as a calendar date.
///
/// # Panics
/// Never: every [`CivilDate`] is a valid Gregorian date within jiff's range.
#[must_use]
pub fn calendar_date(date: CivilDate) -> Date {
    Date::new(
        i16::try_from(date.year()).unwrap_or(i16::MAX),
        i8::try_from(date.month()).unwrap_or(1),
        i8::try_from(date.day()).unwrap_or(1),
    )
    .unwrap_or(Date::MAX)
}

/// Splits `amount` over `days` days exactly: the first `amount % days` days get one
/// nano-unit more.
fn split(amount: i64, days: i64, index: i64) -> i64 {
    amount / days + i64::from(index < amount % days)
}

/// The start of cycle `k` of a recurring entry (the anchor's day, or the month's last).
fn cycle_start(anchor: Date, recurrence: Recurrence, k: i64) -> Option<Date> {
    match recurrence {
        Recurrence::Once => (k == 0).then_some(anchor),
        Recurrence::Monthly => anchor.checked_add(k.months()).ok(),
        Recurrence::Yearly => anchor.checked_add(k.years()).ok(),
    }
}

/// The first cycle that can overlap a range starting at `first`.
fn first_cycle(anchor: Date, recurrence: Recurrence, first: Date) -> i64 {
    let unit = match recurrence {
        Recurrence::Once => return 0,
        Recurrence::Monthly => Unit::Month,
        Recurrence::Yearly => Unit::Year,
    };
    let elapsed = anchor.until((unit, first)).map_or(0, |span| match unit {
        Unit::Year => i64::from(span.get_years()),
        _ => i64::from(span.get_months()),
    });
    // One cycle earlier, so a cycle that started before `first` is not missed.
    (elapsed - 1).max(0)
}

/// Days from `a` to `b` (negative when `b` is earlier).
fn days_between(a: Date, b: Date) -> i64 {
    a.until((Unit::Day, b))
        .map_or(0, |span| i64::from(span.get_days()))
}

/// The entry's amount on each of `days` consecutive days starting at `first`.
#[must_use]
pub fn daily_amounts(entry: &BillingEntryDraft, first: Date, days: usize) -> Vec<i64> {
    let mut amounts = vec![0; days];
    let anchor = calendar_date(entry.starts_on);
    let last_day = entry.ends_on.map(calendar_date);
    let length = i64::try_from(days).unwrap_or(i64::MAX);
    let mut k = first_cycle(anchor, entry.recurrence, first);
    while let Some(start) = cycle_start(anchor, entry.recurrence, k) {
        if days_between(first, start) >= length || last_day.is_some_and(|end| start > end) {
            break;
        }
        let cycle_days = cycle_start(anchor, entry.recurrence, k + 1)
            .map_or(1, |next| days_between(start, next).max(1));
        for index in 0..cycle_days {
            let offset = days_between(first, start) + index;
            let past_end = last_day.is_some_and(|end| days_between(end, start) + index > 0);
            if past_end || offset >= length {
                break;
            }
            if let Ok(slot) = usize::try_from(offset) {
                amounts[slot] += split(entry.amount_nano, cycle_days, index);
            }
        }
        k += 1;
    }
    amounts
}

/// Subscriptions and charges per day over `days` days starting at `first`.
#[must_use]
pub fn daily_billing(entries: &[BillingEntryDraft], first: Date, days: usize) -> DailyBilling {
    let mut subscriptions = vec![0_i64; days];
    let mut charges = vec![0_i64; days];
    let mut other_currency_entries = 0;
    for entry in entries {
        if entry.currency != TOTALLED_CURRENCY {
            other_currency_entries += 1;
            continue;
        }
        let target = match entry.kind {
            EntryKind::Subscription => &mut subscriptions,
            EntryKind::Charge => &mut charges,
        };
        for (total, amount) in target.iter_mut().zip(daily_amounts(entry, first, days)) {
            *total += amount;
        }
    }
    DailyBilling {
        subscriptions: subscriptions
            .into_iter()
            .map(|n| SubscriptionSpend::new(NanoUsd(n)))
            .collect(),
        charges: charges
            .into_iter()
            .map(|n| ActualCharge::new(NanoUsd(n)))
            .collect(),
        other_currency_entries,
    }
}

#[cfg(test)]
mod tests;
