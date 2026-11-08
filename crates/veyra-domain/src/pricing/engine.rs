//! The pricing engine: canonical token counts + the rule in effect → API-equivalent cost.
//!
//! Every component is computed in exact integer arithmetic and rounded once, half to
//! even. Anything the engine had to assume is recorded, never silently absorbed.

use super::catalog::PriceCatalog;
use super::rule::{PriceRule, RateCard, TierFactor};
use crate::event::{Granularity, UsageEvent};
use crate::money::{MicroUsdPerMTok, Multiplier, NanoUsd, div_round_half_even};
use crate::time::CivilDate;
use crate::tokens::TokenCounts;

/// The outcome class of pricing one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CostStatus {
    /// Every reported component was priced with published rates.
    Priced,
    /// Priced, but some component or condition could not be priced exactly.
    Partial,
    /// No price rule covers the model (or the model is unknown).
    UnpricedModel,
    /// The event reports no tokens at all.
    NoUsage,
}

impl CostStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Priced => "priced",
            Self::Partial => "partial",
            Self::UnpricedModel => "unpriced-model",
            Self::NoUsage => "no-usage",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Priced,
            Self::Partial,
            Self::UnpricedModel,
            Self::NoUsage,
        ]
        .into_iter()
        .find(|s| s.as_str() == value)
    }
}

/// Something the engine had to assume; shown in the cost assumptions panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Assumption {
    /// Cache writes without a reported TTL were priced at the short-TTL write rate.
    UnspecifiedCacheWriteAtShortRate,
    /// Per-request prompt size was unknown, so prompt-length tiers used the base rate.
    PromptSizeUnknownBaseTierUsed,
    /// The service tier (batch, priority, …) has no published rate; standard was used.
    ServiceTierRateMissing,
    /// US-only inference was requested but no regional multiplier is published.
    InferenceGeoRateMissing,
    /// A reported token component has no published rate and was left unpriced.
    ComponentRateMissing,
    /// The event predates the rule's verification; prices are assumed unchanged.
    HistoryAssumed,
}

impl Assumption {
    const ALL: [Self; 6] = [
        Self::UnspecifiedCacheWriteAtShortRate,
        Self::PromptSizeUnknownBaseTierUsed,
        Self::ServiceTierRateMissing,
        Self::InferenceGeoRateMissing,
        Self::ComponentRateMissing,
        Self::HistoryAssumed,
    ];

    fn bit(self) -> u32 {
        match self {
            Self::UnspecifiedCacheWriteAtShortRate => 1,
            Self::PromptSizeUnknownBaseTierUsed => 1 << 1,
            Self::ServiceTierRateMissing => 1 << 2,
            Self::InferenceGeoRateMissing => 1 << 3,
            Self::ComponentRateMissing => 1 << 4,
            Self::HistoryAssumed => 1 << 5,
        }
    }

    /// Whether this assumption means the cost is not exact for the event.
    fn makes_partial(self) -> bool {
        !matches!(self, Self::HistoryAssumed)
    }
}

/// A compact set of [`Assumption`]s, stored as a bit set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Assumptions(u32);

impl Assumptions {
    pub fn insert(&mut self, assumption: Assumption) {
        self.0 |= assumption.bit();
    }

    #[must_use]
    pub fn contains(self, assumption: Assumption) -> bool {
        self.0 & assumption.bit() != 0
    }

    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Restores a set from stored bits, ignoring unknown bits.
    #[must_use]
    pub fn from_bits(bits: u32) -> Self {
        let known = Assumption::ALL.iter().fold(0, |acc, a| acc | a.bit());
        Self(bits & known)
    }

    pub fn iter(self) -> impl Iterator<Item = Assumption> {
        Assumption::ALL
            .into_iter()
            .filter(move |a| self.contains(*a))
    }

    fn makes_partial(self) -> bool {
        self.iter().any(Assumption::makes_partial)
    }
}

