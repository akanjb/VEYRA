use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::Scope;

/// What a report lists, one row per group (PRD FR-EXP-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ExportPreset {
    /// Usage per application.
    UsageSummary,
    /// API-equivalent cost per provider, and the subscription spend and charges the user
    /// recorded, each in its own cost kind.
    CostSummary,
    ByProject,
    ByModel,
    Daily,
    Weekly,
    Monthly,
}

/// The file a report is written as (PRD FR-EXP-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// RFC 4180 CSV in UTF-8.
    Csv,
    /// CSV with a byte-order mark, so Excel reads it as UTF-8.
    CsvExcel,
    Json,
    /// An Excel workbook with one sheet.
    Xlsx,
}

/// What a report leaves out or disguises (PRD FR-EXP-03). Reports never contain session
/// titles or device names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportRedaction {
    /// Replaces each project's name with a hash that is stable on this device.
    pub hash_project_names: bool,
    /// Leaves out project folder paths.
    pub drop_paths: bool,
}

/// Request for [`crate::Api::preview_export`] and [`crate::Api::save_export`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub scope: Scope,
    pub preset: ExportPreset,
    pub format: ExportFormat,
    pub redaction: ExportRedaction,
}

/// The first rows of a report, as they will be written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Rows in the whole report.
    pub row_count: u32,
    /// The file name the save dialog suggests.
    pub file_name: String,
}

/// The outcome of [`crate::Api::save_export`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportSaved {
    /// `false` when the user closed the save dialog without choosing a file.
    pub saved: bool,
    pub row_count: u32,
}
