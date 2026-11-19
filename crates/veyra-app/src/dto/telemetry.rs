use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request for [`crate::Api::get_telemetry_receiver`]. Intentionally empty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TelemetryReceiverRequest {}

/// Request for [`crate::Api::set_telemetry_receiver`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetTelemetryReceiverRequest {
    pub enabled: bool,
}

/// The local OpenTelemetry receiver (ADR-038).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryReceiverInfo {
    /// Whether the user turned it on (kept across restarts).
    pub enabled: bool,
    /// Whether it is accepting connections now.
    pub listening: bool,
    /// The OTLP/HTTP base address tools send to.
    pub endpoint: String,
    /// Why it is not listening although turned on, e.g. the port is taken.
    pub error: Option<String>,
    /// Exports accepted since Veyra started, whether or not they carried usage.
    #[ts(type = "number")]
    pub exports: u64,
    /// Usage records received since Veyra started.
    #[ts(type = "number")]
    pub records: u64,
    /// UTC epoch milliseconds.
    #[ts(type = "number | null")]
    pub last_received_at: Option<i64>,
    /// Why the latest export was refused, until one is accepted.
    pub last_refusal: Option<String>,
}
