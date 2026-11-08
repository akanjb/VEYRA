//! Calculation tests for every metric, with hand-computed expected values.

use veyra_domain::money::NanoUsd;

use super::{METRICS, MetricId, MetricValue, delta};
use crate::sums::BaseSums;

/// 2 events, 4 requests; input 1 000 uncached + 3 000 cache read + 500 short + 500 long
/// writes; 600 output of which 100 reasoning; $0.05 total; saving $0.02; overhead $0.004.
fn sample() -> BaseSums {
    let mut s = BaseSums {
        events: 2,
        requests: Some(4),
        tokens_priced: 5_000,
        ..BaseSums::default()
    };
    s.tokens.input_uncached = Some(1_000);
    s.tokens.cache_read = Some(3_000);
    s.tokens.cache_write_short = Some(500);
    s.tokens.cache_write_long = Some(500);
    s.tokens.output_total = Some(600);
    s.tokens.reasoning = Some(100);
    s.cost.total = NanoUsd(50_000_000);
    s.cost.cache_read_saving = Some(NanoUsd(20_000_000));
    s.cost.cache_write_overhead = Some(NanoUsd(4_000_000));
    s
}

fn value(id: MetricId, s: &BaseSums) -> Option<MetricValue> {
    id.compute(s)
}

#[test]
fn token_metrics_use_canonical_formulas() {
    let s = sample();
    assert_eq!(
        value(MetricId::InputTokens, &s),
        Some(MetricValue::Count(5_000))
    );
    assert_eq!(
        value(MetricId::TotalTokens, &s),
        Some(MetricValue::Count(5_600))
    );
    assert_eq!(
        value(MetricId::OutputTokens, &s),
        Some(MetricValue::Count(600))
    );
    assert_eq!(
        value(MetricId::ReasoningTokens, &s),
        Some(MetricValue::Count(100))
    );
    assert_eq!(
        value(MetricId::CacheWriteTokens, &s),
        Some(MetricValue::Count(1_000))
    );
}

#[test]
fn ratios_are_ratios_of_sums() {
    let s = sample();
    // 3 000 / 5 000
    assert_eq!(
        value(MetricId::CacheHitRatio, &s),
        Some(MetricValue::Ratio(0.6))
    );
    // 3 000 / 1 000
    assert_eq!(
        value(MetricId::CacheReuseRatio, &s),
        Some(MetricValue::Ratio(3.0))
    );
    // 5 000 priced of 5 600 total
    assert_eq!(
        value(MetricId::PricingCoverage, &s),
        Some(MetricValue::Ratio(5_000.0 / 5_600.0))
    );
    // 5 600 / 4
    assert_eq!(
        value(MetricId::TokensPerRequest, &s),
        Some(MetricValue::Ratio(1_400.0))
    );
}

#[test]
fn money_metrics_are_exact() {
    let s = sample();
    // $0.05 / 4 = $0.0125
    assert_eq!(
        value(MetricId::CostPerRequest, &s),
        Some(MetricValue::Money(NanoUsd(12_500_000)))
    );
    // $0.05 / 600 × 1M = $83.333…  → 83 333 333 333 n$ (half-even)
    assert_eq!(
        value(MetricId::CostPerMillionOutputTokens, &s),
        Some(MetricValue::Money(NanoUsd(83_333_333_333)))
    );
    // $0.02 − $0.004
    assert_eq!(
        value(MetricId::NetCacheBenefit, &s),
        Some(MetricValue::Money(NanoUsd(16_000_000)))
    );
}

#[test]
fn unreported_inputs_give_no_value_instead_of_zero() {
    let empty = BaseSums::default();
    assert_eq!(value(MetricId::TotalTokens, &empty), None);
    assert_eq!(value(MetricId::CacheHitRatio, &empty), None);
    assert_eq!(value(MetricId::CostPerRequest, &empty), None);
    assert_eq!(value(MetricId::PricingCoverage, &empty), None);
    assert_eq!(value(MetricId::ReportedCost, &empty), None);
    assert_eq!(value(MetricId::Events, &empty), Some(MetricValue::Count(0)));
}

#[test]
fn net_cache_benefit_is_unknown_when_write_overhead_is_unpriced() {
    let mut s = sample();
    s.cost.cache_write_overhead = None;
    assert_eq!(value(MetricId::NetCacheBenefit, &s), None);
    // No cache writes at all: nothing to subtract.
    s.tokens.cache_write_short = None;
    s.tokens.cache_write_long = None;
    assert_eq!(
        value(MetricId::NetCacheBenefit, &s),
        Some(MetricValue::Money(NanoUsd(20_000_000)))
    );
}

#[test]
fn every_metric_has_a_unique_key_and_description() {
    let mut keys = std::collections::HashSet::new();
    for metric in METRICS {
        assert!(keys.insert(metric.key), "duplicate key {}", metric.key);
        assert_ne!(metric.description, "");
        assert_eq!(metric.id.definition().key, metric.key);
    }
}

#[test]
fn deltas_handle_zero_baselines() {
    let d = delta(Some(MetricValue::Count(150)), Some(MetricValue::Count(100))).unwrap();
    assert_eq!((d.absolute, d.relative), (50.0, Some(0.5)));
    let from_zero = delta(Some(MetricValue::Count(5)), Some(MetricValue::Count(0))).unwrap();
    assert_eq!(from_zero.relative, None);
    assert_eq!(delta(None, Some(MetricValue::Count(1))), None);
}
