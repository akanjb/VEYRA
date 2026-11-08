use veyra_domain::event::EventError;

/// Error raised by the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration {version} failed: {source}")]
    Migration {
        version: u32,
        #[source]
        source: rusqlite::Error,
    },
    #[error(
        "database schema version {found} is newer than this build supports ({supported}); update Veyra"
    )]
    SchemaTooNew { found: u32, supported: u32 },
    #[error("event `{key}` is invalid: {source}")]
    InvalidEvent {
        key: String,
        #[source]
        source: EventError,
    },
    #[error("{0} is out of the storable range")]
    OutOfRange(&'static str),
    #[error("stored data is inconsistent: {0}")]
    Corrupt(String),
    #[error("a database connection lock was poisoned by an earlier panic")]
    Poisoned,
}
