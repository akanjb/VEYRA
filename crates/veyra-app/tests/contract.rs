//! Contract tests: the analytics commands return exactly what the
//! analytics engine computes for the same scope, on data ingested from the Claude Code
//! fixtures through the real pipeline.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use jiff::tz::TimeZone;
use veyra_analytics::dimension::Filters;
use veyra_analytics::engine::evaluate;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_app::dto::{
    CacheRequest, Comparison, Labelled, ModelsRequest, OverviewRequest, Scope, ScopeFilters,
    SessionRequest, SessionSort, SessionsRequest, UsageRequest,
};
use veyra_app::{Api, AppMetadata, DataServices, DisplayNames};
use veyra_domain::source::{DiscoveryEnv, IngestControl};
use veyra_ingest::{DEFAULT_CHUNK_BYTES, IngestContext, discover, scan_all};
use veyra_registry::Registry;
use veyra_store::Store;
use veyra_testkit::TempDir;

struct NoIngest;

impl IngestControl for NoIngest {
    fn request_scan(&self) {}
    fn sources_changed(&self) {}
}

/// A database holding the ingested fixtures, removed on drop.
struct Ingested {
    _dir: TempDir,
    store: Arc<Store>,
}

fn ingested() -> Ingested {
    let dir = TempDir::new("veyra-contract");
    let home = dir.with_claude_code_home();
    let store = Arc::new(Store::open(&dir.path().join("veyra.db")).unwrap());
    let registry = Registry::bundled().unwrap();
    let connectors = veyra_connectors::all();
    let env = DiscoveryEnv {
        home: Some(home),
        location_overrides: BTreeMap::new(),
    };
    for source in discover(&store, &connectors, &env).unwrap() {
        store
            .set_source_enabled(source.id, true, veyra_domain::time::UtcMillis::now())
            .unwrap();
    }
    let cancel = AtomicBool::new(false);
    let report = scan_all(
        &IngestContext {
            store: &store,
            registry: &registry,
            connectors: &connectors,
            cancel: &cancel,
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            throttle: None,
        },
        "manual",
    )
    .unwrap();
    assert_eq!(report.inserted, 4, "the fixtures hold four requests");
    Ingested { _dir: dir, store }
}

fn api(store: &Arc<Store>) -> Api {
    let api = Api::new(AppMetadata {
        product_name: "Veyra".to_owned(),
        version: "0.0.0".to_owned(),
        build_profile: veyra_app::dto::BuildProfile::Debug,
    });
    let registry = Registry::bundled().unwrap();
    let connectors: Vec<_> = veyra_connectors::all()
        .iter()
        .map(|c| c.descriptor().clone())
        .collect();
    let names = DisplayNames::new(
        registry.provider_names(),
        registry.model_names(),
        &connectors,
    );
    assert!(api.attach(Ok(DataServices {
        sources: Arc::clone(store) as _,
        analytics: Arc::clone(store) as _,
        connectors,
        names,
        ingest: Box::new(NoIngest),
    })));
    api
}

/// The fixture day, 2026-11-08, in UTC.
fn scope() -> Scope {
    Scope {
        time_zone: "UTC".to_owned(),
        start: "2026-11-08".to_owned(),
        end: "2026-11-08".to_owned(),
        comparison: Comparison::Previous,
        filters: ScopeFilters::default(),
    }
}

fn engine_value(store: &Store, metric: MetricId) -> Option<f64> {
    let first = "2026-11-08".parse().unwrap();
    let result = evaluate(
        store,
        &QuerySpec {
            period: Period::local_days(first, 1, &TimeZone::UTC).unwrap(),
            comparison: None,
            filters: Filters::new(),
            group_by: vec![],
            grain: TimeGrain::Total,
            time_zone: TimeZone::UTC,
        },
    )
    .unwrap();
    metric.compute(&result.totals).map(MetricValue::as_f64)
}

