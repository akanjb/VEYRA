use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request for [`crate::Api::import_export`]. Intentionally empty: the file is chosen in
/// the operating system's dialog, never named by the dashboard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ImportExportRequest {}

/// What importing a chat app's data export found (ADR-040).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportImported {
    /// `false` when the user cancelled the dialog; the counts are then zero.
    pub imported: bool,
    /// The application whose export it was, e.g. `ChatGPT`.
    pub app: Option<String>,
    #[ts(type = "number")]
    pub conversations: u64,
    /// Conversations whose shape was not understood.
    #[ts(type = "number")]
    pub skipped: u64,
    /// Model replies found, one usage record each.
    #[ts(type = "number")]
    pub replies: u64,
    /// Records new to Veyra.
    #[ts(type = "number")]
    pub added: u64,
    /// Records stored by an earlier import whose counts changed.
    #[ts(type = "number")]
    pub updated: u64,
    /// Records already stored by an earlier import.
    #[ts(type = "number")]
    pub unchanged: u64,
}
