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

    // Same request, corrected by a more complete observation: more tokens and a different
    // time (another bucket). A correction must carry higher precedence.
    let mut corrected = priced(event("req-1", NOV_8 + HOUR, tokens(300, 30)), &catalog);
    corrected.precedence = 1;
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

mod ingestion {
    use std::collections::HashSet;

    use veyra_domain::source::ParseIssue;
    use veyra_domain::tooling::{ToolCall, ToolOutcome};
    use veyra_store::{FileCheckpoint, IngestBatch, RunStats};

    use super::support::{NOV_8, TempDb, bundled_catalog, event, priced, tokens};
    use super::{all_time, now};

    fn observation(precedence: i64, at: i64, output: u64) -> veyra_store::PricedEvent {
        let mut p = priced(event("req-x", at, tokens(100, output)), &bundled_catalog());
        p.precedence = precedence;
        p
    }

    #[test]
    fn the_most_complete_observation_wins_in_any_order() {
        let partial = observation(1, NOV_8 + 1_000, 5);
        let final_ = observation(2, NOV_8 + 2_000, 50);
        for order in [[&partial, &final_], [&final_, &partial]] {
            let db = TempDb::new();
            for obs in order {
                db.store.write_events(&[(*obs).clone()], now()).unwrap();
            }
            assert_eq!(all_time(&db.store).tokens.output_total, Some(50));
        }
        // Equal precedence: the earlier observation is kept, whatever the order.
        let early = observation(2, NOV_8 + 1_000, 7);
        let late = observation(2, NOV_8 + 9_000, 9);
        for order in [[&early, &late], [&late, &early]] {
            let db = TempDb::new();
            for obs in order {
                db.store.write_events(&[(*obs).clone()], now()).unwrap();
            }
            assert_eq!(all_time(&db.store).tokens.output_total, Some(7));
        }
    }

    #[test]
    fn checkpoints_tool_calls_and_outcomes_commit_together() {
        let db = TempDb::new();
        let source = db
            .store
            .register_source("claude-code", "/root", now())
            .unwrap();
        assert!(!source.enabled, "sources start disabled until consent");
        let mut first = priced(event("req-1", NOV_8, tokens(1, 1)), &bundled_catalog());
        first.tool_calls = vec![ToolCall {
            id: "toolu_1".to_owned(),
            name: "mcp__github__create_issue".to_owned(),
        }];
        db.store
            .commit(
                &IngestBatch {
                    events: vec![first],
                    tool_outcomes: vec![ToolOutcome {
                        tool_call_id: "toolu_1".to_owned(),
                        is_error: true,
                    }],
                    checkpoint: Some(FileCheckpoint {
                        source_id: source.id,
                        path: "p/s.jsonl".to_owned(),
                        size: 120,
                        modified_at: Some(now()),
                        offset: 120,
                        head_hash: 42,
                        head_len: 120,
                        parser_version: 1,
                    }),
                },
                now(),
            )
            .unwrap();
        let checkpoints = db.store.checkpoints(source.id).unwrap();
        assert_eq!(checkpoints["p/s.jsonl"].offset, 120);

        let enabled = db
            .store
            .set_source_enabled(source.id, true, now())
            .unwrap()
            .unwrap();
        assert!(enabled.enabled && enabled.consented_at.is_some());

        assert_eq!(
            db.store
                .mark_missing_files(source.id, &HashSet::new())
                .unwrap(),
            1
        );
        assert_eq!(
            all_time(&db.store).events,
            1,
            "events survive missing source files"
        );

        let run = db.store.start_run("test", now()).unwrap();
        let issue = ParseIssue {
            kind: "malformed-json",
            offset: 7,
        };
        db.store
            .record_issues(run, source.id, "p/s.jsonl", &[issue, issue])
            .unwrap();
        db.store
            .finish_run(run, RunStats::default(), "ok", now())
            .unwrap();
    }
}

mod session_reads {
    use veyra_analytics::activity::{DEFAULT_IDLE_THRESHOLD_MS, activity};
    use veyra_analytics::dimension::{Dimension, Filters};
    use veyra_analytics::port::{ActivityReader, LabelReader, RangeQuery, SizeReader};
    use veyra_analytics::sessions::{SessionQuery, SessionReader, SessionSort};
    use veyra_analytics::sizes::{SizeHistogram, size_bin};
    use veyra_domain::identity::ModelId;
    use veyra_domain::time::UtcMillis;
    use veyra_domain::tooling::{ToolCall, ToolOutcome};
    use veyra_store::IngestBatch;