#[test]
fn overview_numbers_equal_the_metric_engine() {
    let data = ingested();
    let overview = api(&data.store)
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    let k = &overview.kpis;
    for (kpi, metric) in [
        (&k.tokens, MetricId::TotalTokens),
        (&k.cost_nano_usd, MetricId::ApiEquivalentCost),
        (&k.requests, MetricId::Requests),
        (&k.cache_hit_ratio, MetricId::CacheHitRatio),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
        assert_eq!(
            kpi.definition,
            metric.definition().description,
            "one source of definitions"
        );
        assert!(kpi.value.is_some(), "{metric:?} is reported");
    }
    // Nothing was recorded the day before: not reported, never shown as zero.
    assert_eq!(k.tokens.previous, None);
    assert_eq!(k.tokens.change, None);
    assert_eq!(
        k.sessions.value,
        Some(2.0),
        "a main session and its subagent"
    );
    assert!(k.active_ms.value.unwrap() > 0.0);

    // The daily trend, the model ranking and the calendar add up to the headline.
    let tokens = k.tokens.value.unwrap();
    let daily: u64 = overview
        .daily
        .iter()
        .flat_map(|d| d.values.iter().map(|v| v.tokens))
        .sum();
    let ranked: u64 = overview.models.iter().map(|m| m.measures.tokens).sum();
    let calendar: u64 = overview.calendar.iter().map(|d| d.tokens).sum();
    let hours: u64 = overview.hours.iter().map(|h| h.tokens).sum();
    #[allow(clippy::cast_precision_loss, reason = "comparing with an f64 KPI")]
    for (part, name) in [
        (daily, "daily"),
        (ranked, "models"),
        (calendar, "calendar"),
        (hours, "hours"),
    ] {
        assert_eq!(part as f64, tokens, "{name}");
    }
    let named = |key: &str, label: &str| Labelled {
        key: key.to_owned(),
        label: label.to_owned(),
    };
    assert_eq!(overview.apps, [named("claude-code", "Claude Code")]);
    assert_eq!(overview.providers, [named("anthropic", "Anthropic")]);
    let labels: Vec<&str> = overview.models.iter().map(|m| m.label.as_str()).collect();
    assert_eq!(labels.len(), 2);
    assert!(
        overview
            .models
            .iter()
            .all(|m| m.provider.as_deref() == Some("anthropic"))
    );
    assert_eq!(overview.composition.exact, 1.0);
    assert_eq!(overview.projects.len(), 1);
    assert!(overview.projects[0].label.contains("work/demo"));
}

#[test]
fn sessions_and_their_timelines_match_the_overview() {
    let data = ingested();
    let api = api(&data.store);
    let list = api
        .list_sessions(SessionsRequest {
            scope: scope(),
            sort: SessionSort::Tokens,
            page: 0,
            page_size: 50,
        })
        .unwrap();
    assert_eq!(list.total, 2);
    let main = list.rows.iter().find(|r| !r.is_subagent).unwrap();
    let sub = list.rows.iter().find(|r| r.is_subagent).unwrap();
    assert_eq!(sub.parent_id, Some(main.id));
    assert_eq!(main.events, 3);
    // req_C reported an inconsistent cache-write split: priced with an assumption.
    assert!(main.cost_assumptions);
    assert!(!sub.cost_assumptions);
    assert_eq!(main.unpriced_requests + sub.unpriced_requests, 0);

    let overview = api
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    let listed: u64 = list.rows.iter().map(|r| r.measures.tokens).sum();
    #[allow(clippy::cast_precision_loss, reason = "comparing with an f64 KPI")]
    let listed = listed as f64;
    assert_eq!(Some(listed), overview.kpis.tokens.value);

    let detail = api.get_session(SessionRequest { id: main.id }).unwrap();
    assert_eq!(detail.session.measures, main.measures);
    assert_eq!(detail.events.len(), 3);
    assert_eq!(detail.subagents.len(), 1);
    let tools: Vec<&str> = detail
        .events
        .iter()
        .flat_map(|e| e.tools.iter().map(|t| t.name.as_str()))
        .collect();
    assert!(tools.contains(&"Bash"), "{tools:?}");
    let events_total: u64 = detail.events.iter().map(|e| e.tokens.total).sum();
    assert_eq!(events_total, main.measures.tokens);

    // Every page names a model the same way.
    let overview_names: Vec<&str> = overview.models.iter().map(|m| m.label.as_str()).collect();
    let session_names = main.models.iter().map(|m| m.label.as_str());
    let timeline_names = detail
        .events
        .iter()
        .flat_map(|e| &e.model)
        .map(|m| m.label.as_str());
    for name in session_names.chain(timeline_names) {
        assert!(
            overview_names.contains(&name),
            "{name} vs {overview_names:?}"
        );
    }
}

