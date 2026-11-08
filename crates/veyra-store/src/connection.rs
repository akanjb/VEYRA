//! Connection setup. WAL lets readers proceed while the single writer commits.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::StoreError;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Writer page cache in KiB (negative, per SQLite): 16 MiB instead of the default ~2 MiB,
/// so the hot index pages of a backfill stay in memory.
const WRITER_CACHE_KIB: i64 = -16_384;

/// Checkpoint the write-ahead log every 4,000 pages (16 MiB) instead of SQLite's 1,000.
/// An ingest step writes several MiB, so the default checkpointed after nearly every
/// commit; a larger interval lets index pages rewritten by consecutive steps be copied
/// back once.
const WAL_CHECKPOINT_PAGES: i64 = 4_000;

/// Truncate the write-ahead log back to at most 32 MiB after a checkpoint.
const WAL_SIZE_LIMIT_BYTES: i64 = 32 * 1024 * 1024;

pub fn open_writer(path: &Path) -> Result<Connection, StoreError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    connection.pragma_update(None, "cache_size", WRITER_CACHE_KIB)?;
    connection.pragma_update(None, "wal_autocheckpoint", WAL_CHECKPOINT_PAGES)?;
    connection.pragma_update(None, "journal_size_limit", WAL_SIZE_LIMIT_BYTES)?;
    Ok(connection)
}

pub fn open_reader(path: &Path) -> Result<Connection, StoreError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "query_only", "ON")?;
    Ok(connection)
}
