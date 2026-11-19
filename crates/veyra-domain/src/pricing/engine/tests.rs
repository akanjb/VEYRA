//! Pricing calculation tests. Every expected figure is derived by hand in the comments.

use super::{Assumption, CostStatus, price_event, price_tokens};
use crate::accuracy::{Accuracy, AccuracyMap};
use crate::event::{
    Attribution, DedupKey, EventContext, Granularity, ServiceTier, SourceRef, UsageEvent,
};
use crate::identity::{AppId, ConnectorId, Interface, ModelId, ProviderId};
use crate::money::{MicroUsdPerMTok, Multiplier, NanoUsd};
use crate::pricing::catalog::PriceCatalog;
use crate::pricing::rule::{
    EffectiveRange, ModelMatch, Modifiers, PriceRule, PromptTier, RateCard, TierFactor,
};
use crate::time::{CivilDate, UtcMillis};
use crate::tokens::TokenCounts;

const NOV_8: i64 = 1_794_096_000_000; // 2026-11-08T00:00:00Z

fn usd(value: f64) -> MicroUsdPerMTok {
    MicroUsdPerMTok::from_usd_per_mtok(value).unwrap()
}

/// $2 in, $10 out, $0.10 cache read, $2.50 short-TTL write, $4 long-TTL write (per 1M).
fn full_rates() -> RateCard {
    RateCard {
        input: usd(2.0),
        output: usd(10.0),
        cache_read: Some(usd(0.10)),
        cache_write_short: Some(usd(2.5)),
        cache_write_long: Some(usd(4.0)),
        reasoning: None,
    }
}

fn rule(id: &str, from: Option<i64>) -> PriceRule {
    PriceRule {
        id: id.to_owned(),
        provider: ProviderId::new("acme").unwrap(),
        model: ModelMatch::Exact(ModelId::new("model-a").unwrap()),
        effective: EffectiveRange {
            from: from.map(UtcMillis),
            until: None,
        },
        base: full_rates(),
        tiers: Vec::new(),
        modifiers: Modifiers::default(),
        source_url: "https://example.com/pricing".to_owned(),
        verified_at: CivilDate::new(2026, 11, 8).unwrap(),
        history_assumed: false,
    }
}

fn catalog(rules: Vec<PriceRule>) -> PriceCatalog {
    PriceCatalog::new("test-1", rules).unwrap()
}

fn event(tokens: TokenCounts) -> UsageEvent {
    UsageEvent {
        dedup_key: DedupKey::new("acme:req-1").unwrap(),
        occurred_at: UtcMillis(NOV_8),
        source: SourceRef {
            connector: ConnectorId::new("test").unwrap(),
            parser_version: 1,
            tool_version: None,
        },
        attribution: Attribution {
            provider: Some(ProviderId::new("acme").unwrap()),
            model: Some(ModelId::new("model-a").unwrap()),
            raw_model: Some("model-a".to_owned()),
            app: AppId::new("test-app").unwrap(),
            interface: Interface::Cli,
            gateway: None,
        },
        context: EventContext::default(),
        tokens,
        requests: Some(1),
        reported_cost: None,
        accuracy: AccuracyMap::uniform(Accuracy::Exact),
        granularity: Granularity::Request,
        service_tier: ServiceTier::Standard,
        inference_geo_us: false,
    }
}

fn typical_tokens() -> TokenCounts {
    TokenCounts {
        input_uncached: Some(1_000),
        cache_read: Some(10_000),
        cache_write_short: Some(2_000),
        cache_write_long: Some(500),
        output_total: Some(300),
        ..TokenCounts::default()
    }
}

