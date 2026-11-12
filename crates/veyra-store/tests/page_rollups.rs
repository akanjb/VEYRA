//! The page rollups of ADR-030 give exactly what the raw events give.
//!
//! A query over whole buckets with filters on the rollup's keys is read from the rollup;
//! the same query ending one millisecond earlier selects the same events (all requests
//! fall on whole seconds) but is read from the events. Every read is compared both ways
//! on random data, after late writes and after a project merge, and the migration's
//! backfill is checked against the incremental rebuild.

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::port::{
    ActivityReader, BaseSumsQuery, BaseSumsReader, PricingReader, RangeQuery, SizeReader, TokenRow,
};
use veyra_analytics::sessions::{SessionQuery, SessionReader, SessionSort};
use veyra_domain::identity::{AppId, ModelId};
use veyra_domain::project::ProjectRepository;
use veyra_domain::time::UtcMillis;
use veyra_store::{PricedEvent, Store};

use support::{HOUR, NOV_8, Rng, TempDb, bundled_catalog, event, priced, tokens};

const SPAN_HOURS: i64 = 6;
const SORTS: [SessionSort; 6] = [
    SessionSort::LastActive,
    SessionSort::Tokens,
    SessionSort::Cost,
    SessionSort::Events,
    SessionSort::CacheWrites,
    SessionSort::Duration,
];

/// Requests of eight sessions over six hours, at whole seconds. Sessions mostly keep one
/// project and application, but sometimes switch within a bucket, and models vary.
fn random_events(rng: &mut Rng, prefix: &str, count: usize) -> Vec<PricedEvent> {
    let catalog = bundled_catalog();
    let models = ["claude-sonnet-5-5", "claude-opus-5-5", "gpt-6.1-sol"];
    (0..count)
        .map(|i| {
            let session = rng.below(8);
            let second =
                i64::try_from(rng.below(u64::try_from(SPAN_HOURS * 3_600).unwrap())).unwrap();
            let mut e = event(
                &format!("{prefix}{i}"),
                NOV_8 + second * 1_000,
                tokens(rng.below(20_000), rng.below(3_000)),
            );
            e.context.session_key = Some(format!("claude-code:r{session}"));
            let project = if rng.chance(15) { 9 } else { session % 3 };
            e.context.project_key = Some(format!("/p/{project}"));
            if rng.chance(10) {
                e.attribution.app = AppId::new("codex").unwrap();
            }
            e.attribution.model = if rng.chance(10) {
                None
            } else {
                Some(ModelId::new(*rng.pick(&models)).unwrap())
            };
            priced(e, &catalog)
        })
        .collect()
}

/// The same events, read from the rollup and from the events.
fn both(filters: &Filters) -> (RangeQuery, RangeQuery) {
    let rollup = RangeQuery {
        from: UtcMillis(NOV_8),
        until: UtcMillis(NOV_8 + SPAN_HOURS * HOUR),
        filters: filters.clone(),
    };
    let events = RangeQuery {
        until: UtcMillis(rollup.until.0 - 1),
        ..rollup.clone()
    };
    (rollup, events)
}

fn page(range: &RangeQuery, sort: SessionSort, offset: u32) -> SessionQuery {
    SessionQuery {
        from: range.from,
        until: range.until,
        filters: range.filters.clone(),
        sort,
        limit: 4,
        offset,
    }
}

fn sorted<T: std::fmt::Debug>(mut items: Vec<T>) -> Vec<T> {
    items.sort_by_key(|item| format!("{item:?}"));
    items
}

/// Every read agrees between the rollup and the events, for several filters.
fn assert_rollups_match_events(store: &Store) {
    let project = store
        .list_projects()
        .unwrap()
        .into_iter()
        .find(|p| p.path == "/p/1")
        .unwrap()
        .id;
    for filters in [
        Filters::new(),
        Filters::new().with(Dimension::Project, [Some(project.to_string())]),
        Filters::new().with(Dimension::App, [Some("codex".to_owned())]),
        Filters::new().with(Dimension::Model, [Some("claude-opus-5-5".to_owned()), None]),
    ] {
        let (rollup, events) = both(&filters);
        assert_eq!(store.spans(&rollup).unwrap(), store.spans(&events).unwrap());
        assert_eq!(
            store.project_spans(&rollup).unwrap(),
            store.project_spans(&events).unwrap()
        );
        assert_eq!(
            store.session_spans(&rollup, &[1, 3]).unwrap(),
            store.session_spans(&events, &[1, 3]).unwrap()
        );
        assert_eq!(
            sorted(store.session_sizes(&rollup).unwrap()),
            sorted(store.session_sizes(&events).unwrap())
        );
        assert_eq!(
            sorted(store.pricing_uses(&rollup).unwrap()),
            sorted(store.pricing_uses(&events).unwrap())
        );
        for sort in SORTS {
            for offset in [0, 3] {
                assert_eq!(
                    store.sessions(&page(&rollup, sort, offset)).unwrap(),
                    store.sessions(&page(&events, sort, offset)).unwrap(),
                    "{sort:?} from {offset} with {filters:?}"
                );
            }
        }
    }
}