    use super::support::{NOV_8, TempDb, bundled_catalog, event, priced, tokens};
    use super::{all_time, now};

    const MIN: i64 = 60_000;

    /// Session s1 (three requests, one with tools), its subagent s1:a (one request on
    /// another model) and session s2 (one request), all on 2026-11-08.
    fn seeded() -> TempDb {
        let db = TempDb::new();
        let catalog = bundled_catalog();
        let mut events = Vec::new();
        for (i, (minute, input)) in [(0, 100), (2, 200), (40, 300)].into_iter().enumerate() {
            let mut priced_event = priced(
                event(&format!("s1-{i}"), NOV_8 + minute * MIN, tokens(input, 10)),
                &catalog,
            );
            if i == 1 {
                priced_event.tool_calls = vec![
                    ToolCall {
                        id: "toolu_1".to_owned(),
                        name: "Bash".to_owned(),
                    },
                    ToolCall {
                        id: "toolu_2".to_owned(),
                        name: "mcp__github__search".to_owned(),
                    },
                ];
            }
            events.push(priced_event);
        }
        let mut sub = event("sub-0", NOV_8 + 3 * MIN, tokens(1_000, 50));
        sub.context.session_key = Some("claude-code:s1:a".to_owned());
        sub.context.parent_session_key = Some("claude-code:s1".to_owned());
        sub.context.is_subagent = true;
        sub.attribution.model = Some(ModelId::new("claude-opus-5-5").unwrap());
        sub.attribution.raw_model = Some("claude-opus-5-5".to_owned());
        events.push(priced(sub, &catalog));
        let mut other = event("s2-0", NOV_8 + 60 * MIN, tokens(5_000, 500));
        other.context.session_key = Some("claude-code:s2".to_owned());
        other.context.project_key = Some("/work/other".to_owned());
        events.push(priced(other, &catalog));
        db.store
            .commit(
                &IngestBatch {
                    events,
                    tool_outcomes: vec![ToolOutcome {
                        tool_call_id: "toolu_1".to_owned(),
                        is_error: true,
                    }],
                    checkpoint: None,
                },
                now(),
            )
            .unwrap();
        db
    }

    fn day_query(sort: SessionSort) -> SessionQuery {
        SessionQuery {
            from: UtcMillis(NOV_8),
            until: UtcMillis(NOV_8 + 24 * 60 * MIN),
            filters: Filters::new(),
            sort,
            limit: 50,
            offset: 0,
        }
    }

    #[test]
    fn lists_sessions_with_totals_that_add_up_to_the_rollups() {
        let db = seeded();
        let page = db.store.sessions(&day_query(SessionSort::Tokens)).unwrap();
        assert_eq!(page.total, 3);
        let keys: Vec<&str> = page.sessions.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            ["claude-code:s2", "claude-code:s1:a", "claude-code:s1"]
        );

        let s1 = &page.sessions[2];
        assert_eq!(s1.sums.events, 3);
        assert_eq!(s1.sums.tokens.input_uncached, Some(600));
        assert_eq!((s1.first_at.0, s1.last_at.0), (NOV_8, NOV_8 + 40 * MIN));
        assert_eq!(s1.project.as_deref(), Some("/work/veyra"));
        assert_eq!(s1.models, ["claude-sonnet-5-5"]);
        assert!(page.sessions[1].is_subagent);
        assert_eq!(page.sessions[1].parent_id, Some(s1.id));