#[test]
fn prices_every_component_exactly() {
    let cost = price_event(&event(typical_tokens()), &catalog(vec![rule("r", None)]));
    // input   1 000 × $2/M    = $0.002   = 2 000 000 n$
    // read   10 000 × $0.10/M = $0.001   = 1 000 000 n$
    // w-short2 000 × $2.50/M = $0.005   = 5 000 000 n$
    // w-long   500 × $4/M    = $0.002   = 2 000 000 n$
    // output    300 × $10/M   = $0.003   = 3 000 000 n$
    assert_eq!(cost.status, CostStatus::Priced);
    assert_eq!(cost.breakdown.input, NanoUsd(2_000_000));
    assert_eq!(cost.breakdown.cache_read, NanoUsd(1_000_000));
    assert_eq!(cost.breakdown.cache_write, NanoUsd(7_000_000));
    assert_eq!(cost.breakdown.output, NanoUsd(3_000_000));
    assert_eq!(cost.breakdown.total(), NanoUsd(13_000_000));
    assert_eq!(cost.rule_id.as_deref(), Some("r"));
    assert_eq!(cost.catalog_version, "test-1");
}

#[test]
fn computes_cache_economics() {
    let cost = price_event(&event(typical_tokens()), &catalog(vec![rule("r", None)]));
    // saving:   10 000 × ($2 − $0.10)/M          = 19 000 000 n$
    // overhead: 2 000 × ($2.50 − $2)/M = 1 000 000; 500 × ($4 − $2)/M = 1 000 000
    assert_eq!(cost.breakdown.cache_read_saving, Some(NanoUsd(19_000_000)));
    assert_eq!(
        cost.breakdown.cache_write_overhead,
        Some(NanoUsd(2_000_000))
    );
}

#[test]
fn cache_economics_are_unavailable_without_rates_or_tokens() {
    let mut r = rule("r", None);
    r.base.cache_read = None;
    let cost = price_event(&event(typical_tokens()), &catalog(vec![r]));
    assert_eq!(cost.breakdown.cache_read_saving, None);
    assert_eq!(cost.status, CostStatus::Partial);
    assert!(cost.assumptions.contains(Assumption::ComponentRateMissing));

    let no_cache = TokenCounts {
        input_uncached: Some(10),
        output_total: Some(10),
        ..TokenCounts::default()
    };
    let cost = price_event(&event(no_cache), &catalog(vec![rule("r", None)]));
    assert_eq!(cost.breakdown.cache_read_saving, None);
    assert_eq!(cost.breakdown.cache_write_overhead, None);
}

#[test]
fn prompt_tier_boundary_is_strictly_greater() {
    let mut r = rule("r", None);
    r.tiers = vec![PromptTier {
        above_prompt_tokens: 200_000,
        rates: RateCard {
            input: usd(4.0),
            output: usd(15.0),
            ..full_rates()
        },
    }];
    let catalog = catalog(vec![r]);
    let at = |prompt: u64| {
        let tokens = TokenCounts {
            input_uncached: Some(prompt),
            output_total: Some(1_000),
            ..TokenCounts::default()
        };
        price_event(&event(tokens), &catalog).breakdown
    };
    // 200 000 is not above the threshold → base: 200 000 × $2/M = $0.40; 1 000 × $10/M
    assert_eq!(at(200_000).input, NanoUsd(400_000_000));
    assert_eq!(at(200_000).output, NanoUsd(10_000_000));
    // 200 001 → tier: 200 001 × $4/M = $0.800004; 1 000 × $15/M = $0.015
    assert_eq!(at(200_001).input, NanoUsd(800_004_000));
    assert_eq!(at(200_001).output, NanoUsd(15_000_000));
}

#[test]
fn uses_the_rule_effective_at_the_event_time() {
    let mut old = rule("old", None);
    old.effective.until = Some(UtcMillis(NOV_8));
    old.base.input = usd(1.0);
    let new = rule("new", Some(NOV_8));
    let catalog = catalog(vec![old, new]);
    let tokens = TokenCounts {
        input_uncached: Some(1_000_000),
        ..TokenCounts::default()
    };
    let mut before = event(tokens);
    before.occurred_at = UtcMillis(NOV_8 - 1);
    let mut exactly = event(tokens);
    exactly.occurred_at = UtcMillis(NOV_8);

    let before = price_event(&before, &catalog);
    assert_eq!(
        (before.rule_id.as_deref(), before.breakdown.input),
        (Some("old"), NanoUsd(1_000_000_000))
    );
    let exactly = price_event(&exactly, &catalog);
    assert_eq!(
        (exactly.rule_id.as_deref(), exactly.breakdown.input),
        (Some("new"), NanoUsd(2_000_000_000))
    );
}

