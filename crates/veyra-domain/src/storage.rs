//! Veyra's own storage: how much space the database takes, and its backups. Nothing here
//! touches the files of the tools Veyra reads.

use crate::source::PortError;
use crate::time::UtcMillis;

/// The space one table takes, its indexes included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSize {
    pub table: String,
    pub bytes: u64,
}

/// How much space the database takes and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footprint {
    /// The database file and its write-ahead log.
    pub database_bytes: u64,
    /// Largest first.
    pub tables: Vec<TableSize>,
    pub events: u64,
    pub first_event_at: Option<UtcMillis>,
    pub last_event_at: Option<UtcMillis>,
}

/// A backup file of the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// The file name, which identifies the backup (`veyra-YYYYMMDD-HHMMSS.db`).
    pub name: String,
    pub created_at: UtcMillis,
    pub bytes: u64,
}

/// What a backup holds, shown before restoring it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupContents {
    pub events: u64,
    pub first_event_at: Option<UtcMillis>,
    pub last_event_at: Option<UtcMillis>,
}

/// Why a backup cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackupError {
    #[error("no backup is named `{0}`")]
    NotFound(String),
    #[error("`{0}` is not a Veyra backup this version can restore")]
    Unusable(String),
}

/// The database's space and backups.
pub trait StorageRepository: Send + Sync {
    /// How many backups are kept; older ones are deleted after each new backup.
    fn kept_backups(&self) -> u32;

    /// The database's size, per-table sizes and the events it holds.
    ///
    /// # Errors
    /// Returns [`PortError`] when the database or its file cannot be read.
    fn footprint(&self) -> Result<Footprint, PortError>;

    /// Every backup, newest first.
    ///
    /// # Errors
    /// Returns [`PortError`] when the backup folder cannot be read.
    fn backups(&self) -> Result<Vec<Backup>, PortError>;

    /// Writes a consistent copy of the database now, then keeps only the newest
    /// backups.
    ///
    /// # Errors
    /// Returns [`PortError`] when the copy cannot be written.
    fn create_backup(&self, now: UtcMillis) -> Result<Backup, PortError>;

    /// What the backup named `name` holds.
    ///
    /// # Errors
    /// [`BackupError`] inside [`PortError`] for an unknown or unusable backup.
    fn backup_contents(&self, name: &str) -> Result<BackupContents, PortError>;

    /// Prepares the backup named `name` to replace the database the next time Veyra
    /// starts, before anything opens it.
    ///
    /// # Errors
    /// [`BackupError`] inside [`PortError`] for an unknown or unusable backup, or a
    /// failure to copy it.
    fn schedule_restore(&self, name: &str) -> Result<(), PortError>;
}
