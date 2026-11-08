//! Store integration tests: idempotency, rollup correctness, analytics reads,
//! re-pricing and backups on a real SQLite file.

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use jiff::tz::TimeZone;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::engine::evaluate;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_analytics::sums::BaseSums;
use veyra_domain::accuracy::Accuracy;
use veyra_domain::identity::{ModelId, ProviderId};
use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{CostStatus, PriceCatalog, price_event};
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;
use veyra_store::{PricedEvent, Store, StoreError};

use support::{HOUR, NOV_8, Rng, TempDb, bundled_catalog, event, priced, tokens};

fn now() -> UtcMillis {
    UtcMillis(NOV_8 + 48 * HOUR)
}

fn all_time(store: &Store) -> BaseSums {
    let rows = store
        .read_base_sums(&BaseSumsQuery {
            from: UtcMillis(0),
            until: UtcMillis(NOV_8 * 2),
            filters: Filters::new(),
            group_by: vec![],
            by_bucket: false,
        })
        .unwrap();
    BaseSums::total(rows.iter().map(|r| &r.sums))
}

#[test]
fn writing_the_same_batch_twice_changes_nothing() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    let batch: Vec<PricedEvent> = (0..50)
        .map(|i| {
            priced(
                event(&format!("req-{i}"), NOV_8 + i * 60_000, tokens(1_000, 200)),
                &catalog,
            )
        })
        .collect();

    let first = db.store.write_events(&batch, now()).unwrap();
    assert_eq!((first.inserted, first.updated, first.unchanged), (50, 0, 0));
    let version = db.store.data_version();

    let second = db.store.write_events(&batch, now()).unwrap();
    assert_eq!(
        (second.inserted, second.updated, second.unchanged),
        (0, 0, 50)
    );
    assert_eq!(
        db.store.data_version(),
        version,
        "no-op writes do not bump the version"
    );

    let totals = all_time(&db.store);
    assert_eq!(totals.events, 50);
    assert_eq!(totals.tokens.input_uncached, Some(50_000));
}

#[test]
fn changed_events_update_and_moved_events_rebuild_both_buckets() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    db.store
        .write_events(
            &[priced(event("req-1", NOV_8, tokens(100, 10)), &catalog)],
            now(),
        )
        .unwrap();

    // Same request, corrected later: more tokens and a different time (another bucket).
    let corrected = priced(event("req-1", NOV_8 + HOUR, tokens(300, 30)), &catalog);
    let summary = db.store.write_events(&[corrected], now()).unwrap();
    assert_eq!((summary.updated, summary.buckets_rebuilt), (1, 2));

    let totals = all_time(&db.store);
    assert_eq!(totals.events, 1, "still one event: deduplicated by key");
    assert_eq!(totals.tokens.input_uncached, Some(300));
    let first_bucket = db
        .store
        .read_base_sums(&BaseSumsQuery {
            from: UtcMillis(NOV_8),
            until: UtcMillis(NOV_8 + 15 * 60_000),
            filters: Filters::new(),
            group_by: vec![],
            by_bucket: true,
        })
        .unwrap();
    assert!(
        first_bucket.is_empty(),
        "the old bucket no longer holds the event"
    );
}

#[test]
fn rollups_equal_raw_events_for_random_data() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    let mut rng = Rng::new(0x5eed_1234);
    let models = [
        "claude-sonnet-5-5",
        "claude-opus-5-5",
        "gpt-6.1-sol",
        "gemini-4-argon",
        "unknown-model",
    ];
    let classes = [
        Accuracy::Exact,
        Accuracy::Exact,
        Accuracy::Estimated,
        Accuracy::ProviderReported,
    ];

    let mut batch = Vec::new();
    let mut expected = BaseSums::default();
    for i in 0..2_000 {
        let opt = |rng: &mut Rng, max: u64| rng.chance(80).then(|| rng.below(max));
        let mut tokens = TokenCounts {
            input_uncached: opt(&mut rng, 50_000),
            cache_read: opt(&mut rng, 200_000),
            cache_write_short: opt(&mut rng, 20_000),
            cache_write_long: opt(&mut rng, 5_000),
            output_total: opt(&mut rng, 8_000),
            ..TokenCounts::default()
        };
        tokens.reasoning = tokens.output_total.map(|o| o / 3);
        let mut e = event(
            &format!("r{i}"),
            NOV_8 + i64::try_from(rng.below(3 * 24 * 60)).unwrap() * 60_000,
            tokens,
        );
        e.attribution.model = Some(ModelId::new(*rng.pick(&models)).unwrap());
        e.attribution.provider = Some(ProviderId::new("anthropic").unwrap());
        e.accuracy.tokens = *rng.pick(&classes);
        e.context.project_key = Some(format!("/p/{}", rng.below(5)));
        let p = priced(e, &catalog);
        expected.events += 1;
        expected.tokens.output_total = Some(
            expected.tokens.output_total.unwrap_or(0) + p.event.tokens.output_total.unwrap_or(0),
        );
        expected.cost.total = expected.cost.total + p.cost.breakdown.total();
        expected.tokens_by_accuracy[Accuracy::ALL
            .iter()
            .position(|a| *a == p.event.accuracy.tokens)
            .unwrap()] += p.event.tokens.total().unwrap_or(0);
        batch.push(p);
    }
    for chunk in batch.chunks(300) {
        db.store.write_events(chunk, now()).unwrap();
    }

    let actual = all_time(&db.store);
    assert_eq!(actual.events, expected.events);
    assert_eq!(actual.tokens.output_total, expected.tokens.output_total);
    assert_eq!(actual.cost.total, expected.cost.total);
    assert_eq!(actual.tokens_by_accuracy, expected.tokens_by_accuracy);
    assert!(
        actual.events_unpriced > 0,
        "unknown-model events are counted as unpriced"
    );

    // Grouped by project, the groups add up to the same totals.
    let by_project = db
        .store
        .read_base_sums(&BaseSumsQuery {
            from: UtcMillis(0),
            until: UtcMillis(NOV_8 * 2),
            filters: Filters::new(),
            group_by: vec![Dimension::Project],
            by_bucket: true,
        })
        .unwrap();
    let regrouped = BaseSums::total(by_project.iter().map(|r| &r.sums));
    assert_eq!(regrouped, actual);
}

