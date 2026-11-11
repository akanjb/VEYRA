//! How a period's cost was estimated, for the cost assumptions panel: which price rules
//! were applied, what the engine had to assume, which models have no price and which
//! rates have not been checked for a while (FR-PRICE-06).

use std::collections::BTreeMap;

use veyra_domain::pricing::{Assumption, CostStatus};
use veyra_domain::time::{CivilDate, UtcMillis};

use crate::port::{PricingUse, Tally};

/// A rate not checked against its published source for longer than this is stale.
pub const STALE_RATE_DAYS: i64 = 120;

const DAY_MILLIS: i64 = 86_400_000;

/// What the cost assumptions panel shows.
pub const ASSUMPTIONS_DEFINITION: &str = "How API-equivalent cost was estimated for the period: the published prices applied, anything that had to be assumed, and models with no published price, which add nothing to the cost. A price is stale when it was last checked against its source more than 120 days ago.";

/// Every outcome, in the order they are shown.
pub const STATUSES: [CostStatus; 4] = [
    CostStatus::Priced,
    CostStatus::Partial,
    CostStatus::UnpricedModel,
    CostStatus::NoUsage,
];

/// Whether a rate checked on `verified_at` is stale at `now`.
#[must_use]
pub fn is_stale(verified_at: CivilDate, now: UtcMillis) -> bool {
    now.0 - verified_at.start_utc().0 > STALE_RATE_DAYS * DAY_MILLIS
}

/// A model with no price rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpriced {
    pub provider: String,
    pub model: Option<String>,
    pub tally: Tally,
}

/// How a period was priced, summed from [`PricingUse`] rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PricingSummary {
    /// Each outcome that occurred, in [`STATUSES`] order.
    pub statuses: Vec<(CostStatus, Tally)>,
    /// Each assumption that was made, in [`Assumption::ALL`] order; an event with
    /// several assumptions counts under each.
    pub assumptions: Vec<(Assumption, Tally)>,
    /// Each price rule applied, highest cost first, then by id.
    pub rules: Vec<(String, Tally)>,
    /// Models with no price rule, most tokens first, then by provider and model.
    pub unpriced: Vec<Unpriced>,
}

/// Sums `uses` by outcome, assumption, rule and unpriced model.
#[must_use]
pub fn summarize(uses: &[PricingUse]) -> PricingSummary {
    let statuses = STATUSES
        .into_iter()
        .map(|status| (status, total(uses.iter().filter(|u| u.status == status))))
        .filter(|(_, tally)| tally.events > 0)
        .collect();
    let assumptions = Assumption::ALL
        .into_iter()
        .map(|a| (a, total(uses.iter().filter(|u| u.assumptions.contains(a)))))
        .filter(|(_, tally)| tally.events > 0)
        .collect();
    let mut rules: Vec<(String, Tally)> = grouped(
        uses.iter()
            .filter_map(|u| u.rule_id.clone().map(|id| (id, u.tally))),
    );
    rules.sort_by(|a, b| b.1.cost.cmp(&a.1.cost).then_with(|| a.0.cmp(&b.0)));
    let mut unpriced: Vec<Unpriced> = grouped(
        uses.iter()
            .filter(|u| u.status == CostStatus::UnpricedModel)
            .map(|u| ((u.provider.clone(), u.model.clone()), u.tally)),
    )
    .into_iter()
    .map(|((provider, model), tally)| Unpriced {
        provider,
        model,
        tally,
    })
    .collect();
    unpriced.sort_by(|a, b| {
        b.tally
            .tokens
            .cmp(&a.tally.tokens)
            .then_with(|| (&a.provider, &a.model).cmp(&(&b.provider, &b.model)))
    });
    PricingSummary {
        statuses,
        assumptions,
        rules,
        unpriced,
    }
}

fn total<'a>(uses: impl Iterator<Item = &'a PricingUse>) -> Tally {
    uses.fold(Tally::default(), |mut sum, u| {
        sum += u.tally;
        sum
    })
}

fn grouped<K: Ord>(items: impl Iterator<Item = (K, Tally)>) -> Vec<(K, Tally)> {
    let mut groups: BTreeMap<K, Tally> = BTreeMap::new();
    for (key, tally) in items {
        *groups.entry(key).or_default() += tally;
    }
    groups.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use veyra_domain::money::NanoUsd;
    use veyra_domain::pricing::{Assumption, Assumptions, CostStatus};
    use veyra_domain::time::{CivilDate, UtcMillis};

    use super::{STALE_RATE_DAYS, is_stale, summarize};
    use crate::port::{PricingUse, Tally};

    fn tally(events: u64, tokens: u64, cost: i64) -> Tally {
        Tally {
            events,
            tokens,
            cost: NanoUsd(cost),
        }
    }

    fn use_of(
        model: &str,
        rule: Option<&str>,
        status: CostStatus,
        assumed: &[Assumption],
        t: Tally,
    ) -> PricingUse {
        let mut assumptions = Assumptions::default();
        for a in assumed {
            assumptions.insert(*a);
        }
        PricingUse {
            provider: "acme".to_owned(),
            model: Some(model.to_owned()),
            rule_id: rule.map(str::to_owned),
            status,
            assumptions,
            tally: t,
        }
    }

    #[test]
    fn uses_are_summed_by_outcome_assumption_rule_and_unpriced_model() {
        let summary = summarize(&[
            use_of("a", Some("r-a"), CostStatus::Priced, &[], tally(2, 10, 5)),
            use_of(
                "a",
                Some("r-a"),
                CostStatus::Partial,
                &[
                    Assumption::HistoryAssumed,
                    Assumption::PromptSizeUnknownBaseTierUsed,
                ],
                tally(1, 4, 3),
            ),
            use_of("b", Some("r-b"), CostStatus::Priced, &[], tally(1, 50, 20)),
            use_of("x", None, CostStatus::UnpricedModel, &[], tally(3, 7, 0)),
            use_of("y", None, CostStatus::UnpricedModel, &[], tally(1, 9, 0)),
        ]);
        assert_eq!(
            summary.statuses,
            vec![
                (CostStatus::Priced, tally(3, 60, 25)),
                (CostStatus::Partial, tally(1, 4, 3)),
                (CostStatus::UnpricedModel, tally(4, 16, 0)),
            ]
        );
        assert_eq!(
            summary.assumptions,
            vec![
                (Assumption::PromptSizeUnknownBaseTierUsed, tally(1, 4, 3)),
                (Assumption::HistoryAssumed, tally(1, 4, 3)),
            ]
        );
        assert_eq!(
            summary.rules,
            vec![
                ("r-b".to_owned(), tally(1, 50, 20)),
                ("r-a".to_owned(), tally(3, 14, 8)),
            ]
        );
        let unpriced: Vec<_> = summary
            .unpriced
            .iter()
            .map(|u| u.model.as_deref())
            .collect();
        assert_eq!(unpriced, [Some("y"), Some("x")]);
    }

    #[test]
    fn nothing_to_summarize_gives_an_empty_summary() {
        assert_eq!(summarize(&[]), super::PricingSummary::default());
    }

    #[test]
    fn a_rate_is_stale_after_the_limit() {
        let checked = CivilDate::new(2026, 1, 1).unwrap();
        let day = 86_400_000;
        let limit = checked.start_utc().0 + STALE_RATE_DAYS * day;
        assert!(!is_stale(checked, UtcMillis(limit)));
        assert!(is_stale(checked, UtcMillis(limit + 1)));
    }
}
