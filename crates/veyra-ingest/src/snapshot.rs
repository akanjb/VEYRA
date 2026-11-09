//! Reading live SQLite databases owned by other applications (ADR-015).
//!
//! The database is opened read-only and copied in one step into a private temporary
//! snapshot with SQLite's online backup API; in WAL mode this never blocks the owning
//! application's writers. The connector's queries run against the snapshot, which is
//! deleted straight afterwards because it holds everything the application stores,
//! including content Veyra never reads.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Map, Value};
use veyra_domain::connector::DatabaseQuery;

use crate::IngestError;

/// Every row of every applicable query, one JSON object line per row (column name to
/// value; binary columns are left out).
///
/// # Errors
/// Returns [`IngestError::Snapshot`] when the database cannot be opened, copied or
/// queried.
pub fn database_rows(path: &Path, queries: &[DatabaseQuery]) -> Result<Vec<u8>, IngestError> {
    database_rows_in(path, queries, &std::env::temp_dir())
}

fn database_rows_in(
    path: &Path,
    queries: &[DatabaseQuery],
    scratch: &Path,
) -> Result<Vec<u8>, IngestError> {
    let failed = |error: rusqlite::Error| IngestError::Snapshot {
        path: path.into(),
        message: error.to_string(),
    };
    let dir = SnapshotDir::create(scratch).map_err(IngestError::io(scratch))?;
    let snapshot = copy(path, &dir.path().join("snapshot.db")).map_err(failed)?;
    let mut out = Vec::new();
    for query in queries {
        if has_tables(&snapshot, query.tables).map_err(failed)? {
            write_rows(&snapshot, query.sql, &mut out).map_err(failed)?;
        }
    }
    Ok(out)
}

/// Copies the live database at `from` into a new database at `to`, in one backup step.
fn copy(from: &Path, to: &Path) -> rusqlite::Result<Connection> {
    let source = Connection::open_with_flags(
        from,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut snapshot = Connection::open(to)?;
    let step = Backup::new(&source, &mut snapshot)?.step(-1)?;
    match step {
        StepResult::Done => Ok(snapshot),
        // The source stayed busy or locked; the next scan retries.
        _ => Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("the database was busy".to_owned()),
        )),
    }
}

/// Whether every table exists (each version of an application has its own set).
fn has_tables(db: &Connection, tables: &[&str]) -> rusqlite::Result<bool> {
    let mut statement =
        db.prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")?;
    for table in tables {
        if !statement.exists([table])? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Appends each result row of `sql` as one JSON object line.
fn write_rows(db: &Connection, sql: &str, out: &mut Vec<u8>) -> rusqlite::Result<()> {
    let mut statement = db.prepare(sql)?;
    let names: Vec<String> = statement.column_names().map(str::to_owned).collect();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            if let Some(value) = json_value(row.get_ref(index)?) {
                object.insert(name.clone(), value);
            }
        }
        // Serialising a map of plain JSON values cannot fail.
        let _ = serde_json::to_writer(&mut *out, &Value::Object(object));
        out.push(b'\n');
    }
    Ok(())
}

/// A column value as JSON; `None` for binary data, which no parser needs.
fn json_value(value: ValueRef<'_>) -> Option<Value> {
    match value {
        ValueRef::Null => Some(Value::Null),
        ValueRef::Integer(n) => Some(Value::from(n)),
        ValueRef::Real(x) => {
            Some(serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number))
        }
        ValueRef::Text(text) => Some(Value::String(String::from_utf8_lossy(text).into_owned())),
        ValueRef::Blob(_) => None,
    }
}

/// A private directory for one snapshot, removed with its contents when dropped.
struct SnapshotDir(PathBuf);

impl SnapshotDir {
    fn create(parent: &Path) -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            "veyra-snapshot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = parent.join(name);
        // `create_dir` fails if the directory exists, so a snapshot never reuses one.
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for SnapshotDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests;
