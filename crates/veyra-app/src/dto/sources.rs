use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request for [`crate::Api::list_sources`]. Intentionally empty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ListSourcesRequest {}

/// Every usage source found on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceList {
    pub sources: Vec<SourceSummary>,
    /// Tools Veyra can read that were not found on this device, by name.
    pub not_found: Vec<String>,
}

/// How completely a source reports one kind of information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SupportLevel {
    Full,
    Partial,
    Unsupported,
}

/// One row of a source's capability matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    /// What is reported, e.g. `Tokens`.
    pub label: String,
    pub support: SupportLevel,
    /// The limitation, in plain words, when support is partial.
    pub note: Option<String>,
}

/// A usage source and its ingestion health. Timestamps are UTC epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub connector_id: String,
    /// Name of the tool the source belongs to, e.g. `Claude Code`.
    pub display_name: String,
    /// Directory the source is read from.
    pub root: String,
    /// Sources are read only after the user enables them.
    pub enabled: bool,
    /// Whether the vendor documents the format. Undocumented formats may change without
    /// notice, so the dashboard labels them.
    pub documented_format: bool,
    #[ts(type = "number | null")]
    pub consented_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_scan_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_success_at: Option<i64>,
    /// Why the last scan failed, if it did.
    pub last_error: Option<String>,
    /// Log files being read.
    #[ts(type = "number")]
    pub files_active: u64,
    /// Log files that disappeared; their usage is kept.
    #[ts(type = "number")]
    pub files_missing: u64,
    /// When a file of this source was last read.
    #[ts(type = "number | null")]
    pub last_ingested_at: Option<i64>,
    /// Requests stored from this source's application, all time.
    #[ts(type = "number")]
    pub events: u64,
    /// Problems found in the last 7 days, per kind (never content).
    pub recent_issues: Vec<IssueCount>,
    /// What this source can report.
    pub capabilities: Vec<Capability>,
}

/// How often one kind of problem occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueCount {
    /// Stable identifier, e.g. `malformed-json`.
    pub kind: String,
    #[ts(type = "number")]
    pub occurrences: u64,
}

/// Request for [`crate::Api::set_source_enabled`]. Enabling records the user's consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetSourceEnabledRequest {
    #[ts(type = "number")]
    pub source_id: i64,
    pub enabled: bool,
}

/// Request for [`crate::Api::request_rescan`]. Intentionally empty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RescanRequest {}

/// The rescan was queued; results arrive through the `data-changed` event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RescanAccepted {}

/// Payload of the `data-changed` event: an ingest scan changed stored usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DataChanged {
    #[ts(type = "number")]
    pub inserted: u64,
    #[ts(type = "number")]
    pub updated: u64,
}
