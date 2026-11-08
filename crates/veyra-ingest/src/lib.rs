//! Veyra background ingestion (ADR-023).
//!
//! Finds usage sources, reads their new bytes incrementally and read-only, hands them to
//! connectors, normalises and prices the records, and commits them to the store together
//! with each file's checkpoint. Nothing is read until the user enables a source.

mod discovery;
mod error;
mod fs;
mod normalize;
mod pipeline;
mod tail;
mod worker;

pub use discovery::{discover, discovery_env};
pub use error::IngestError;
pub use fs::{FoundFile, ScopedFs};
pub use normalize::{NormalizeError, normalize, project_key};
pub use pipeline::{DEFAULT_CHUNK_BYTES, IngestContext, ScanReport, scan_all, scan_source};
pub use worker::{ChangeListener, IngestHandle, IngestWorker, WorkerConfig, spawn};

/// The current time as UTC milliseconds.
#[must_use]
pub fn now() -> veyra_domain::time::UtcMillis {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    veyra_domain::time::UtcMillis(millis)
}
