//! What a period would have cost on another model: the tokens each model used, priced at
//! another model's current published rates, beside their API-equivalent cost as
//! recorded (ADR-036). Only models whose every request was priced are compared, so both
//! sides cover the same tokens.

use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{Assumptions, PriceRule, price_tokens};

use crate::sums::BaseSums;

/// What the comparison means.
pub const WHAT_IF_DEFINITION: &str = "The tokens each model used in the period, priced at another model's current published rates at the standard service tier, beside their API-equivalent cost. Models with requests that have no published price are left out of both sides. Batch, priority and long-prompt prices are not applied to the other model, and a different model may need a different number of tokens for the same work.";

/// One model's recorded cost and its cost on the other model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repriced<K> {
    pub key: K,
    /// API-equivalent cost as recorded.
    pub current: NanoUsd,
    /// The same tokens at the other model's rates.
    pub repriced: NanoUsd,
    pub tokens: u64,
}

/// A period priced on another model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatIf<K> {
    /// Compared models, largest recorded cost first.
    pub rows: Vec<Repriced<K>>,
    pub current: NanoUsd,
    pub repriced: NanoUsd,
    /// Tokens compared on both sides.
    pub tokens_compared: u64,
    /// Tokens of models with unpriced requests, left out of both sides.
    pub tokens_left_out: u64,
    /// What had to be assumed to price the tokens on the other model.
    pub assumptions: Assumptions,
}

/// Prices each model's tokens at `target`'s rates. `models` holds the sums of each model.
#[must_use]
pub fn what_if<K>(
    models: impl IntoIterator<Item = (K, BaseSums)>,
    target: &PriceRule,
) -> WhatIf<K> {
    let mut result = WhatIf {
        rows: Vec::new(),
        current: NanoUsd::ZERO,
        repriced: NanoUsd::ZERO,
        tokens_compared: 0,
        tokens_left_out: 0,
        assumptions: Assumptions::default(),
    };
    for (key, sums) in models {
        let tokens = sums.tokens.total().unwrap_or(0);
        if tokens == 0 {
            continue;
        }
        if sums.events_unpriced > 0 {
            result.tokens_left_out = result.tokens_left_out.saturating_add(tokens);
            continue;
        }
        let (breakdown, assumptions) = price_tokens(&sums.tokens, target);
        for assumption in assumptions.iter() {
            result.assumptions.insert(assumption);
        }
        let row = Repriced {
            key,
            current: sums.cost.total,
            repriced: breakdown.total(),
            tokens,
        };
        result.current = result.current + row.current;
        result.repriced = result.repriced + row.repriced;
        result.tokens_compared = result.tokens_compared.saturating_add(tokens);
        result.rows.push(row);
    }
    result
        .rows
        .sort_by_key(|row| std::cmp::Reverse(row.current));
    result
}

#[cfg(test)]
mod tests {
    use veyra_domain::identity::{ModelId, ProviderId};
    use veyra_domain::money::{MicroUsdPerMTok, NanoUsd};
    use veyra_domain::pricing::{
        Assumption, EffectiveRange, ModelMatch, Modifiers, PriceRule, PromptTier, RateCard,
    };
    use veyra_domain::time::CivilDate;
    use veyra_domain::tokens::TokenCounts;

    use super::what_if;
    use crate::sums::BaseSums;

    fn usd(value: f64) -> MicroUsdPerMTok {
        MicroUsdPerMTok::from_usd_per_mtok(value).unwrap()
    }

    /// $1 in, $5 out, $0.10 cache read per 1M tokens.
    fn target() -> PriceRule {
        PriceRule {
            id: "acme/small".to_owned(),
            provider: ProviderId::new("acme").unwrap(),
            model: ModelMatch::Exact(ModelId::new("small").unwrap()),
            effective: EffectiveRange {
                from: None,
                until: None,
            },
            base: RateCard {
                input: usd(1.0),
                output: usd(5.0),
                cache_read: Some(usd(0.10)),
                cache_write_short: None,
                cache_write_long: None,
                reasoning: None,
            },
            tiers: Vec::new(),
            modifiers: Modifiers::default(),
            source_url: "https://example.com/pricing".to_owned(),
            verified_at: CivilDate::new(2026, 11, 8).unwrap(),
            history_assumed: false,
        }
    }

    fn sums(input: u64, output: u64, cost: i64, unpriced: u64) -> BaseSums {
        let mut sums = BaseSums {
            events: 1,
            events_unpriced: unpriced,
            tokens: TokenCounts {
                input_uncached: Some(input),
                output_total: Some(output),
                ..TokenCounts::default()
            },
            ..BaseSums::default()
        };
        sums.cost.total = NanoUsd(cost);
        sums
    }

    #[test]
    fn prices_each_compared_model_at_the_target_rates() {
        let result = what_if(
            [
                ("a", sums(1_000_000, 100_000, 3_000_000_000, 0)),
                ("b", sums(2_000_000, 0, 8_000_000_000, 0)),
            ],
            &target(),
        );
        // a: 1M × $1 + 0.1M × $5 = $1.50; b: 2M × $1 = $2.
        let rows: Vec<_> = result.rows.iter().map(|r| (r.key, r.repriced.0)).collect();
        assert_eq!(rows, [("b", 2_000_000_000), ("a", 1_500_000_000)]);
        assert_eq!(result.current, NanoUsd(11_000_000_000));
        assert_eq!(result.repriced, NanoUsd(3_500_000_000));
        assert_eq!(
            (result.tokens_compared, result.tokens_left_out),
            (3_100_000, 0)
        );
        assert_eq!(result.assumptions.iter().count(), 0);
    }

    #[test]
    fn models_with_unpriced_requests_are_left_out_of_both_sides() {
        let result = what_if(
            [
                ("priced", sums(1_000_000, 0, 2_000_000_000, 0)),
                ("unpriced", sums(500_000, 500_000, 0, 1)),
                ("empty", BaseSums::default()),
            ],
            &target(),
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.current, NanoUsd(2_000_000_000));
        assert_eq!(
            (result.tokens_compared, result.tokens_left_out),
            (1_000_000, 1_000_000)
        );
    }

    #[test]
    fn records_what_had_to_be_assumed() {
        let mut rule = target();
        rule.tiers = vec![PromptTier {
            above_prompt_tokens: 200_000,
            rates: rule.base,
        }];
        let mut writes = sums(0, 0, 1, 0);
        writes.tokens.cache_write_short = Some(1_000);
        let result = what_if([("a", writes)], &rule);
        assert!(
            result
                .assumptions
                .contains(Assumption::PromptSizeUnknownBaseTierUsed)
        );
        // The target publishes no cache-write rate, so those tokens add nothing.
        assert!(
            result
                .assumptions
                .contains(Assumption::ComponentRateMissing)
        );
        assert_eq!(result.repriced, NanoUsd::ZERO);
    }
}