#[test]
fn unknown_models_and_empty_events_are_not_priced_as_zero() {
    let catalog = catalog(vec![rule("r", None)]);
    let mut unknown = event(typical_tokens());
    unknown.attribution.model = Some(ModelId::new("model-z").unwrap());
    assert_eq!(
        price_event(&unknown, &catalog).status,
        CostStatus::UnpricedModel
    );

    let mut unrecognised = event(typical_tokens());
    unrecognised.attribution.model = None;
    assert_eq!(
        price_event(&unrecognised, &catalog).status,
        CostStatus::UnpricedModel
    );

    let empty = event(TokenCounts::default());
    assert_eq!(price_event(&empty, &catalog).status, CostStatus::NoUsage);
}

#[test]
fn unspecified_cache_writes_use_the_5m_rate_and_are_partial() {
    let tokens = TokenCounts {
        cache_write_unspecified: Some(1_000),
        ..TokenCounts::default()
    };
    let cost = price_event(&event(tokens), &catalog(vec![rule("r", None)]));
    // 1 000 × $2.50/M = $0.0025
    assert_eq!(cost.breakdown.cache_write, NanoUsd(2_500_000));
    assert_eq!(cost.status, CostStatus::Partial);
    assert!(
        cost.assumptions
            .contains(Assumption::UnspecifiedCacheWriteAtShortRate)
    );
}

#[test]
fn applies_service_tier_and_regional_multipliers() {
    let mut r = rule("r", None);
    r.modifiers.batch = Some(TierFactor {
        factor: Multiplier::from_factor(0.5).unwrap(),
        cache_read: None,
    });
    r.modifiers.inference_geo_us = Some(Multiplier::from_factor(1.1).unwrap());
    let catalog = catalog(vec![r]);
    let tokens = TokenCounts {
        input_uncached: Some(1_000_000),
        ..TokenCounts::default()
    };

    let mut batch = event(tokens);
    batch.service_tier = ServiceTier::Batch;
    batch.inference_geo_us = true;
    // $2 × 0.5 × 1.1 = $1.10
    let cost = price_event(&batch, &catalog);
    assert_eq!(cost.breakdown.input, NanoUsd(1_100_000_000));
    assert_eq!(cost.status, CostStatus::Priced);

    let mut priority = event(tokens);
    priority.service_tier = ServiceTier::Priority;
    let cost = price_event(&priority, &catalog);
    assert_eq!(
        cost.breakdown.input,
        NanoUsd(2_000_000_000),
        "falls back to standard"
    );
    assert_eq!(cost.status, CostStatus::Partial);
    assert!(
        cost.assumptions
            .contains(Assumption::ServiceTierRateMissing)
    );
}

#[test]
fn bucketed_events_cannot_use_prompt_tiers() {
    let mut tiered = rule("r", None);
    tiered.tiers = vec![PromptTier {
        above_prompt_tokens: 10,
        rates: full_rates(),
    }];
    let mut bucket = event(typical_tokens());
    bucket.granularity = Granularity::Bucket;
    let cost = price_event(&bucket, &catalog(vec![tiered]));
    assert_eq!(cost.status, CostStatus::Partial);
    assert!(
        cost.assumptions
            .contains(Assumption::PromptSizeUnknownBaseTierUsed)
    );

    let cost = price_event(&bucket, &catalog(vec![rule("plain", None)]));
    assert_eq!(
        cost.status,
        CostStatus::Priced,
        "no tiers → nothing assumed"
    );
}

