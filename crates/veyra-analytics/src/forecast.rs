//! Month-to-date cost, the daily run rate, the month forecast and budget states
//! (FR-PRICE-09, PRD §7). Every figure is API-equivalent cost; a forecast is a projection.

use veyra_domain::money::{NanoUsd, div_round_half_even};

/// Complete days the run rate averages.
pub const RUN_RATE_DAYS: usize = 7;

/// What the run rate means.
pub const RUN_RATE_DEFINITION: &str = "API-equivalent cost over the last 7 complete days, divided by 7. Days without usage count as zero.";

/// What the month forecast means.
pub const FORECAST_DEFINITION: &str = "A projection, not a bill: month-to-date API-equivalent cost plus the daily run rate for each day left in the month.";

/// What a budget's state means.
pub const BUDGET_DEFINITION: &str = "A monthly limit on API-equivalent cost. It warns once month-to-date cost reaches its warning share, and is over once it reaches the limit. The forecast state applies the same thresholds to the month forecast.";

/// The average daily cost over the last [`RUN_RATE_DAYS`] of `complete_days` (oldest
/// first), rounded half to even. Fewer days count as days without usage.
#[must_use]
pub fn run_rate(complete_days: &[NanoUsd]) -> NanoUsd {
    let start = complete_days.len().saturating_sub(RUN_RATE_DAYS);
    let total: i128 = complete_days[start..].iter().map(|d| i128::from(d.0)).sum();
    let divisor = i128::try_from(RUN_RATE_DAYS).unwrap_or(1);
    let magnitude = div_round_half_even(total.unsigned_abs(), divisor.unsigned_abs());
    let value = i64::try_from(magnitude).unwrap_or(i64::MAX);
    NanoUsd(if total < 0 { -value } else { value })
}

/// Month-to-date cost plus the run rate for each of the `days_left` days.
#[must_use]
pub fn forecast(month_to_date: NanoUsd, run_rate: NanoUsd, days_left: u32) -> NanoUsd {
    NanoUsd(
        month_to_date
            .0
            .saturating_add(run_rate.0.saturating_mul(i64::from(days_left))),
    )
}

/// Running totals: each value plus every value before it.
#[must_use]
pub fn cumulative(values: &[NanoUsd]) -> Vec<NanoUsd> {
    values
        .iter()
        .scan(0_i64, |total, value| {
            *total = total.saturating_add(value.0);
            Some(NanoUsd(*total))
        })
        .collect()
}

/// The cumulative projection for each of the `days_left` days after today.
#[must_use]
pub fn projection(month_to_date: NanoUsd, run_rate: NanoUsd, days_left: u32) -> Vec<NanoUsd> {
    (1..=days_left)
        .map(|day| forecast(month_to_date, run_rate, day))
        .collect()
}

/// Where a cost stands against a budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BudgetState {
    /// Below the warning threshold.
    Within,
    /// At or above the warning threshold, below the limit.
    Warning,
    /// At or above the limit.
    Over,
}

/// The state of `cost` against a limit with a warning share in percent.
#[must_use]
pub fn budget_state(cost: NanoUsd, limit: NanoUsd, warn_percent: u8) -> BudgetState {
    let cost = i128::from(cost.0);
    let limit = i128::from(limit.0);
    if cost >= limit {
        BudgetState::Over
    } else if cost * 100 >= limit * i128::from(warn_percent) {
        BudgetState::Warning
    } else {
        BudgetState::Within
    }
}

#[cfg(test)]
mod tests {
    use veyra_domain::money::NanoUsd;

    use super::{BudgetState, budget_state, cumulative, forecast, projection, run_rate};

    fn usd(n: i64) -> NanoUsd {
        NanoUsd(n * 1_000_000_000)
    }

    #[test]
    fn the_run_rate_averages_the_last_seven_days() {
        let days: Vec<NanoUsd> = (1..=10).map(usd).collect();
        // Days 4 to 10: 49 dollars over 7 days.
        assert_eq!(run_rate(&days), usd(7));
    }

    #[test]
    fn missing_days_count_as_zero() {
        assert_eq!(run_rate(&[usd(14)]), usd(2));
        assert_eq!(run_rate(&[]), NanoUsd(0));
    }

    #[test]
    fn the_run_rate_rounds_half_to_even() {
        // 10.5 rounds to 10, 3.5 rounds to 4.
        assert_eq!(run_rate(&[NanoUsd(73)]), NanoUsd(10));
        assert_eq!(run_rate(&[NanoUsd(24), NanoUsd(1)]), NanoUsd(4));
    }

    #[test]
    fn the_forecast_adds_the_run_rate_for_each_day_left() {
        assert_eq!(forecast(usd(30), usd(2), 10), usd(50));
        assert_eq!(forecast(usd(30), usd(2), 0), usd(30));
        assert_eq!(
            projection(usd(30), usd(2), 3),
            vec![usd(32), usd(34), usd(36)]
        );
    }

    #[test]
    fn running_totals_add_every_earlier_day() {
        assert_eq!(
            cumulative(&[usd(1), usd(0), usd(3)]),
            vec![usd(1), usd(1), usd(4)]
        );
        assert_eq!(cumulative(&[]).len(), 0);
    }

    #[test]
    fn budget_states_use_both_thresholds() {
        let limit = usd(100);
        assert_eq!(budget_state(usd(79), limit, 80), BudgetState::Within);
        assert_eq!(budget_state(usd(80), limit, 80), BudgetState::Warning);
        assert_eq!(budget_state(usd(100), limit, 80), BudgetState::Over);
        assert_eq!(budget_state(usd(100), limit, 100), BudgetState::Over);
        assert_eq!(budget_state(usd(99), limit, 100), BudgetState::Within);
    }
}