/// Cost per token component, plus the cache economics of the event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CostBreakdown {
    pub input: NanoUsd,
    pub cache_read: NanoUsd,
    pub cache_write: NanoUsd,
    pub output: NanoUsd,
    /// What cache reads saved versus paying the input rate. `None` when not computable.
    pub cache_read_saving: Option<NanoUsd>,
    /// What cache writes cost above the input rate. `None` when not computable.
    pub cache_write_overhead: Option<NanoUsd>,
}

impl CostBreakdown {
    #[must_use]
    pub fn total(&self) -> NanoUsd {
        self.input + self.cache_read + self.cache_write + self.output
    }
}

/// The API-equivalent cost of one event and how it was derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCost {
    pub status: CostStatus,
    pub rule_id: Option<String>,
    pub catalog_version: String,
    pub breakdown: CostBreakdown,
    pub assumptions: Assumptions,
    /// When the applied rates were last verified; staleness is judged at read time.
    pub rate_verified_at: Option<CivilDate>,
}

impl EventCost {
    fn without_rule(status: CostStatus, catalog: &PriceCatalog) -> Self {
        Self {
            status,
            rule_id: None,
            catalog_version: catalog.version().to_owned(),
            breakdown: CostBreakdown::default(),
            assumptions: Assumptions::default(),
            rate_verified_at: None,
        }
    }
}

/// Prices one event with the rule in effect at its timestamp.
#[must_use]
pub fn price_event(event: &UsageEvent, catalog: &PriceCatalog) -> EventCost {
    if event.tokens.is_empty() {
        return EventCost::without_rule(CostStatus::NoUsage, catalog);
    }
    let rule = event
        .attribution
        .model
        .as_ref()
        .and_then(|model| catalog.resolve(model, event.occurred_at));
    let Some(rule) = rule else {
        return EventCost::without_rule(CostStatus::UnpricedModel, catalog);
    };

    let mut assumptions = Assumptions::default();
    let rates = select_rates(rule, event, &mut assumptions);
    let factors = select_factors(rule, event, &mut assumptions);
    let breakdown = compute(&event.tokens, rates, factors, &mut assumptions);
    if rule.history_assumed
        && rule.effective.from.is_none()
        && event.occurred_at < rule.verified_at.start_utc()
    {
        assumptions.insert(Assumption::HistoryAssumed);
    }

    EventCost {
        status: if assumptions.makes_partial() {
            CostStatus::Partial
        } else {
            CostStatus::Priced
        },
        rule_id: Some(rule.id.clone()),
        catalog_version: catalog.version().to_owned(),
        breakdown,
        assumptions,
        rate_verified_at: Some(rule.verified_at),
    }
}

fn select_rates<'r>(
    rule: &'r PriceRule,
    event: &UsageEvent,
    assumptions: &mut Assumptions,
) -> &'r RateCard {
    let prompt = match event.granularity {
        Granularity::Request => event.tokens.prompt_tokens(),
        Granularity::Bucket => None,
    };
    if prompt.is_none() && !rule.tiers.is_empty() {
        assumptions.insert(Assumption::PromptSizeUnknownBaseTierUsed);
    }
    rule.rates_for(prompt)
}

/// Multipliers for the event: one for input, output and cache writes, one for cache
/// reads (tiers may discount cache reads differently).
#[derive(Debug, Clone, Copy)]
struct Factors {
    main: Multiplier,
    cache_read: Multiplier,
}

fn select_factors(rule: &PriceRule, event: &UsageEvent, assumptions: &mut Assumptions) -> Factors {
    let tier = rule
        .modifiers
        .for_tier(event.service_tier)
        .unwrap_or_else(|| {
            assumptions.insert(Assumption::ServiceTierRateMissing);
            TierFactor::STANDARD
        });
    let geo = if event.inference_geo_us {
        rule.modifiers.inference_geo_us.unwrap_or_else(|| {
            assumptions.insert(Assumption::InferenceGeoRateMissing);
            Multiplier::ONE
        })
    } else {
        Multiplier::ONE
    };
    Factors {
        main: tier.factor.then(geo),
        cache_read: tier.cache_read_factor().then(geo),
    }
}

