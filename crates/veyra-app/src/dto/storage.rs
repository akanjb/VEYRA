use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request for [`crate::Api::get_storage`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StorageRequest {
    /// IANA time zone the months are counted in.
    pub time_zone: String,
}

/// The space one table takes, its indexes included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TableBytes {
    pub table: String,
    #[ts(type = "number")]
    pub bytes: u64,
}

/// Events recorded in one local month.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MonthEvents {
    /// `YYYY-MM`.
    pub month: String,
    #[ts(type = "number")]
    pub events: u64,
}

/// A backup of the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// Identifies the backup in restore requests.
    pub name: String,
    /// UTC epoch milliseconds.
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub bytes: u64,
}

/// The Storage page: Veyra's own database and its backups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    /// The database file and its write-ahead log.
    #[ts(type = "number")]
    pub database_bytes: u64,
    /// Largest first.
    pub tables: Vec<TableBytes>,
    #[ts(type = "number")]
    pub events: u64,
    #[ts(type = "number | null")]
    pub first_event_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_event_at: Option<i64>,
    /// From the first event's month to the last's, empty months included.
    pub months: Vec<MonthEvents>,
    /// Newest first.
    pub backups: Vec<BackupInfo>,
    /// How many backups are kept; older ones are deleted.
    pub kept_backups: u32,
}

/// Request for [`crate::Api::create_backup`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateBackupRequest {}

/// Request for [`crate::Api::preview_restore`] and [`crate::Api::restore_backup`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    /// A backup name from [`StorageOverview::backups`].
    pub name: String,
}

/// What restoring a backup would change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub backup: BackupInfo,
    /// Events in the backup.
    #[ts(type = "number")]
    pub events: u64,
    #[ts(type = "number | null")]
    pub first_event_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_event_at: Option<i64>,
    /// Events stored now, which the backup replaces.
    #[ts(type = "number")]
    pub current_events: u64,
}

/// A restore is staged; Veyra restarts to apply it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RestoreScheduled {}

/// Request for [`crate::Api::preview_delete`] and [`crate::Api::delete_data`]: which
/// stored usage to delete. Every part left out widens the scope; all left out means
/// everything Veyra stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDataRequest {
    /// IANA time zone the days are in.
    pub time_zone: String,
    /// First local day (`YYYY-MM-DD`), inclusive.
    pub start: Option<String>,
    /// Last local day (`YYYY-MM-DD`), inclusive.
    pub end: Option<String>,
    /// Only usage read by this connector.
    pub connector_id: Option<String>,
}

/// The stored usage a deletion covers, before anything changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeletePreview {
    #[ts(type = "number")]
    pub events: u64,
    #[ts(type = "number | null")]
    pub first_event_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_event_at: Option<i64>,
}

/// Stored usage was deleted after a backup was taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DataDeleted {
    #[ts(type = "number")]
    pub events: u64,
    /// The backup taken just before, which undoes the deletion when restored.
    pub backup: BackupInfo,
}
