//! Registry tests: the bundled data loads, references are consistent, and spot checks
//! reproduce prices exactly as published on the official pages (read 2026-11-08).

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{
    Attribution, DedupKey, EventContext, Granularity, ServiceTier, SourceRef, UsageEvent,
};
use veyra_domain::identity::{AppId, ConnectorId, Interface, ModelId};
use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{CostStatus, EventCost, price_event};
use veyra_domain::time::CivilDate;
use veyra_domain::tokens::TokenCounts;

use crate::{BUNDLED, Registry, RegistryError, Sources};

const MILLION: u64 = 1_000_000;
/// Below every published long-context threshold (100K, 200K, 272K).
const SHORT: u64 = 100_000;

fn model_id(value: &str) -> ModelId {
    ModelId::new(value).unwrap()
}

fn registry() -> Registry {
    Registry::bundled().unwrap_or_else(|e| panic!("bundled registry is invalid: {e}"))
}

fn price(model: &str, date: &str, tokens: TokenCounts, tier: ServiceTier) -> EventCost {
    let event = UsageEvent {
        dedup_key: DedupKey::new("test:1").unwrap(),
        occurred_at: CivilDate::parse(date).unwrap().start_utc(),
        source: SourceRef {
            connector: ConnectorId::new("test").unwrap(),
            parser_version: 1,
            tool_version: None,
        },
        attribution: Attribution {
            provider: None,
            model: Some(model_id(model)),
            raw_model: None,
            app: AppId::new("test").unwrap(),
            interface: Interface::Cli,
            gateway: None,
        },
        context: EventContext::default(),
        tokens,
        requests: Some(1),
        reported_cost: None,
        accuracy: AccuracyMap::uniform(Accuracy::Exact),
        granularity: Granularity::Request,
        service_tier: tier,
        inference_geo_us: false,
    };
    price_event(&event, registry().catalog())
}

fn input_only(n: u64) -> TokenCounts {
    TokenCounts {
        input_uncached: Some(n),
        ..TokenCounts::default()
    }
}

fn usd(dollars: f64) -> NanoUsd {
    #[expect(clippy::cast_possible_truncation, reason = "test values are small")]
    NanoUsd((dollars * 1e9).round() as i64)
}

#[test]
fn bundled_registry_is_valid() {
    let registry = registry();
    assert_eq!(registry.catalog().version(), "2026-11-08.1");
    assert!(registry.providers().len() >= 3);
    assert!(registry.models().len() >= 40);
}

#[test]
fn resolves_models_by_id_and_alias_case_insensitively() {
    let registry = registry();
    let resolve = |s: &str| registry.resolve_model(s).map(|m| m.id.to_string());
    assert_eq!(
        resolve("claude-sonnet-5-5").as_deref(),
        Some("claude-sonnet-5-5")
    );
    assert_eq!(
        resolve("Claude-Opus-4-5-20251101").as_deref(),
        Some("claude-opus-4-5")
    );
    assert_eq!(resolve(" gpt-6.1-sol ").as_deref(), Some("gpt-6.1-sol"));
    assert_eq!(resolve("not-a-model"), None);
}

#[test]
fn resolves_the_names_gateways_use() {
    let registry = registry();
    let resolve = |s: &str| registry.resolve_model(s).map(|m| m.id.to_string());
    // GitHub Copilot writes dots; OpenRouter adds the vendor.
    assert_eq!(
        resolve("claude-sonnet-4.5").as_deref(),
        Some("claude-sonnet-4-5")
    );
    assert_eq!(
        resolve("anthropic/claude-sonnet-4.5").as_deref(),
        Some("claude-sonnet-4-5")
    );
    assert_eq!(resolve("openai/gpt-5.5").as_deref(), Some("gpt-5.5"));
    // Names with dots of their own still match exactly.
    assert_eq!(resolve("gpt-5.5").as_deref(), Some("gpt-5.5"));
    assert_eq!(resolve("vendor/not-a-model"), None);
}

#[test]
fn anthropic_prices_match_the_published_table() {
    // Sonnet 5.5: $2 input; $0.10 cache hit; $2.50 5-minute write; $4 1-hour write.
    let tokens = TokenCounts {
        input_uncached: Some(MILLION),
        cache_read: Some(MILLION),
        cache_write_short: Some(MILLION),
        cache_write_long: Some(MILLION),
        output_total: Some(MILLION),
        ..TokenCounts::default()
    };
    let cost = price(
        "claude-sonnet-5-5",
        "2026-11-08",
        tokens,
        ServiceTier::Standard,
    );
    assert_eq!(cost.status, CostStatus::Priced);
    assert_eq!(cost.breakdown.input, usd(2.0));
    assert_eq!(cost.breakdown.cache_read, usd(0.10));
    assert_eq!(cost.breakdown.cache_write, usd(2.5 + 4.0));
    assert_eq!(cost.breakdown.output, usd(10.0));

    // Opus 5.5 fast mode is published as $8 / $40.
    let fast = price(
        "claude-opus-5-5",
        "2026-11-08",
        input_only(MILLION),
        ServiceTier::Fast,
    );
    assert_eq!(fast.breakdown.input, usd(8.0));
    // Opus 5.5 batch is published as $2 input.
    let batch = price(
        "claude-opus-5-5",
        "2026-11-08",
        input_only(MILLION),
        ServiceTier::Batch,
    );
    assert_eq!(batch.breakdown.input, usd(2.0));
}