        let mut summed = veyra_analytics::sums::BaseSums::default();
        for session in &page.sessions {
            summed.merge(&session.sums);
        }
        assert_eq!(
            summed,
            all_time(&db.store),
            "sessions add up to the rollups"
        );
    }

    #[test]
    fn filters_sorts_and_pages() {
        let db = seeded();
        let mut query = day_query(SessionSort::LastActive);
        query.filters = Filters::new().with(Dimension::Model, [Some("claude-opus-5-5".to_owned())]);
        let page = db.store.sessions(&query).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.sessions[0].key, "claude-code:s1:a");

        let mut query = day_query(SessionSort::LastActive);
        query.limit = 1;
        query.offset = 1;
        let page = db.store.sessions(&query).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.sessions.len(), 1);
        assert_eq!(page.sessions[0].key, "claude-code:s1", "second most recent");
    }

    #[test]
    fn a_session_shows_its_timeline_tools_and_subagents() {
        let db = seeded();
        let id = db
            .store
            .sessions(&day_query(SessionSort::Events))
            .unwrap()
            .sessions[0]
            .id;
        let detail = db.store.session(id).unwrap().unwrap();
        assert_eq!(detail.summary.key, "claude-code:s1");
        assert_eq!(detail.events.len(), 3);
        assert!(detail.events.windows(2).all(|w| w[0].at <= w[1].at));
        let tools = &detail.events[1].tools;
        assert_eq!(tools.len(), 2);
        assert_eq!(
            (tools[0].name.as_str(), tools[0].is_error),
            ("Bash", Some(true))
        );
        assert_eq!(tools[1].mcp_server.as_deref(), Some("github"));
        assert!(detail.events.iter().all(|e| e.cost.is_some()), "all priced");
        assert_eq!(detail.subagents.len(), 1);
        assert_eq!(detail.subagents[0].key, "claude-code:s1:a");
        assert!(db.store.session(999_999).unwrap().is_none());
    }

    #[test]
    fn event_times_feed_session_count_and_active_time() {
        let db = seeded();
        let times = db
            .store
            .event_times(&RangeQuery {
                from: UtcMillis(NOV_8),
                until: UtcMillis(NOV_8 + 24 * 60 * MIN),
                filters: Filters::new(),
            })
            .unwrap();
        assert_eq!(times.len(), 5);
        let result = activity(&times, DEFAULT_IDLE_THRESHOLD_MS);
        assert_eq!(result.sessions, 3);
        // s1: 0→2 (2 min) and 2→40 (capped at 5 min); s1:a and s2 have one event each.
        assert_eq!(result.active_ms, 7 * MIN);
    }

    #[test]
    fn labels_projects() {
        let db = seeded();
        let paths: Vec<String> = db
            .store
            .projects()
            .unwrap()
            .into_iter()
            .map(|p| p.path)
            .collect();
        assert_eq!(paths, ["/work/other", "/work/veyra"]);
    }

    fn day() -> RangeQuery {
        RangeQuery {
            from: UtcMillis(NOV_8),
            until: UtcMillis(NOV_8 + 24 * 60 * MIN),
            filters: Filters::new(),
        }
    }

    fn histogram(rows: &[veyra_analytics::sizes::SizeRow]) -> Vec<(u32, u64)> {
        let mut histogram = SizeHistogram::default();
        for row in rows {
            histogram.add(row.bin, row.requests);
        }
        histogram.bins().collect()
    }

    fn expected(sizes: &[u64]) -> Vec<(u32, u64)> {
        let mut histogram = SizeHistogram::default();
        for size in sizes {
            histogram.add(size_bin(*size), 1);
        }
        histogram.bins().collect()
    }

    #[test]
    fn request_sizes_count_each_request_in_its_size_bin() {
        let db = seeded();
        let all = db.store.request_sizes(&day()).unwrap();
        assert_eq!(histogram(&all), expected(&[110, 210, 310, 1_050, 5_500]));
        let opus = RangeQuery {
            filters: Filters::new().with(Dimension::Model, [Some("claude-opus-5-5".to_owned())]),
            ..day()
        };
        assert_eq!(
            histogram(&db.store.request_sizes(&opus).unwrap()),
            expected(&[1_050])
        );
    }

    #[test]
    fn the_migration_backfill_matches_the_rebuild() {
        let db = seeded();
        let rebuilt = db.store.request_sizes(&day()).unwrap();
        let connection = rusqlite::Connection::open(db.path().join("veyra.db")).unwrap();
        connection
            .create_scalar_function(
                "veyra_size_bin",
                1,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                |context| {
                    let tokens = u64::try_from(context.get::<i64>(0)?).unwrap_or(0);
                    Ok(i64::from(size_bin(tokens)))
                },
            )
            .unwrap();
        let migration = include_str!("../migrations/0004_request_sizes.sql");
        let backfill = &migration[migration.find("INSERT INTO").unwrap()..];
        connection
            .execute_batch(&format!("DELETE FROM rollup_15m_sizes; {backfill}"))
            .unwrap();
        drop(connection);
        assert_eq!(db.store.request_sizes(&day()).unwrap(), rebuilt);
    }

    #[test]
    fn session_sizes_total_each_sessions_tokens() {
        let db = seeded();
        let mut sizes: Vec<(u64, i64)> = db
            .store
            .session_sizes(&day())
            .unwrap()
            .into_iter()
            .map(|s| (s.tokens, s.first_at.0 - NOV_8))
            .collect();
        sizes.sort_unstable();
        assert_eq!(sizes, [(630, 0), (1_050, 3 * MIN), (5_500, 60 * MIN)]);
    }
}