#[test]
fn filters_select_values_and_unknowns() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    let mut unknown = event("u", NOV_8, tokens(5, 5));
    unknown.attribution.model = None;
    db.store
        .write_events(
            &[
                priced(event("k", NOV_8, tokens(10, 10)), &catalog),
                priced(unknown, &catalog),
            ],
            now(),
        )
        .unwrap();
    let read = |filters: Filters| {
        let rows = db
            .store
            .read_base_sums(&BaseSumsQuery {
                from: UtcMillis(0),
                until: UtcMillis(NOV_8 * 2),
                filters,
                group_by: vec![],
                by_bucket: false,
            })
            .unwrap();
        BaseSums::total(rows.iter().map(|r| &r.sums)).events
    };
    assert_eq!(
        read(Filters::new().with(Dimension::Model, [Some("claude-sonnet-5-5".to_owned())])),
        1
    );
    assert_eq!(read(Filters::new().with(Dimension::Model, [None])), 1);
    assert_eq!(
        read(Filters::new().with(Dimension::Model, [Some("x".to_owned()), None])),
        1
    );
    assert_eq!(
        read(Filters::new().with(Dimension::App, [Some("claude-code".to_owned())])),
        2
    );
}

#[test]
fn analytics_engine_runs_on_the_store() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    // Sonnet 5.5: 1M input × $2 + 100K output × $10 = $3.00 on 2026-11-08 (UTC day).
    db.store
        .write_events(
            &[priced(
                event("a", NOV_8 + HOUR, tokens(1_000_000, 100_000)),
                &catalog,
            )],
            now(),
        )
        .unwrap();
    let period = Period::new(UtcMillis(NOV_8), UtcMillis(NOV_8 + 24 * HOUR)).unwrap();
    let result = evaluate(
        &db.store,
        &QuerySpec {
            period,
            comparison: Some(period.previous()),
            filters: Filters::new(),
            group_by: vec![Dimension::Model],
            grain: TimeGrain::Hour,
            time_zone: TimeZone::UTC,
        },
    )
    .unwrap();
    assert_eq!(result.totals.cost.total, NanoUsd(3_000_000_000));
    assert_eq!(result.series.len(), 24);
    assert_eq!(result.series[1].sums.events, 1);
    assert_eq!(
        result.groups[0].key,
        vec![Some("claude-sonnet-5-5".to_owned())]
    );
    assert_eq!(result.previous_totals.map(|p| p.events), Some(0));
}

#[test]
fn repricing_applies_a_new_catalogue_and_rebuilds_rollups() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    db.store
        .write_events(
            &[priced(event("a", NOV_8, tokens(1_000_000, 0)), &catalog)],
            now(),
        )
        .unwrap();
    assert_eq!(all_time(&db.store).cost.total, NanoUsd(2_000_000_000));

    // A new catalogue version where Sonnet 5.5 input costs $3.
    let mut rules = catalog.rules().to_vec();
    for rule in &mut rules {
        if rule.id.ends_with("/claude-sonnet-5-5") {
            rule.base.input = veyra_domain::money::MicroUsdPerMTok(3_000_000);
        }
    }
    let updated = PriceCatalog::new("test-2", rules).unwrap();

    let pending = db
        .store
        .events_priced_with_other_catalog(updated.version(), 100)
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].event.tokens.input_uncached,
        Some(1_000_000),
        "round-trips the event"
    );
    let costs: Vec<_> = pending
        .into_iter()
        .map(|stored| {
            let cost = price_event(&stored.event, &updated);
            (stored, cost)
        })
        .collect();
    assert_eq!(costs[0].1.status, CostStatus::Priced);
    db.store.apply_costs(&costs).unwrap();

    assert_eq!(all_time(&db.store).cost.total, NanoUsd(3_000_000_000));
    let remaining = db
        .store
        .events_priced_with_other_catalog(updated.version(), 100)
        .unwrap();
    assert_eq!(remaining.len(), 0, "every event now uses the new catalogue");
    // Re-ingesting the same event with the new catalogue is now a no-op.
    let again = db
        .store
        .write_events(
            &[priced(event("a", NOV_8, tokens(1_000_000, 0)), &updated)],
            now(),
        )
        .unwrap();
    assert_eq!(again.unchanged, 1);
}

