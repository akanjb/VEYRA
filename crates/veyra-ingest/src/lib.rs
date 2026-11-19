//! Veyra background ingestion (ADR-023).
//!
//! Finds usage sources, reads them read-only (new bytes of append-only logs, whole files,
//! or snapshots of live databases), hands them to
//! connectors, normalises and prices the records, and commits them to the store together
//! with each file's checkpoint. Nothing is read until the user enables a source.
//! Tools can also push usage to an opt-in local OpenTelemetry receiver (ADR-038).

mod discovery;
mod error;
mod fs;
mod http;
mod normalize;
mod otlp;
mod pipeline;
mod proxy;
mod push;
mod receiver;
mod snapshot;
mod tail;
mod whole;
mod worker;

pub use discovery::{discover, discovery_env};
pub use error::IngestError;
pub use fs::{FoundFile, ScopedFs};
pub use normalize::{NormalizeError, normalize, project_key};
pub use pipeline::{DEFAULT_CHUNK_BYTES, IngestContext, ScanReport, scan_all, scan_source};
pub use proxy::{MAX_CONNECTIONS, PROXY_PORT, Proxy, ProxyParts};
pub use receiver::{DEFAULT_PORT, Receiver, ReceiverParts};
pub use worker::{ChangeListener, IngestHandle, IngestWorker, WorkerConfig, spawn};
