//! The metric registry (ADR-008): every derived metric's formula, unit and description,
//! defined once. The dashboard shows these descriptions as tooltips.

use veyra_domain::money::{NanoUsd, div_round_half_even};

use crate::sums::BaseSums;

/// Every metric Veyra can compute from base sums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricId {
    TotalTokens,
    InputTokens,
    OutputTokens,
    ReasoningTokens,
    CacheReadTokens,
    CacheWriteTokens,
    Requests,
    Events,
    ApiEquivalentCost,
    ReportedCost,
    PricingCoverage,
    CacheHitRatio,
    CacheReuseRatio,
    GrossCacheSaving,
    CacheWriteOverhead,
    NetCacheBenefit,
    CostPerRequest,
    TokensPerRequest,
    CostPerMillionOutputTokens,
}

/// How a metric value is expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Tokens,
    Count,
    /// Nano-USD.
    Money,
    /// A fraction between 0 and 1 (or above, for reuse).
    Ratio,
    TokensPerRequest,
    /// Nano-USD per one million output tokens.
    MoneyPerMillionTokens,
}

/// A computed metric value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricValue {
    Count(u64),
    Money(NanoUsd),
    Ratio(f64),
}

impl MetricValue {
    /// The value as a float, for ranking and change calculations.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "ranking and display only")]
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Count(n) => n as f64,
            Self::Money(m) => m.0 as f64,
            Self::Ratio(r) => r,
        }
    }
}

/// A metric's definition: the single source of its formula and description.
#[derive(Debug, Clone, Copy)]
pub struct MetricDefinition {
    pub id: MetricId,
    /// Stable key used in contracts and exports.
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub description: &'static str,
    compute: fn(&BaseSums) -> Option<MetricValue>,
}

impl MetricDefinition {
    /// Computes the metric; `None` when its inputs were not reported.
    #[must_use]
    pub fn compute(&self, sums: &BaseSums) -> Option<MetricValue> {
        (self.compute)(sums)
    }
}

impl MetricId {
    /// This metric's definition.
    #[must_use]
    pub fn definition(self) -> &'static MetricDefinition {
        METRICS.iter().find(|m| m.id == self).unwrap_or(&METRICS[0])
    }

    #[must_use]
    pub fn compute(self, sums: &BaseSums) -> Option<MetricValue> {
        self.definition().compute(sums)
    }
}

/// The relative and absolute change of a metric between two periods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Delta {
    pub absolute: f64,
    /// `None` when the previous value is zero (change is undefined, not infinite).
    pub relative: Option<f64>,
}

/// The change from `previous` to `current`.
#[must_use]
pub fn delta(current: Option<MetricValue>, previous: Option<MetricValue>) -> Option<Delta> {
    let (current, previous) = (current?.as_f64(), previous?.as_f64());
    let absolute = current - previous;
    let relative = (previous != 0.0).then(|| absolute / previous.abs());
    Some(Delta { absolute, relative })
}

#[expect(clippy::cast_precision_loss, reason = "ratios are for display")]
fn ratio(numerator: u64, denominator: u64) -> Option<MetricValue> {
    (denominator > 0).then(|| MetricValue::Ratio(numerator as f64 / denominator as f64))
}

/// `money × scale / count`, rounded half to even; `None` for zero counts or negative money.
fn money_per(money: NanoUsd, scale: u64, count: u64) -> Option<MetricValue> {
    if count == 0 || money.0 < 0 {
        return None;
    }
    let numerator = u128::try_from(money.0).ok()? * u128::from(scale);
    let value = div_round_half_even(numerator, u128::from(count));
    i64::try_from(value)
        .ok()
        .map(|v| MetricValue::Money(NanoUsd(v)))
}

fn count(value: Option<u64>) -> Option<MetricValue> {
    value.map(MetricValue::Count)
}

fn cache_write_total(s: &BaseSums) -> Option<u64> {
    s.tokens.cache_write_total()
}

fn net_cache_benefit(s: &BaseSums) -> Option<MetricValue> {
    let saving = s.cost.cache_read_saving?;
    // Without reported cache writes there is no overhead; with writes whose overhead
    // could not be computed, the net benefit is unknown rather than overstated.
    let overhead = match (cache_write_total(s), s.cost.cache_write_overhead) {
        (None | Some(0), _) => NanoUsd::ZERO,
        (Some(_), Some(overhead)) => overhead,
        (Some(_), None) => return None,
    };
    Some(MetricValue::Money(NanoUsd(saving.0 - overhead.0)))
}

/// What a loss-making cache session is.
pub const CACHE_LOSS_DEFINITION: &str = "Sessions whose cache writes cost more at API prices than their cache reads saved (a negative net cache benefit), among the sessions that write the most to the cache.";

/// The net cache benefit in nano-USD; `None` when it cannot be computed.
#[must_use]
pub fn net_cache_benefit_nano_usd(sums: &BaseSums) -> Option<i64> {
    match MetricId::NetCacheBenefit.compute(sums) {
        Some(MetricValue::Money(money)) => Some(money.0),
        _ => None,
    }
}

