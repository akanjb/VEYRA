//! Connection setup. WAL lets readers proceed while the single writer commits.

use std::path::Path;
use std::time::Duration;

use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, OpenFlags};
use veyra_analytics::sizes::size_bin;

use crate::StoreError;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Writer page cache in KiB (negative, per SQLite): 16 MiB instead of the default ~2 MiB,
/// so the hot index pages of a backfill stay in memory.
const WRITER_CACHE_KIB: i64 = -16_384;

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
    register_functions(&connection)?;
    Ok(connection)
}

/// SQL name of [`size_bin`], used by the size rollups and their migration.
pub const SIZE_BIN_FUNCTION: &str = "veyra_size_bin";

/// Registers the Rust functions the writer's SQL calls, so each rule has one definition.
pub(crate) fn register_functions(connection: &Connection) -> Result<(), StoreError> {
    connection.create_scalar_function(
        SIZE_BIN_FUNCTION,
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            let tokens = u64::try_from(context.get::<i64>(0)?).unwrap_or(0);
            Ok(i64::from(size_bin(tokens)))
        },
    )?;
    crate::spans::register(connection)?;
    Ok(())
}

pub fn open_reader(path: &Path) -> Result<Connection, StoreError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "query_only", "ON")?;
    // Sorts and groupings stay in memory: temporary files are slow, and on some systems
    // each one is scanned by antivirus software before it can be used.
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(connection)
}
