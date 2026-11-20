//! Shared harness of the application's integration tests: the Claude Code fixtures
//! ingested through the real pipeline, and an API over them.

#![allow(dead_code, reason = "each test crate uses part of the harness")]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use jiff::tz::TimeZone;
use veyra_analytics::dimension::Filters;
use veyra_analytics::engine::evaluate;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_app::dto::{Comparison, Scope, ScopeFilters};
use veyra_app::{Api, AppControl, AppMetadata, DataServices, DisplayNames, FilePicker, FileSaver};
use veyra_domain::capture::{CaptureProxy, ProxyStatus};
use veyra_domain::import::ReadSeek;
use veyra_domain::source::{DiscoveryEnv, IngestControl};
use veyra_domain::telemetry::{ReceiverStatus, TelemetryReceiver};
use veyra_ingest::{DEFAULT_CHUNK_BYTES, ImportParts, Importer, IngestContext, discover, scan_all};
use veyra_registry::Registry;
use veyra_store::{Storage, Store};
use veyra_testkit::TempDir;

struct NoIngest;

impl IngestControl for NoIngest {
    fn request_scan(&self) {}
    fn sources_changed(&self) {}
}

/// No receiver runs in these tests.
struct NoReceiver;

impl TelemetryReceiver for NoReceiver {
    fn set_enabled(&self, _: bool) {}
    fn status(&self) -> ReceiverStatus {
        ReceiverStatus::default()
    }
}

/// No proxy runs in these tests.
struct NoProxy;

impl CaptureProxy for NoProxy {
    fn set_enabled(&self, _: bool) {}
    fn status(&self) -> ProxyStatus {
        ProxyStatus::default()
    }
}

/// Files "saved" by exports, as (suggested name, bytes), newest last.
pub type SavedFiles = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// Saves to memory, as if the user accepted the suggested name.
struct MemoryFiles(SavedFiles);

impl FileSaver for MemoryFiles {
    fn save(&self, suggested_name: &str, _: &str, bytes: &[u8]) -> Result<bool, String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .push((suggested_name.to_owned(), bytes.to_vec()));
        Ok(true)
    }
}

/// Files the user will choose in the open dialog, first chosen first; an empty queue
/// is a cancelled dialog.
pub type Picks = Arc<Mutex<Vec<Vec<u8>>>>;

struct QueuedPicks(Picks);

impl FilePicker for QueuedPicks {
    fn pick(&self, _: &str, _: &[&str]) -> Result<Option<Box<dyn ReadSeek>>, String> {
        let mut queue = self.0.lock().map_err(|e| e.to_string())?;
        Ok((!queue.is_empty())
            .then(|| Box::new(std::io::Cursor::new(queue.remove(0))) as Box<dyn ReadSeek>))
    }
}

/// A database holding the ingested fixtures, removed on drop.
pub struct Ingested {
    dir: TempDir,
    pub store: Arc<Store>,
}

impl Ingested {
    pub fn database(&self) -> std::path::PathBuf {
        self.dir.path().join("veyra.db")
    }

    pub fn backups(&self) -> std::path::PathBuf {
        self.dir.path().join("backups")
    }
}

/// Counts restart requests instead of restarting.
pub struct Restarts(pub Arc<AtomicUsize>);

impl AppControl for Restarts {
    fn restart(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
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
    Ingested { dir, store }
}

pub fn api(data: &Ingested) -> Api {
    api_with_files(data).0
}

/// An API over the fixtures whose exports are kept in memory.
pub fn api_with_files(data: &Ingested) -> (Api, SavedFiles) {
    let (api, files, _) = api_with_restarts(data);
    (api, files)
}

/// An API over the fixtures, with its saved exports and restart requests.
pub fn api_with_restarts(data: &Ingested) -> (Api, SavedFiles, Arc<AtomicUsize>) {
    let (api, files, restarts, _) = build(data);
    (api, files, restarts)
}

/// An API over the fixtures that imports the files queued in its picks.
pub fn api_with_picks(data: &Ingested) -> (Api, Picks) {
    let (api, _, _, picks) = build(data);
    (api, picks)
}

fn build(data: &Ingested) -> (Api, SavedFiles, Arc<AtomicUsize>, Picks) {
    let store = &data.store;
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
    let files = SavedFiles::default();
    let picks = Picks::default();
    let importer = Importer::new(ImportParts {
        store: Arc::clone(store),
        registry: Arc::new(Registry::bundled().unwrap()),
        importers: veyra_connectors::exports::all(),
        on_change: Box::new(|_| {}),
    });
    let restarts = Arc::new(AtomicUsize::new(0));
    let storage = Storage::new(Arc::clone(store), data.database(), data.backups());
    assert!(api.attach(Ok(DataServices {
        sources: Arc::clone(store) as _,
        projects: Arc::clone(store) as _,
        billing: Arc::clone(store) as _,
        settings: Arc::clone(store) as _,
        audit: Arc::clone(store) as _,
        storage: Arc::new(storage),
        analytics: Arc::clone(store) as _,
        catalog: Arc::new(registry.catalog().clone()),
        connectors,
        names,
        ingest: Box::new(NoIngest),
        telemetry: Box::new(NoReceiver),
        proxy: Box::new(NoProxy),
        capture_routes: Vec::new(),
        importer: Box::new(importer),
        files: Box::new(MemoryFiles(Arc::clone(&files))),
        picker: Box::new(QueuedPicks(Arc::clone(&picks))),
        control: Box::new(Restarts(Arc::clone(&restarts))),
    })));
    (api, files, restarts, picks)
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