#[test]
fn rounds_each_component_half_to_even() {
    let mut r = rule("r", None);
    r.base.input = MicroUsdPerMTok(500); // $0.0005 per 1M tokens
    let catalog = catalog(vec![r]);
    let price = |n: u64| {
        let tokens = TokenCounts {
            input_uncached: Some(n),
            ..TokenCounts::default()
        };
        price_event(&event(tokens), &catalog).breakdown.input
    };
    // n × 500 / 1000 n$: 3 → 1.5 → 2 (to even), 5 → 2.5 → 2 (to even), 7 → 3.5 → 4
    assert_eq!(price(3), NanoUsd(2));
    assert_eq!(price(5), NanoUsd(2));
    assert_eq!(price(7), NanoUsd(4));
}

#[test]
fn separately_billed_reasoning_is_split_from_output() {
    let mut r = rule("r", None);
    r.base.reasoning = Some(usd(20.0));
    let tokens = TokenCounts {
        output_total: Some(1_000),
        reasoning: Some(400),
        ..TokenCounts::default()
    };
    let cost = price_event(&event(tokens), &catalog(vec![r]));
    // 600 × $10/M = 6 000 000 n$; 400 × $20/M = 8 000 000 n$
    assert_eq!(cost.breakdown.output, NanoUsd(14_000_000));
}

#[test]
fn flags_assumed_history_before_verification() {
    let mut r = rule("r", None);
    r.history_assumed = true;
    let catalog = catalog(vec![r]);
    let mut old = event(typical_tokens());
    old.occurred_at = UtcMillis(NOV_8 - 1);
    let cost = price_event(&old, &catalog);
    assert!(cost.assumptions.contains(Assumption::HistoryAssumed));
    assert_eq!(
        cost.status,
        CostStatus::Priced,
        "assumed history is flagged, not partial"
    );

    let recent = price_event(&event(typical_tokens()), &catalog);
    assert!(!recent.assumptions.contains(Assumption::HistoryAssumed));
}

#[test]
fn tiers_can_discount_cache_reads_differently() {
    // Google-style batch: 0.5× on input and output, cache reads at the standard price.
    let mut r = rule("r", None);
    r.modifiers.batch = Some(TierFactor {
        factor: Multiplier::from_factor(0.5).unwrap(),
        cache_read: Some(Multiplier::ONE),
    });
    let tokens = TokenCounts {
        input_uncached: Some(1_000_000),
        cache_read: Some(1_000_000),
        ..TokenCounts::default()
    };
    let mut batch = event(tokens);
    batch.service_tier = ServiceTier::Batch;
    let cost = price_event(&batch, &catalog(vec![r]));
    // input: $2 × 0.5 = $1; cache read: $0.10 × 1.0 = $0.10
    assert_eq!(cost.breakdown.input, NanoUsd(1_000_000_000));
    assert_eq!(cost.breakdown.cache_read, NanoUsd(100_000_000));
    // saving: avoided batch input $1.00 − actual read $0.10 = $0.90
    assert_eq!(cost.breakdown.cache_read_saving, Some(NanoUsd(900_000_000)));
}

#[test]
fn summed_tokens_are_priced_at_standard_base_rates() {
    let mut r = rule("r", None);
    r.modifiers.batch = Some(TierFactor {
        factor: Multiplier(5_000),
        cache_read: None,
    });
    let (breakdown, assumptions) = price_tokens(&typical_tokens(), &r);
    // Same figures as one standard request: 13 000 000 n$ in total.
    assert_eq!(breakdown.total(), NanoUsd(13_000_000));
    assert_eq!(assumptions.iter().count(), 0);

    r.tiers = vec![PromptTier {
        above_prompt_tokens: 1,
        rates: RateCard {
            input: usd(4.0),
            ..full_rates()
        },
    }];
    let (breakdown, assumptions) = price_tokens(&typical_tokens(), &r);
    // The prompt size of each request is unknown, so the tier is not used.
    assert_eq!(breakdown.input, NanoUsd(2_000_000));
    assert!(assumptions.contains(Assumption::PromptSizeUnknownBaseTierUsed));
}