#[test]
fn usage_numbers_equal_the_metric_engine_and_add_up() {
    let data = ingested();
    let usage = api(&data.store)
        .get_usage(UsageRequest { scope: scope() })
        .unwrap();
    let k = &usage.kpis;
    for (kpi, metric) in [
        (&k.tokens, MetricId::TotalTokens),
        (&k.input_tokens, MetricId::InputTokens),
        (&k.output_tokens, MetricId::OutputTokens),
        (&k.requests, MetricId::Requests),
        (&k.tokens_per_request, MetricId::TokensPerRequest),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
        assert_eq!(kpi.definition, metric.definition().description);
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "whole tokens"
    )]
    let tokens = k.tokens.value.unwrap() as u64;

    // The four token kinds add up to the total, day by day.
    let mixed: u64 = usage
        .composition
        .iter()
        .map(|d| d.input_uncached + d.cache_read + d.cache_write + d.output)
        .sum();
    assert_eq!(mixed, tokens);

    // Every breakdown covers every token once, and its Pareto line ends at 100%.
    assert_eq!(usage.breakdowns.len(), 6);
    for breakdown in &usage.breakdowns {
        let total: u64 = breakdown.rows.iter().map(|r| r.measures.tokens).sum();
        assert_eq!(total, tokens, "{:?}", breakdown.dimension);
        assert_eq!(breakdown.cumulative_share.len(), breakdown.rows.len());
        assert_eq!(breakdown.cumulative_share.last().copied(), Some(1.0));
        let index = breakdown.concentration.unwrap();
        assert!(index > 0.0 && index <= 1.0, "{:?}", breakdown.dimension);
    }

    // Each request is in one size bin; each session counts once.
    let binned: u64 = usage.request_sizes.iter().map(|b| b.requests).sum();
    assert_eq!(binned, 4, "the fixtures hold four requests");
    let per_day: u64 = usage.per_request.iter().map(|p| p.count).sum();
    assert_eq!(per_day, 4);
    let sessions: u64 = usage.per_session.iter().map(|p| p.count).sum();
    assert_eq!(sessions, 2, "a main session and its subagent");
    let day = &usage.per_request[0];
    assert!(day.p50.unwrap() <= day.p90.unwrap());
    assert_eq!(
        usage.request_definition,
        veyra_analytics::sizes::REQUEST_PERCENTILES_DEFINITION
    );
}

