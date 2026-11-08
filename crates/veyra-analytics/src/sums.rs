//! Additive base measures: the only numbers the data layer returns.

use veyra_domain::accuracy::Accuracy;
use veyra_domain::money::NanoUsd;
use veyra_domain::tokens::TokenCounts;

/// Sums over a set of usage events. Token fields stay `None` when no event in the set
/// reported them, so "unavailable" survives aggregation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BaseSums {
    pub events: u64,
    pub requests: Option<u64>,
    pub tokens: TokenCounts,
    /// Cost reported by tools themselves (not an estimate).
    pub reported_cost: Option<NanoUsd>,
    pub cost: CostSums,
    pub events_priced: u64,
    pub events_partial: u64,
    pub events_unpriced: u64,
    /// Tokens on events that were priced (fully or partially).
    pub tokens_priced: u64,
    /// Tokens per accuracy class of the token measurement, indexed like [`Accuracy::ALL`].
    pub tokens_by_accuracy: [u64; 5],
}

/// API-equivalent cost sums.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CostSums {
    pub input: NanoUsd,
    pub cache_read: NanoUsd,
    pub cache_write: NanoUsd,
    pub output: NanoUsd,
    pub total: NanoUsd,
    /// Sum over events where the saving was computable.
    pub cache_read_saving: Option<NanoUsd>,
    /// Sum over events where the overhead was computable.
    pub cache_write_overhead: Option<NanoUsd>,
}

impl BaseSums {
    /// Adds another set of sums into this one.
    pub fn merge(&mut self, other: &Self) {
        self.events += other.events;
        self.requests = add_opt(self.requests, other.requests);
        let (t, o) = (&mut self.tokens, &other.tokens);
        t.input_uncached = add_opt(t.input_uncached, o.input_uncached);
        t.cache_read = add_opt(t.cache_read, o.cache_read);
        t.cache_write_short = add_opt(t.cache_write_short, o.cache_write_short);
        t.cache_write_long = add_opt(t.cache_write_long, o.cache_write_long);
        t.cache_write_unspecified = add_opt(t.cache_write_unspecified, o.cache_write_unspecified);
        t.output_total = add_opt(t.output_total, o.output_total);
        t.reasoning = add_opt(t.reasoning, o.reasoning);
        t.tool_tokens = add_opt(t.tool_tokens, o.tool_tokens);
        self.reported_cost = add_money_opt(self.reported_cost, other.reported_cost);
        self.cost.merge(&other.cost);
        self.events_priced += other.events_priced;
        self.events_partial += other.events_partial;
        self.events_unpriced += other.events_unpriced;
        self.tokens_priced += other.tokens_priced;
        for (mine, theirs) in self
            .tokens_by_accuracy
            .iter_mut()
            .zip(other.tokens_by_accuracy)
        {
            *mine += theirs;
        }
    }

    /// Tokens measured with the given accuracy class.
    #[must_use]
    pub fn tokens_with_accuracy(&self, accuracy: Accuracy) -> u64 {
        let index = Accuracy::ALL
            .iter()
            .position(|a| *a == accuracy)
            .unwrap_or(0);
        self.tokens_by_accuracy[index]
    }

    /// Sums of many rows.
    pub fn total<'a>(rows: impl IntoIterator<Item = &'a Self>) -> Self {
        rows.into_iter().fold(Self::default(), |mut acc, row| {
            acc.merge(row);
            acc
        })
    }
}

impl CostSums {
    fn merge(&mut self, other: &Self) {
        self.input = self.input + other.input;
        self.cache_read = self.cache_read + other.cache_read;
        self.cache_write = self.cache_write + other.cache_write;
        self.output = self.output + other.output;
        self.total = self.total + other.total;
        self.cache_read_saving = add_money_opt(self.cache_read_saving, other.cache_read_saving);
        self.cache_write_overhead =
            add_money_opt(self.cache_write_overhead, other.cache_write_overhead);
    }
}

fn add_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

fn add_money_opt(a: Option<NanoUsd>, b: Option<NanoUsd>) -> Option<NanoUsd> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or_default() + b.unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::BaseSums;
    use veyra_domain::money::NanoUsd;

    #[test]
    fn unreported_values_stay_unreported_when_merged() {
        let mut a = BaseSums {
            events: 1,
            ..BaseSums::default()
        };
        a.tokens.input_uncached = Some(10);
        let mut b = BaseSums {
            events: 2,
            ..BaseSums::default()
        };
        b.tokens.input_uncached = Some(5);
        b.tokens.output_total = Some(7);
        b.cost.total = NanoUsd(100);
        a.merge(&b);
        assert_eq!(a.events, 3);
        assert_eq!(a.tokens.input_uncached, Some(15));
        assert_eq!(a.tokens.output_total, Some(7));
        assert_eq!(a.tokens.reasoning, None, "still unreported");
        assert_eq!(a.cost.total, NanoUsd(100));
    }
}
