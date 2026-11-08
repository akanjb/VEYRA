//! Starts the data services: the database in the local app-data directory and the
//! background ingest worker. Failures are attached to the API, which then reports them
//! through every data command instead of preventing the window from opening.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use veyra_app::dto::DataChanged;
use veyra_app::events::DATA_CHANGED;
use veyra_app::{Api, DataServices, DisplayNames};
use veyra_ingest::{IngestWorker, WorkerConfig, discovery_env};
use veyra_registry::Registry;
use veyra_store::Store;

/// File name of the database inside the local app-data directory.
const DATABASE: &str = "veyra.db";

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
    let store = Arc::new(Store::open(&dir.join(DATABASE)).map_err(|e| e.to_string())?);
    let registry = Arc::new(Registry::bundled().map_err(|e| e.to_string())?);
    let connectors = veyra_connectors::all();
    let descriptors: Vec<_> = connectors.iter().map(|c| c.descriptor().clone()).collect();
    let names = DisplayNames::new(
        registry.provider_names(),
        registry.model_names(),
        &descriptors,
    );
    let emitter = app.clone();
    let worker = IngestWorker {
        store: Arc::clone(&store),
        registry,
        connectors,
        env: discovery_env(),
        config: WorkerConfig::default(),
        on_change: Box::new(move |report| {
            let payload = DataChanged {
                inserted: report.inserted,
                updated: report.updated,
            };
            // Nothing is listening before the dashboard loads; the next scan re-emits.
            let _ = emitter.emit(DATA_CHANGED, payload);
        }),
    };
    let ingest = veyra_ingest::spawn(worker).map_err(|e| format!("cannot start ingestion: {e}"))?;
    Ok(DataServices {
        sources: Arc::clone(&store) as _,
        analytics: store,
        connectors: descriptors,
        names,
        ingest: Box::new(ingest),
    })
}
