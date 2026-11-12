//! The database's footprint, its backups and restoring one (ADR-033).
//!
//! Backups are `VACUUM INTO` copies in Veyra's own backup folder, named after the moment
//! they were taken (`veyra-<UTC milliseconds>.db`). A name from outside is used only after
//! it is checked against that pattern, so it can never point elsewhere. A restore is
//! staged next to the database and applied by [`apply_pending_restore`] the next time
//! Veyra starts, before any connection opens the database.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use veyra_domain::source::PortError;
use veyra_domain::storage::{
    Backup, BackupContents, BackupError, Footprint, StorageRepository, TableSize,
};
use veyra_domain::time::UtcMillis;

use crate::migrations::LATEST_SCHEMA_VERSION;
use crate::{Store, StoreError};

/// Backups kept; older ones are deleted after each new backup.
pub const KEPT_BACKUPS: usize = 7;

const PREFIX: &str = "veyra-";
const EXTENSION: &str = ".db";

/// The database file of a Veyra installation and its backup folder.
pub struct Storage {
    store: Arc<Store>,
    database: PathBuf,
    backups: PathBuf,
}

impl Storage {
    /// Storage for the database at `database`, opened as `store`, with backups in
    /// `backups` (created when needed).
    #[must_use]
    pub fn new(store: Arc<Store>, database: PathBuf, backups: PathBuf) -> Self {
        Self {
            store,
            database,
            backups,
        }
    }

    /// Takes a backup unless the newest is less than a day old. Returns it when taken.
    ///
    /// # Errors
    /// Returns [`PortError`] when the backup folder or the copy fails.
    pub fn back_up_daily(&self, now: UtcMillis) -> Result<Option<Backup>, PortError> {
        const DAY: i64 = 86_400_000;
        let newest = self.backups()?.into_iter().next();
        if newest.is_some_and(|b| now.0 - b.created_at.0 < DAY) {
            return Ok(None);
        }
        self.create_backup(now).map(Some)
    }

    /// The path of an existing backup with this name, refusing anything that is not a
    /// backup name.
    fn backup_path(&self, name: &str) -> Result<PathBuf, BackupError> {
        let path = self.backups.join(name);
        if created_at(name).is_none() || !path.is_file() {
            return Err(BackupError::NotFound(name.to_owned()));
        }
        Ok(path)
    }
}

/// When the backup named `name` was taken, if `name` is a backup name.
fn created_at(name: &str) -> Option<UtcMillis> {
    let digits = name.strip_prefix(PREFIX)?.strip_suffix(EXTENSION)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(UtcMillis)
}

fn file_bytes(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |m| m.len())
}

/// `<database>-wal`, `<database>-shm` or another file beside the database.
fn sibling(database: &Path, suffix: &str) -> PathBuf {
    let mut name = database.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Where a restore waits for the next start.
fn staged(database: &Path) -> PathBuf {
    sibling(database, ".restore")
}

/// Events and the first and last of them. Reads the totals rollup and the bucket
/// index, so it stays fast on large databases.
fn contents(connection: &Connection) -> Result<BackupContents, StoreError> {
    let events: i64 = connection.query_row(
        "SELECT COALESCE(SUM(events), 0) FROM rollup_15m_totals",
        [],
        |row| row.get(0),
    )?;
    let edge = |order: &str| -> Result<Option<UtcMillis>, StoreError> {
        Ok(connection
            .query_row(
                &format!(
                    "SELECT {order}(occurred_at) FROM usage_events WHERE bucket_start = \
                     (SELECT {order}(bucket_start) FROM usage_events)"
                ),
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .map(UtcMillis))
    };
    Ok(BackupContents {
        events: u64::try_from(events).map_err(|_| StoreError::Corrupt("negative events".into()))?,
        first_event_at: edge("MIN")?,
        last_event_at: edge("MAX")?,
    })
}

/// What a backup file holds, if it is a Veyra database this build can open.
fn backup_contents(path: &Path, name: &str) -> Result<BackupContents, PortError> {
    let unusable = || BackupError::Unusable(name.to_owned());
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| unusable())?;
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| unusable())?;
    if version == 0 || version > LATEST_SCHEMA_VERSION {
        return Err(unusable().into());
    }
    Ok(contents(&connection).map_err(|_| unusable())?)
}