#[test]
fn every_page_read_matches_the_events() {
    let db = TempDb::new();
    let mut rng = Rng::new(0x0bad_5eed);
    db.store
        .write_events(&random_events(&mut rng, "a", 600), UtcMillis(NOV_8))
        .unwrap();
    // The data holds sessions that switch project or application within a bucket.
    let connection = rusqlite::Connection::open(db.path().join("veyra.db")).unwrap();
    let mixed: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (SELECT 1 FROM rollup_15m_sessions \
             GROUP BY session_id, bucket_start HAVING COUNT(*) > 1)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(connection);
    assert!(mixed > 0);
    assert_rollups_match_events(&db.store);

    // Late requests land in buckets already rolled up.
    db.store
        .write_events(&random_events(&mut rng, "b", 200), UtcMillis(NOV_8))
        .unwrap();
    assert_rollups_match_events(&db.store);

    // Merging moves usage between projects, and the rollups follow.
    let projects = db.store.list_projects().unwrap();
    let id = |path: &str| projects.iter().find(|p| p.path == path).unwrap().id;
    db.store.merge_project(id("/p/9"), id("/p/1")).unwrap();
    assert_rollups_match_events(&db.store);
}

#[test]
fn the_migration_fills_the_page_rollups_like_the_rebuild() {
    let db = TempDb::new();
    let mut rng = Rng::new(0x5ca1_ab1e);
    db.store
        .write_events(&random_events(&mut rng, "m", 400), UtcMillis(NOV_8))
        .unwrap();
    let (rollup, _) = both(&Filters::new());
    let before = (
        db.store.spans(&rollup).unwrap(),
        db.store.pricing_uses(&rollup).unwrap(),
        db.store
            .sessions(&page(&rollup, SessionSort::Cost, 0))
            .unwrap(),
    );

    // Back to the schema before the page rollups, then reopen: the migration runs again.
    let path = db.path().join("veyra.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "DROP TABLE rollup_15m_sessions; DROP TABLE rollup_15m_services; \
             DROP TABLE rollup_15m_pricing; DROP TABLE rollup_15m_size_totals; \
             PRAGMA user_version = 6;",
        )
        .unwrap();
    drop(connection);
    let store = Store::open(&path).unwrap();
    let after = (
        store.spans(&rollup).unwrap(),
        store.pricing_uses(&rollup).unwrap(),
        store
            .sessions(&page(&rollup, SessionSort::Cost, 0))
            .unwrap(),
    );
    assert_eq!(before, after);
}

#[test]
fn request_size_totals_match_the_sizes_by_dimension() {
    let db = TempDb::new();
    let mut rng = Rng::new(0x51ce_0f00);
    db.store
        .write_events(&random_events(&mut rng, "z", 500), UtcMillis(NOV_8))
        .unwrap();
    let (unfiltered, _) = both(&Filters::new());
    // A filter that keeps every application reads the per-dimension sizes instead.
    let (every_app, _) = both(&Filters::new().with(
        Dimension::App,
        [
            Some("claude-code".to_owned()),
            Some("codex".to_owned()),
            None,
        ],
    ));
    let totals = db.store.request_sizes(&unfiltered).unwrap();
    assert_ne!(totals.len(), 0);
    assert_eq!(totals, db.store.request_sizes(&every_app).unwrap());
}

#[test]
fn token_reads_equal_the_total_tokens_of_full_sums() {
    let db = TempDb::new();
    let mut rng = Rng::new(0x70ce_0005);
    db.store
        .write_events(&random_events(&mut rng, "t", 400), UtcMillis(NOV_8))
        .unwrap();
    let (range, _) = both(&Filters::new());
    let codex = Filters::new().with(Dimension::App, [Some("codex".to_owned())]);
    // Totals, one dimension, services and every combination, by bucket and in total.
    for (group_by, filters) in [
        (vec![], Filters::new()),
        (vec![Dimension::Model], Filters::new()),
        (vec![Dimension::Provider, Dimension::App], codex.clone()),
        (vec![Dimension::Project, Dimension::Model], codex),
    ] {
        for by_bucket in [false, true] {
            let query = BaseSumsQuery {
                from: range.from,
                until: range.until,
                filters: filters.clone(),
                group_by: group_by.clone(),
                by_bucket,
            };
            let expected: Vec<TokenRow> = db
                .store
                .read_base_sums(&query)
                .unwrap()
                .into_iter()
                .map(|row| TokenRow {
                    tokens: match MetricId::TotalTokens.compute(&row.sums) {
                        Some(MetricValue::Count(n)) => n,
                        _ => 0,
                    },
                    bucket_start: row.bucket_start,
                    key: row.key,
                })
                .collect();
            assert_eq!(
                sorted(db.store.read_tokens(&query).unwrap()),
                sorted(expected),
                "{group_by:?} by bucket: {by_bucket}"
            );
        }
    }
}