fn compute(
    tokens: &TokenCounts,
    rates: &RateCard,
    factors: Factors,
    assumptions: &mut Assumptions,
) -> CostBreakdown {
    let mut price =
        |count: Option<u64>, rate: Option<MicroUsdPerMTok>, m: Multiplier| match (count, rate) {
            (None | Some(0), _) => NanoUsd::ZERO,
            (Some(count), Some(rate)) => cost(count, rate, m),
            (Some(_), None) => {
                assumptions.insert(Assumption::ComponentRateMissing);
                NanoUsd::ZERO
            }
        };

    let m = factors.main;
    let input = price(tokens.input_uncached, Some(rates.input), m);
    let cache_read = price(tokens.cache_read, rates.cache_read, factors.cache_read);
    let output = match (rates.reasoning, tokens.reasoning, tokens.output_total) {
        (Some(reasoning_rate), Some(reasoning), Some(output)) => {
            price(Some(output - reasoning), Some(rates.output), m)
                + price(Some(reasoning), Some(reasoning_rate), m)
        }
        _ => price(tokens.output_total, Some(rates.output), m),
    };
    // Writes without a reported TTL are priced at the short-TTL rate (recorded below).
    let cache_write = price(tokens.cache_write_short, rates.cache_write_short, m)
        + price(tokens.cache_write_long, rates.cache_write_long, m)
        + price(tokens.cache_write_unspecified, rates.cache_write_short, m);
    if tokens.cache_write_unspecified.is_some_and(|n| n > 0) {
        assumptions.insert(Assumption::UnspecifiedCacheWriteAtShortRate);
    }

    CostBreakdown {
        input,
        cache_read,
        cache_write,
        output,
        cache_read_saving: cache_read_saving(tokens, rates, factors),
        cache_write_overhead: cache_write_overhead(tokens, rates, factors.main),
    }
}

/// Input-rate cost avoided by reading from cache, under the same service tier.
/// Requires the cache-read rate.
fn cache_read_saving(tokens: &TokenCounts, rates: &RateCard, factors: Factors) -> Option<NanoUsd> {
    let count = tokens.cache_read?;
    let read_rate = rates.cache_read?;
    Some(NanoUsd(
        cost(count, rates.input, factors.main).0 - cost(count, read_rate, factors.cache_read).0,
    ))
}

/// Extra cost of cache writes above the input rate. `None` unless every reported write
/// component has a rate (unspecified TTL is treated as short-TTL, as when pricing).
fn cache_write_overhead(tokens: &TokenCounts, rates: &RateCard, m: Multiplier) -> Option<NanoUsd> {
    let parts = [
        (tokens.cache_write_short, rates.cache_write_short),
        (tokens.cache_write_long, rates.cache_write_long),
        (tokens.cache_write_unspecified, rates.cache_write_short),
    ];
    let mut overhead: Option<i64> = None;
    for (count, rate) in parts {
        let Some(count) = count else { continue };
        let rate = rate?;
        let extra = cost(count, rate, m).0 - cost(count, rates.input, m).0;
        overhead = Some(overhead.unwrap_or(0) + extra);
    }
    overhead.map(NanoUsd)
}

/// `tokens × rate × multiplier`, in nano-USD, rounded half to even.
///
/// Units: rate is micro-USD per 1M tokens, so micro-USD = tokens × rate / 10⁶ and
/// nUSD = tokens × rate / 10³; the multiplier is in basis points (/ 10⁴).
fn cost(tokens: u64, rate: MicroUsdPerMTok, multiplier: Multiplier) -> NanoUsd {
    let numerator = u128::from(tokens) * u128::from(rate.0) * u128::from(multiplier.0);
    let nano = div_round_half_even(numerator, 10_000_000);
    NanoUsd(i64::try_from(nano).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests;