#[test]
fn invalid_events_reject_the_whole_batch() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    let mut bad = event("bad", NOV_8, tokens(1, 1));
    bad.tokens.reasoning = Some(5); // more reasoning than output
    let result = db.store.write_events(
        &[
            priced(event("good", NOV_8, tokens(1, 1)), &catalog),
            priced(bad, &catalog),
        ],
        now(),
    );
    assert!(matches!(result, Err(StoreError::InvalidEvent { .. })));
    assert_eq!(
        all_time(&db.store).events,
        0,
        "nothing from the batch was written"
    );
}

#[test]
fn backups_and_reopening_preserve_data() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    db.store
        .write_events(&[priced(event("a", NOV_8, tokens(7, 3)), &catalog)], now())
        .unwrap();
    let backup = db.path().join("backup.db");
    db.store.backup_to(&backup).unwrap();
    let restored = Store::open(&backup).unwrap();
    assert_eq!(all_time(&restored).tokens.input_uncached, Some(7));
}

#[test]
fn every_rollup_level_agrees() {
    let db = TempDb::new();
    let catalog = bundled_catalog();
    let mut rng = Rng::new(0xa11_1e7e15);
    let models = ["claude-sonnet-5-5", "gpt-6.1-sol", "gemini-4-argon"];
    let apps = ["claude-code", "codex", "gemini-cli"];
    let batch: Vec<PricedEvent> = (0..1_500)
        .map(|i| {
            let mut e = event(
                &format!("lv{i}"),
                NOV_8 + i64::try_from(rng.below(2 * 24 * 60)).unwrap() * 60_000,
                tokens(rng.below(10_000), rng.below(2_000)),
            );
            e.attribution.model = if rng.chance(10) {
                None
            } else {
                Some(ModelId::new(*rng.pick(&models)).unwrap())
            };
            e.attribution.app = veyra_domain::identity::AppId::new(*rng.pick(&apps)).unwrap();
            e.context.project_key = Some(format!("/p/{}", rng.below(4)));
            priced(e, &catalog)
        })
        .collect();
    db.store.write_events(&batch, now()).unwrap();

    let read = |group_by: Vec<Dimension>, filters: Filters, by_bucket: bool| {
        db.store
            .read_base_sums(&BaseSumsQuery {
                from: UtcMillis(NOV_8),
                until: UtcMillis(NOV_8 + 48 * HOUR),
                filters,
                group_by,
                by_bucket,
            })
            .unwrap()
    };
    let total =
        |rows: &[veyra_analytics::port::BaseSumsRow]| BaseSums::total(rows.iter().map(|r| &r.sums));

    let from_totals = total(&read(vec![], Filters::new(), true));
    let from_dimension = total(&read(vec![Dimension::Model], Filters::new(), true));
    let from_full = total(&read(
        vec![Dimension::Model, Dimension::App],
        Filters::new(),
        true,
    ));
    assert_eq!(from_totals, from_dimension);
    assert_eq!(from_totals, from_full);

    // Per model (including unknown): by-dimension rows equal full-rollup filtered sums.
    for model in [Some("gpt-6.1-sol".to_owned()), None] {
        let by_dimension = total(&read(
            vec![],
            Filters::new().with(Dimension::Model, [model.clone()]),
            false,
        ));
        let via_full = total(&read(
            vec![Dimension::App],
            Filters::new().with(Dimension::Model, [model.clone()]),
            false,
        ));
        assert_eq!(by_dimension, via_full, "{model:?}");
        assert!(by_dimension.events > 0);
    }

    // Integer dimensions (projects) work through every level too.
    let projects = read(vec![Dimension::Project], Filters::new(), false);
    assert_eq!(projects.len(), 4);
    let project = projects[0].key[0].clone();
    let one = total(&read(
        vec![],
        Filters::new().with(Dimension::Project, [project.clone()]),
        false,
    ));
    let one_full = total(&read(
        vec![Dimension::App],
        Filters::new().with(Dimension::Project, [project]),
        false,
    ));
    assert_eq!(one, one_full);
    assert_eq!(one, projects[0].sums);
}