/// The registry. Token totals reuse the domain's canonical formulas (`TokenCounts`).
pub const METRICS: &[MetricDefinition] = &[
    MetricDefinition {
        id: MetricId::TotalTokens,
        key: "total_tokens",
        label: "Total tokens",
        unit: Unit::Tokens,
        description: "Input tokens (uncached, cache reads and cache writes) plus output tokens. Reasoning is part of output and is not added again.",
        compute: |s| count(s.tokens.total()),
    },
    MetricDefinition {
        id: MetricId::InputTokens,
        key: "input_tokens",
        label: "Input tokens",
        unit: Unit::Tokens,
        description: "All prompt tokens: uncached input, cache reads and cache writes.",
        compute: |s| count(s.tokens.prompt_tokens()),
    },
    MetricDefinition {
        id: MetricId::OutputTokens,
        key: "output_tokens",
        label: "Output tokens",
        unit: Unit::Tokens,
        description: "All generated tokens, including reasoning.",
        compute: |s| count(s.tokens.output_total),
    },
    MetricDefinition {
        id: MetricId::ReasoningTokens,
        key: "reasoning_tokens",
        label: "Reasoning tokens",
        unit: Unit::Tokens,
        description: "Reasoning (thinking) tokens where the source reports them; already included in output tokens.",
        compute: |s| count(s.tokens.reasoning),
    },
    MetricDefinition {
        id: MetricId::CacheReadTokens,
        key: "cache_read_tokens",
        label: "Cache reads",
        unit: Unit::Tokens,
        description: "Prompt tokens served from the provider's cache.",
        compute: |s| count(s.tokens.cache_read),
    },
    MetricDefinition {
        id: MetricId::CacheWriteTokens,
        key: "cache_write_tokens",
        label: "Cache writes",
        unit: Unit::Tokens,
        description: "Prompt tokens written to the provider's cache, all TTLs.",
        compute: |s| count(cache_write_total(s)),
    },
    MetricDefinition {
        id: MetricId::Requests,
        key: "requests",
        label: "Requests",
        unit: Unit::Count,
        description: "Model API calls, where the source records them.",
        compute: |s| count(s.requests),
    },
    MetricDefinition {
        id: MetricId::Events,
        key: "events",
        label: "Usage events",
        unit: Unit::Count,
        description: "Recorded usage events (requests or provider-reported buckets).",
        compute: |s| Some(MetricValue::Count(s.events)),
    },
    MetricDefinition {
        id: MetricId::ApiEquivalentCost,
        key: "api_equivalent_cost",
        label: "API-equivalent cost",
        unit: Unit::Money,
        description: "What the recorded tokens would cost at published API prices in effect at the time. An estimate, not a bill; unpriced models are excluded (see pricing coverage).",
        compute: |s| Some(MetricValue::Money(s.cost.total)),
    },
    MetricDefinition {
        id: MetricId::ReportedCost,
        key: "reported_cost",
        label: "Tool-reported cost",
        unit: Unit::Money,
        description: "Cost figures reported by the tools themselves, where available.",
        compute: |s| s.reported_cost.map(MetricValue::Money),
    },
    MetricDefinition {
        id: MetricId::PricingCoverage,
        key: "pricing_coverage",
        label: "Pricing coverage",
        unit: Unit::Ratio,
        description: "Share of tokens on events that could be priced. Below 100% means some models have no published price.",
        compute: |s| ratio(s.tokens_priced, s.tokens.total().unwrap_or(0)),
    },
    MetricDefinition {
        id: MetricId::CacheHitRatio,
        key: "cache_hit_ratio",
        label: "Cache hit ratio",
        unit: Unit::Ratio,
        description: "Cache reads divided by all input tokens.",
        compute: |s| ratio(s.tokens.cache_read?, s.tokens.prompt_tokens()?),
    },
    MetricDefinition {
        id: MetricId::CacheReuseRatio,
        key: "cache_reuse_ratio",
        label: "Cache reuse",
        unit: Unit::Ratio,
        description: "Cache reads per cache-written token. Above 1 means cached content is read more than once.",
        compute: |s| ratio(s.tokens.cache_read?, cache_write_total(s)?),
    },
    MetricDefinition {
        id: MetricId::GrossCacheSaving,
        key: "gross_cache_saving",
        label: "Gross cache saving",
        unit: Unit::Money,
        description: "Input-rate cost avoided by cache reads, over events where cache-read prices are published.",
        compute: |s| s.cost.cache_read_saving.map(MetricValue::Money),
    },
    MetricDefinition {
        id: MetricId::CacheWriteOverhead,
        key: "cache_write_overhead",
        label: "Cache write overhead",
        unit: Unit::Money,
        description: "Extra cost of cache writes above the normal input price.",
        compute: |s| s.cost.cache_write_overhead.map(MetricValue::Money),
    },
    MetricDefinition {
        id: MetricId::NetCacheBenefit,
        key: "net_cache_benefit",
        label: "Net cache benefit",
        unit: Unit::Money,
        description: "Gross cache saving minus cache write overhead. Unknown when write overhead cannot be priced.",
        compute: net_cache_benefit,
    },
    MetricDefinition {
        id: MetricId::CostPerRequest,
        key: "cost_per_request",
        label: "Cost per request",
        unit: Unit::Money,
        description: "API-equivalent cost divided by requests.",
        compute: |s| money_per(s.cost.total, 1, s.requests?),
    },
    MetricDefinition {
        id: MetricId::TokensPerRequest,
        key: "tokens_per_request",
        label: "Tokens per request",
        unit: Unit::TokensPerRequest,
        description: "Total tokens divided by requests.",
        compute: |s| ratio(s.tokens.total()?, s.requests?),
    },
    MetricDefinition {
        id: MetricId::CostPerMillionOutputTokens,
        key: "cost_per_million_output_tokens",
        label: "Cost per 1M output tokens",
        unit: Unit::MoneyPerMillionTokens,
        description: "API-equivalent cost per million output tokens: an effective rate comparable across models.",
        compute: |s| money_per(s.cost.total, 1_000_000, s.tokens.output_total?),
    },
];

#[cfg(test)]
mod tests;