#[test]
fn cache_numbers_equal_the_metric_engine_and_add_up() {
    let data = ingested();
    let cache = api(&data.store)
        .get_cache(CacheRequest { scope: scope() })
        .unwrap();
    let k = &cache.kpis;
    for (kpi, metric) in [
        (&k.net_benefit_nano_usd, MetricId::NetCacheBenefit),
        (&k.gross_saving_nano_usd, MetricId::GrossCacheSaving),
        (&k.write_overhead_nano_usd, MetricId::CacheWriteOverhead),
        (&k.hit_ratio, MetricId::CacheHitRatio),
        (&k.reuse_ratio, MetricId::CacheReuseRatio),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
        assert_eq!(kpi.definition, metric.definition().description);
    }
    assert_eq!(
        cache.pricing_coverage,
        engine_value(&data.store, MetricId::PricingCoverage)
    );

    #[allow(
        clippy::cast_precision_loss,
        reason = "comparing with f64 engine values"
    )]
    let as_f64 = |n: u64| Some(n as f64);
    let reads: u64 = cache.daily.iter().map(|d| d.reads).sum();
    let writes: u64 = cache.daily.iter().map(|d| d.writes).sum();
    assert_eq!(
        as_f64(reads),
        engine_value(&data.store, MetricId::CacheReadTokens)
    );
    assert_eq!(
        as_f64(writes),
        engine_value(&data.store, MetricId::CacheWriteTokens)
    );

    // The hit-ratio rankings cover every input token once.
    for rows in [&cache.by_model, &cache.by_project] {
        let input: u64 = rows.iter().map(|r| r.input_tokens).sum();
        assert_eq!(
            as_f64(input),
            engine_value(&data.store, MetricId::InputTokens)
        );
    }
    let written: u64 = cache
        .writes_by_model
        .iter()
        .map(|r| r.short + r.long + r.unspecified)
        .sum();
    assert_eq!(
        as_f64(written),
        engine_value(&data.store, MetricId::CacheWriteTokens)
    );

    // Listed sessions lost money on caching, largest loss first.
    assert!(cache.inefficient.iter().all(|s| s.net_benefit_nano_usd < 0));
    assert!(
        cache
            .inefficient
            .windows(2)
            .all(|w| w[0].net_benefit_nano_usd <= w[1].net_benefit_nano_usd)
    );
    assert_eq!(
        cache.inefficient_definition,
        veyra_analytics::metrics::CACHE_LOSS_DEFINITION
    );
}

#[test]
fn models_numbers_add_up_to_the_scope() {
    let data = ingested();
    let models = api(&data.store)
        .get_models(ModelsRequest { scope: scope() })
        .unwrap();
    #[allow(
        clippy::cast_precision_loss,
        reason = "comparing with f64 engine values"
    )]
    let as_f64 = |n: u64| Some(n as f64);
    let tokens = engine_value(&data.store, MetricId::TotalTokens);

    // The leaderboard covers every token once, largest model first.
    let ranked: Vec<u64> = models
        .leaderboard
        .iter()
        .map(|m| m.measures.tokens)
        .collect();
    assert_eq!(as_f64(ranked.iter().sum()), tokens);
    assert!(ranked.windows(2).all(|w| w[0] >= w[1]));
    assert!(
        models
            .leaderboard
            .iter()
            .all(|m| m.first_day == "2026-11-08")
    );
    assert!(
        models.leaderboard.iter().all(
            |m| m.provider_label == "Anthropic" && m.cost_per_million_output_nano_usd.is_some()
        )
    );

    // Each day's provider shares add up to one (or are all zero without tokens).
    for day in &models.provider_share {
        let total: f64 = day.shares.iter().sum();
        assert!(total == 0.0 || (total - 1.0).abs() < 1e-9, "{}", day.date);
    }

    // Both matrices cover every token; the timeline covers the top models' tokens.
    for matrix in [&models.provider_by_app, &models.model_by_app] {
        let cells: u64 = matrix.values.iter().flatten().sum();
        assert_eq!(as_f64(cells), tokens);
        assert!(
            matrix
                .values
                .iter()
                .all(|row| row.len() == matrix.columns.len())
        );
    }
    let lanes: u64 = models.timeline.iter().flat_map(|l| l.tokens.iter()).sum();
    assert_eq!(as_f64(lanes), tokens);
    assert_eq!(
        models.rate_definition,
        MetricId::CostPerMillionOutputTokens
            .definition()
            .description
    );
}
