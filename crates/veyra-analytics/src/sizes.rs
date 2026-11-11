//! Request and session sizes in tokens: size bins, distributions and percentiles.
//!
//! Request sizes are stored as additive request counts per size bin in the rollups
//! (ADR-026), so a distribution or a percentile costs no more than any other rollup read.
//! A bin spans a quarter of a doubling (its upper bound is about 1.19 times its lower
//! bound), so a percentile read from bins is within about ±9% of the exact value.
//! Session sizes are exact: they come from per-session totals.

use std::collections::BTreeMap;

use veyra_domain::time::UtcMillis;

use crate::engine::period_index;

/// Bins per doubling of the request size.
pub const STEPS_PER_DOUBLING: u32 = 4;

/// The last bin; requests of 2³² tokens or more fall into it as well.
pub const LAST_BIN: u32 = 32 * STEPS_PER_DOUBLING;

/// What the request-size percentiles mean (shown with every such figure).
pub const REQUEST_PERCENTILES_DEFINITION: &str = "Median (P50) and 90th percentile (P90) of the tokens per request, read from size bins a quarter of a doubling wide, so within about ±9% of the exact value. Only sources that record individual requests are included.";

/// What the session-size figures mean.
pub const SESSION_SIZES_DEFINITION: &str = "Mean, median (P50) and 90th percentile (P90) of the tokens per session, counting each session on the day of its first request in the period.";

/// The bin of a request of `tokens` tokens: 0 for no tokens, otherwise
/// `1 + ⌊4 · log₂ tokens⌋`, computed exactly with integers.
#[must_use]
pub fn size_bin(tokens: u64) -> u32 {
    if tokens == 0 {
        return 0;
    }
    let n = u128::from(tokens.min(u64::from(u32::MAX)));
    // n < 2³², so n⁴ < 2¹²⁸; ⌊log₂ n⁴⌋ = ⌊4 · log₂ n⌋.
    1 + (n * n * n * n).ilog2()
}

/// The smallest size in a bin.
#[must_use]
pub fn bin_lower(bin: u32) -> u64 {
    if bin == 0 {
        return 0;
    }
    let exponent = (bin - 1).min(LAST_BIN - 1);
    // The smallest n with n⁴ ≥ 2^exponent, i.e. ⌈2^(exponent / 4)⌉.
    let target = 1_u128 << exponent;
    let fourth = |n: u64| {
        let n = u128::from(n);
        n * n * n * n
    };
    // Start from the floating-point estimate and correct it exactly in integers.
    let mut n = estimate(exponent).max(1);
    while n > 1 && fourth(n - 1) >= target {
        n -= 1;
    }
    while fourth(n) < target {
        n += 1;
    }
    n
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a positive estimate below 2³², corrected exactly by the caller"
)]
fn estimate(exponent: u32) -> u64 {
    2_f64.powf(f64::from(exponent) / f64::from(STEPS_PER_DOUBLING)) as u64
}

/// The first size above a bin; `None` for the last bin, which is open-ended.
#[must_use]
pub fn bin_upper(bin: u32) -> Option<u64> {
    (bin < LAST_BIN).then(|| bin_lower(bin + 1))
}

/// Request counts per size bin over a 15-minute bucket (read from the rollups).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeRow {
    pub bucket_start: UtcMillis,
    pub bin: u32,
    pub requests: u64,
}

/// A session's tokens over the events considered, and when its first one happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSize {
    pub session: i64,
    pub first_at: UtcMillis,
    pub tokens: u64,
}

/// Request counts per size bin. Counts are additive, so histograms merge exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SizeHistogram {
    counts: BTreeMap<u32, u64>,
}

impl SizeHistogram {
    pub fn add(&mut self, bin: u32, requests: u64) {
        if requests > 0 {
            *self.counts.entry(bin.min(LAST_BIN)).or_default() += requests;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        for (bin, requests) in &other.counts {
            self.add(*bin, *requests);
        }
    }

    #[must_use]
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Non-empty bins in ascending order of size.
    pub fn bins(&self) -> impl Iterator<Item = (u32, u64)> + '_ {
        self.counts.iter().map(|(bin, requests)| (*bin, *requests))
    }

    /// The `p`-quantile (0–1) of request sizes, interpolated geometrically within its
    /// bin (sizes are spread evenly on a log scale); `None` without requests.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "an estimate for display")]
    pub fn quantile(&self, p: f64) -> Option<f64> {
        let total = self.total();
        if total == 0 {
            return None;
        }
        let rank = p.clamp(0.0, 1.0) * total as f64;
        let mut before = 0_u64;
        for (bin, requests) in self.bins() {
            let after = before + requests;
            if rank <= after as f64 || after == total {
                let fraction = ((rank - before as f64) / requests as f64).clamp(0.0, 1.0);
                return Some(within_bin(bin, fraction));
            }
            before = after;
        }
        None
    }
}

#[expect(clippy::cast_precision_loss, reason = "an estimate for display")]
fn within_bin(bin: u32, fraction: f64) -> f64 {
    let lower = bin_lower(bin) as f64;
    match bin_upper(bin) {
        Some(upper) if bin > 0 => lower * (upper as f64 / lower).powf(fraction),
        _ => lower,
    }
}

/// Mean, median and 90th percentile of a distribution; values are `None` without data.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SizeSummary {
    pub count: u64,
    pub mean: Option<f64>,
    pub p50: Option<f64>,
    pub p90: Option<f64>,
}

impl SizeSummary {
    /// Percentiles of a request-size histogram; the mean comes from exact sums.
    #[must_use]
    pub fn of_requests(histogram: &SizeHistogram, mean: Option<f64>) -> Self {
        Self {
            count: histogram.total(),
            mean,
            p50: histogram.quantile(0.5),
            p90: histogram.quantile(0.9),
        }
    }

    /// Exact statistics of individual values, such as session sizes.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "statistics for display")]
    pub fn of_values(values: &[u64]) -> Self {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let count = sorted.len() as u64;
        Self {
            count,
            mean: (count > 0).then(|| sorted.iter().sum::<u64>() as f64 / count as f64),
            p50: quantile_exact(&sorted, 0.5),
            p90: quantile_exact(&sorted, 0.9),
        }
    }
}

/// Request histograms per local period, aligned with ascending period `starts`.
#[must_use]
pub fn histograms_by_period(rows: &[SizeRow], starts: &[UtcMillis]) -> Vec<SizeHistogram> {
    let mut periods = vec![SizeHistogram::default(); starts.len()];
    for row in rows {
        if let Some(index) = period_index(starts, row.bucket_start) {
            periods[index].add(row.bin, row.requests);
        }
    }
    periods
}

/// Session sizes per local period of their first event, aligned with ascending `starts`.
#[must_use]
pub fn sessions_by_period(sessions: &[SessionSize], starts: &[UtcMillis]) -> Vec<Vec<u64>> {
    let mut periods = vec![Vec::new(); starts.len()];
    for session in sessions {
        if let Some(index) = period_index(starts, session.first_at) {
            periods[index].push(session.tokens);
        }
    }
    periods
}

/// The `p`-quantile of ascending values, interpolating linearly between the closest
/// ranks (the usual "type 7" definition); `None` for no values.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "ranks of in-memory slices; the result is for display"
)]
pub fn quantile_exact(sorted: &[u64], p: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let position = p.clamp(0.0, 1.0) * last as f64;
    let below = position.floor() as usize;
    let above = position.ceil() as usize;
    let (low, high) = (sorted[below] as f64, sorted[above] as f64);
    Some(low + (high - low) * (position - below as f64))
}

#[cfg(test)]
mod tests;
