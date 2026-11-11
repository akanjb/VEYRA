//! Shared harness of the application's integration tests: the Claude Code fixtures
//! ingested through the real pipeline, and an API over them.

#![allow(dead_code, reason = "each test crate uses part of the harness")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use jiff::tz::TimeZone;
use veyra_analytics::dimension::Filters;
use veyra_analytics::engine::evaluate;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_app::dto::{Comparison, Scope, ScopeFilters};
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
pub struct Ingested {
    _dir: TempDir,
    pub store: Arc<Store>,
}

pub fn ingested() -> Ingested {
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

pub fn api(store: &Arc<Store>) -> Api {
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
        projects: Arc::clone(store) as _,
        billing: Arc::clone(store) as _,
        analytics: Arc::clone(store) as _,
        catalog: Arc::new(registry.catalog().clone()),
        connectors,
        names,
        ingest: Box::new(NoIngest),
    })));
    api
}

/// The fixture day, 2026-11-08, in UTC.
pub fn scope() -> Scope {
    Scope {
        time_zone: "UTC".to_owned(),
        start: "2026-11-08".to_owned(),
        end: "2026-11-08".to_owned(),
        comparison: Comparison::Previous,
        filters: ScopeFilters::default(),
    }
}

pub fn engine_value(store: &Store, metric: MetricId) -> Option<f64> {
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
