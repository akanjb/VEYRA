//! OpenTelemetry usage pushed by tools to Veyra's local receiver (ADR-038).
//!
//! The receiver decodes the transport format into [`TelemetryRecord`]s; a connector
//! that knows the tool's documented events turns them into usage records. The receiver
//! is off until the user turns it on, and only ever listens on the loopback interface.

use std::collections::HashMap;

use crate::time::UtcMillis;

/// One OpenTelemetry log record, with its resource's service, reduced to scalar
/// attributes. Values are kept as text: integers, doubles and booleans in their decimal
/// or `true`/`false` form. Records are read in memory and never stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TelemetryRecord {
    /// `service.name` of the resource that emitted the record.
    pub service: String,
    /// `service.version` of that resource, when present.
    pub service_version: Option<String>,
    /// The record's own time (`timeUnixNano`), when present.
    pub time: Option<UtcMillis>,
    pub attributes: HashMap<String, String>,
}

impl TelemetryRecord {
    /// The attribute `key` as text.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(String::as_str)
    }

    /// The attribute `key` as a non-negative whole number.
    #[must_use]
    pub fn count(&self, key: &str) -> Option<u64> {
        self.text(key).and_then(|value| value.parse().ok())
    }
}

/// What the receiver is doing now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiverStatus {
    /// Whether it is accepting connections.
    pub listening: bool,
    /// The OTLP/HTTP base address tools send to, e.g. `http://127.0.0.1:4318`.
    pub endpoint: String,
    /// Why it could not start, e.g. the port is taken.
    pub error: Option<String>,
    /// Usage records received since Veyra started.
    pub records: u64,
    pub last_received_at: Option<UtcMillis>,
}

/// Control of the local receiver (implemented by the ingest layer).
pub trait TelemetryReceiver: Send + Sync {
    /// Starts or stops listening; a failure to start is reported by [`status`](Self::status).
    fn set_enabled(&self, enabled: bool);
    fn status(&self) -> ReceiverStatus;
}