impl StorageRepository for Storage {
    fn kept_backups(&self) -> u32 {
        u32::try_from(KEPT_BACKUPS).unwrap_or(u32::MAX)
    }

    fn footprint(&self) -> Result<Footprint, PortError> {
        let (tables, held) = self.store.with_reader(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT m.tbl_name, SUM(d.pgsize) FROM dbstat('main', 1) AS d \
                 JOIN sqlite_schema AS m ON m.name = d.name \
                 GROUP BY m.tbl_name ORDER BY 2 DESC, 1",
            )?;
            let tables = statement
                .query_map([], |row| {
                    Ok(TableSize {
                        table: row.get(0)?,
                        bytes: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok((tables, contents(connection)?))
        })?;
        Ok(Footprint {
            database_bytes: file_bytes(&self.database)
                + file_bytes(&sibling(&self.database, "-wal")),
            tables,
            events: held.events,
            first_event_at: held.first_event_at,
            last_event_at: held.last_event_at,
        })
    }

    fn backups(&self) -> Result<Vec<Backup>, PortError> {
        let entries = match fs::read_dir(&self.backups) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut backups = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let (Some(created_at), true) = (created_at(&name), entry.file_type()?.is_file()) {
                backups.push(Backup {
                    bytes: entry.metadata()?.len(),
                    name,
                    created_at,
                });
            }
        }
        backups.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(backups)
    }

    fn create_backup(&self, now: UtcMillis) -> Result<Backup, PortError> {
        fs::create_dir_all(&self.backups)?;
        let name = format!("{PREFIX}{}{EXTENSION}", now.0);
        let path = self.backups.join(&name);
        // Written under another name and renamed, so a listed backup is always complete.
        let partial = sibling(&path, ".partial");
        let _ = fs::remove_file(&partial);
        self.store.backup_to(&partial)?;
        fs::rename(&partial, &path)?;
        for old in self.backups()?.into_iter().skip(KEPT_BACKUPS) {
            fs::remove_file(self.backups.join(&old.name))?;
        }
        Ok(Backup {
            bytes: file_bytes(&path),
            name,
            created_at: now,
        })
    }

    fn backup_contents(&self, name: &str) -> Result<BackupContents, PortError> {
        backup_contents(&self.backup_path(name)?, name)
    }

    fn schedule_restore(&self, name: &str) -> Result<(), PortError> {
        let path = self.backup_path(name)?;
        backup_contents(&path, name)?;
        let target = staged(&self.database);
        let partial = sibling(&target, ".partial");
        fs::copy(&path, &partial)?;
        fs::rename(&partial, &target)?;
        Ok(())
    }
}

/// Replaces the database with a staged restore, if one is waiting. Call before opening
/// the database. The replaced database is kept beside it as `<database>.replaced` until
/// the next restore. Returns whether a restore was applied.
///
/// # Errors
/// Returns [`StoreError`] when the old database cannot be checkpointed or a file cannot
/// be moved; the staged restore is then left for the next attempt.
pub fn apply_pending_restore(database: &Path) -> Result<bool, StoreError> {
    let restore = staged(database);
    if !restore.is_file() {
        return Ok(false);
    }
    if database.is_file() {
        // Fold the write-ahead log into the old file first, so no log of the old database
        // is left to be replayed into the restored one.
        let connection = Connection::open(database)?;
        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        drop(connection);
        let replaced = sibling(database, ".replaced");
        let _ = fs::remove_file(&replaced);
        fs::rename(database, &replaced)?;
    }
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(sibling(database, suffix));
    }
    fs::rename(&restore, database)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::created_at;
    use veyra_domain::time::UtcMillis;

    #[test]
    fn only_backup_names_are_accepted() {
        assert_eq!(
            created_at("veyra-1794497537318.db"),
            Some(UtcMillis(1_794_497_537_318))
        );
        for name in [
            "veyra-.db",
            "veyra-12a.db",
            "../veyra-1.db",
            "veyra-1.db/../../x",
            "other-1.db",
            "veyra-1.sqlite",
        ] {
            assert_eq!(created_at(name), None, "{name}");
        }
    }
}
