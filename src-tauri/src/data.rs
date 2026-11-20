//! Starts the data services: the database in the local app-data directory and the
//! background ingest worker. Failures are attached to the API, which then reports them
//! through every data command instead of preventing the window from opening.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use veyra_app::dto::DataChanged;
use veyra_app::events::DATA_CHANGED;
use veyra_app::{Api, AppControl, DataServices, DisplayNames};
use veyra_domain::time::UtcMillis;
use veyra_ingest::{
    ChangeListener, DEFAULT_PORT, ImportParts, Importer, IngestWorker, PROXY_PORT, Proxy,
    ProxyParts, Receiver, ReceiverParts, WorkerConfig, discovery_env,
};
use veyra_registry::Registry;
use veyra_store::{Storage, Store};

use crate::files::Dialogs;

/// File name of the database inside the local app-data directory.
const DATABASE: &str = "veyra.db";
/// The backup folder inside the local app-data directory (ADR-033).
const BACKUPS: &str = "backups";
/// The first daily backup waits so the startup scan runs first; then it is checked hourly.
const FIRST_BACKUP_DELAY: Duration = Duration::from_mins(10);
const BACKUP_CHECK: Duration = Duration::from_hours(1);

/// Restarts the application (to apply a restored backup).
struct Restart<R: Runtime>(AppHandle<R>);

impl<R: Runtime> AppControl for Restart<R> {
    fn restart(&self) {
        self.0.request_restart();
    }
}

/// Tells the dashboard that stored usage changed. Nothing is listening before the
/// dashboard loads; the next change re-emits.
fn notify_changes<R: Runtime>(app: &AppHandle<R>) -> ChangeListener {
    let emitter = app.clone();
    Box::new(move |report| {
        let payload = DataChanged {
            inserted: report.inserted,
            updated: report.updated,
        };
        let _ = emitter.emit(DATA_CHANGED, payload);
    })
}

/// Takes a backup once a day, in the background.
fn back_up_daily(storage: Arc<Storage>) -> Result<(), String> {
    std::thread::Builder::new()
        .name("veyra-backups".to_owned())
        .spawn(move || {
            std::thread::sleep(FIRST_BACKUP_DELAY);
            loop {
                if let Err(error) = storage.back_up_daily(UtcMillis::now()) {
                    eprintln!("veyra: daily backup failed: {error}");
                }
                std::thread::sleep(BACKUP_CHECK);
            }
        })
        .map(drop)
        .map_err(|e| format!("cannot start daily backups: {e}"))
}

/// Opens the database, starts ingestion and attaches both to the API.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let services = open(app);
    app.state::<Api>().attach(services);
}

fn open<R: Runtime>(app: &AppHandle<R>) -> Result<DataServices, String> {
    // Local, not roaming: the database is per device and can be large.
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("no app-data directory: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let database = dir.join(DATABASE);
    // A backup the user chose to restore replaces the database before anything opens it.
    veyra_store::apply_pending_restore(&database)
        .map_err(|e| format!("cannot apply the restored backup: {e}"))?;
    let store = Arc::new(Store::open(&database).map_err(|e| e.to_string())?);
    let storage = Arc::new(Storage::new(
        Arc::clone(&store),
        database,
        dir.join(BACKUPS),
    ));
    back_up_daily(Arc::clone(&storage))?;
    let registry = Arc::new(Registry::bundled().map_err(|e| e.to_string())?);
    let connectors = veyra_connectors::all();
    let descriptors: Vec<_> = connectors.iter().map(|c| c.descriptor().clone()).collect();
    let applications: Vec<_> = descriptors
        .iter()
        .cloned()
        .chain(veyra_connectors::capture::descriptors())
        .chain(veyra_connectors::exports::descriptors())
        .collect();
    let names = DisplayNames::new(
        registry.provider_names(),
        registry.model_names(),
        &applications,
    );
    let catalog = Arc::new(registry.catalog().clone());
    // Off until the user turns it on; the API applies the kept choice when attached.
    let telemetry = Receiver::new(
        ReceiverParts {
            store: Arc::clone(&store),
            registry: Arc::clone(&registry),
            connectors: veyra_connectors::all(),
            on_change: notify_changes(app),
        },
        DEFAULT_PORT,
    );
    let proxy = Proxy::new(
        ProxyParts {
            store: Arc::clone(&store),
            registry: Arc::clone(&registry),
            routes: veyra_connectors::capture::all(),
            on_change: notify_changes(app),
        },
        PROXY_PORT,
    );
    let importer = Importer::new(ImportParts {
        store: Arc::clone(&store),
        registry: Arc::clone(&registry),
        importers: veyra_connectors::exports::all(),
        on_change: notify_changes(app),
    });
    let worker = IngestWorker {
        store: Arc::clone(&store),
        registry,
        connectors,
        env: discovery_env(),
        config: WorkerConfig::default(),
        on_change: notify_changes(app),
    };
    let ingest = veyra_ingest::spawn(worker).map_err(|e| format!("cannot start ingestion: {e}"))?;
    Ok(DataServices {
        sources: Arc::clone(&store) as _,
        projects: Arc::clone(&store) as _,
        billing: Arc::clone(&store) as _,
        settings: Arc::clone(&store) as _,
        audit: Arc::clone(&store) as _,
        storage,
        analytics: store,
        catalog,
        connectors: descriptors,
        names,
        ingest: Box::new(ingest),
        telemetry: Box::new(telemetry),
        proxy: Box::new(proxy),
        capture_routes: veyra_connectors::capture::all(),
        importer: Box::new(importer),
        files: Box::new(Dialogs { app: app.clone() }),
        picker: Box::new(Dialogs { app: app.clone() }),
        control: Box::new(Restart(app.clone())),
    })
}