#[test]
fn haiku_5_5_switches_tier_above_100k_prompt_tokens() {
    let at = |n| {
        price(
            "claude-haiku-5-5",
            "2026-11-08",
            input_only(n),
            ServiceTier::Standard,
        )
    };
    // 100 000 × $0.10/M = $0.01; 100 001 × $0.50/M = $0.0500005
    assert_eq!(at(100_000).breakdown.input, usd(0.01));
    assert_eq!(at(100_001).breakdown.input, NanoUsd(50_000_500));
}

#[test]
fn openai_long_context_and_tiers_match_the_published_tables() {
    let at = |n, tier| price("gpt-6.1-sol", "2026-11-08", input_only(n), tier);
    assert_eq!(
        at(MILLION / 4, ServiceTier::Standard).breakdown.input,
        usd(0.5)
    ); // 250K × $2
    assert_eq!(at(300_000, ServiceTier::Standard).breakdown.input, usd(1.2)); // 300K × $4 (long)
    assert_eq!(
        at(MILLION / 4, ServiceTier::Batch).breakdown.input,
        usd(0.25)
    ); // $1.00 batch
    assert_eq!(
        at(MILLION / 4, ServiceTier::Ultrafast).breakdown.input,
        usd(3.0)
    ); // $12.00
    // GPT-5.5 Fast is published at $12.50 (2.5×): 100K × $12.50 = $1.25.
    let fast = price(
        "gpt-5.5",
        "2026-11-08",
        input_only(SHORT),
        ServiceTier::Fast,
    );
    assert_eq!(fast.breakdown.input, usd(1.25));
    // A 1M prompt exceeds 272K, so long-context rates apply: $10 × 2.5 = $25.
    let long = price(
        "gpt-5.5",
        "2026-11-08",
        input_only(MILLION),
        ServiceTier::Fast,
    );
    assert_eq!(long.breakdown.input, usd(25.0));
}

#[test]
fn promotional_prices_end_on_their_published_date() {
    let at = |date| {
        price(
            "gpt-5.6-sol",
            date,
            input_only(SHORT),
            ServiceTier::Standard,
        )
    };
    assert_eq!(at("2026-11-30").breakdown.input, usd(0.40)); // 100K × $4
    assert_eq!(at("2026-12-01").status, CostStatus::UnpricedModel);
}

#[test]
fn google_scheduled_changes_and_reroutes_apply_on_their_dates() {
    let argon = |date| {
        price(
            "gemini-4-argon",
            date,
            input_only(MILLION),
            ServiceTier::Standard,
        )
    };
    assert_eq!(argon("2026-11-30").breakdown.input, usd(2.0));
    assert_eq!(argon("2026-12-01").breakdown.input, usd(4.0));

    // Gemini 2.5 Pro is billed at Gemini 3.8 Flash rates from 2026-12-01.
    let pro = |date| {
        price(
            "gemini-2.5-pro",
            date,
            input_only(SHORT),
            ServiceTier::Standard,
        )
    };
    assert_eq!(pro("2026-11-30").breakdown.input, usd(0.125)); // 100K × $1.25
    assert_eq!(pro("2026-12-01").breakdown.input, usd(0.075)); // 100K × $0.75
    assert_eq!(pro("2027-01-01").breakdown.input, usd(0.150)); // 100K × $1.50
}

#[test]
fn google_batch_keeps_cache_reads_at_standard_where_published() {
    let tokens = TokenCounts {
        input_uncached: Some(SHORT / 2),
        cache_read: Some(SHORT / 2),
        ..TokenCounts::default()
    };
    // 3.1 Pro Preview batch: $1.00 input; cache "Same as Standard" $0.20 (per 1M).
    let pro = price(
        "gemini-3.1-pro-preview",
        "2026-11-08",
        tokens,
        ServiceTier::Batch,
    );
    assert_eq!(pro.breakdown.input, usd(0.05)); // 50K × $1.00
    assert_eq!(pro.breakdown.cache_read, usd(0.01)); // 50K × $0.20
    // Argon batch: cache $0.05 per 1M (halved): 50K × $0.05 = $0.0025.
    let argon = price("gemini-4-argon", "2026-11-08", tokens, ServiceTier::Batch);
    assert_eq!(argon.breakdown.cache_read, usd(0.0025));
}

#[test]
fn models_without_published_prices_are_unpriced() {
    for model in [
        "claude-mythos-preview",
        "claude-3-7-sonnet",
        "claude-3-haiku",
    ] {
        let cost = price(model, "2026-11-08", input_only(10), ServiceTier::Standard);
        assert_eq!(cost.status, CostStatus::UnpricedModel, "{model}");
    }
}

#[test]
fn rejects_unknown_keys_and_unknown_models() {
    let typo = "[defaults]\nprovider = \"anthropic\"\nsource_url = \"x\"\nverified_at = \"2026-11-08\"\n\
                [[rule]]\nid = \"r\"\nmodels = [\"claude-sonnet-5-5\"]\nrates = { input = 1.0, output = 2.0, cache_raed = 0.1 }\n";
    let sources = Sources {
        pricing: &[("typo.toml", typo)],
        ..BUNDLED
    };
    assert!(matches!(
        Registry::parse(&sources),
        Err(RegistryError::Toml { .. })
    ));

    let unknown = typo
        .replace("cache_raed", "cache_read")
        .replace("claude-sonnet-5-5", "nope");
    let sources = Sources {
        pricing: &[("unknown.toml", &unknown)],
        ..BUNDLED
    };
    assert!(matches!(
        Registry::parse(&sources),
        Err(RegistryError::UnknownReference { .. })
    ));
}
