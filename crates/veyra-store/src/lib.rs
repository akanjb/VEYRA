//! Veyra data access: SQLite persistence (ADR-003).
//!
//! One writer connection commits each batch — events, costs and rollups — in a single
//! transaction; a small pool of read-only connections serves analytics concurrently
//! (WAL mode). The store persists what it is given: pricing and normalisation happen
//! upstream, and analytics derives metrics from the additive sums it returns.

mod billing;
mod connection;
mod error;
mod fingerprint;
mod mapping;
mod migrations;
mod pricing_reads;
mod projects;
mod reader;
mod reprice;
mod rollup;
mod session_reads;
mod size_reads;
mod sources;
mod sql;
mod write;

use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, TryLockError};

use rusqlite::{Connection, Row, Transaction};
use veyra_domain::source::PortError;

pub use error::StoreError;
pub use migrations::LATEST_SCHEMA_VERSION;
pub use reprice::StoredEvent;
pub use sources::{RunStats, StoredCheckpoint};
pub use veyra_domain::source::SourceRecord;
pub use write::{FileCheckpoint, IngestBatch, PricedEvent, WriteSummary};

/// Number of read-only connections serving analytics. A dashboard page runs its
/// independent reads at the same time, each on its own connection.
const READERS: usize = 4;

/// The Veyra database.
#[derive(Debug)]
pub struct Store {
    writer: Mutex<Connection>,
    readers: Vec<Mutex<Connection>>,
    next_reader: AtomicUsize,
    /// Incremented whenever a write changes data; lets callers invalidate caches.
    data_version: AtomicU64,
}

impl Store {
    /// Opens (creating if needed) and migrates the database at `path`.
    ///
    /// # Errors
    /// Returns [`StoreError`] if the file cannot be opened, is from a newer version of
    /// Veyra, or a migration fails.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut writer = connection::open_writer(path)?;
        migrations::migrate(&mut writer)?;
        let readers = (0..READERS)
            .map(|_| connection::open_reader(path).map(Mutex::new))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            writer: Mutex::new(writer),
            readers,
            next_reader: AtomicUsize::new(0),
            data_version: AtomicU64::new(0),
        })
    }

    /// Changes whenever stored data changes (starts at 0 when opened).
    #[must_use]
    pub fn data_version(&self) -> u64 {
        self.data_version.load(Ordering::Acquire)
    }

    /// Writes a batch of priced events idempotently (no checkpoint).
    ///
    /// # Errors
    /// Returns [`StoreError`] for invalid events or database failures; nothing from the
    /// batch is written in that case.
    pub fn write_events(
        &self,
        events: &[PricedEvent],
        now: veyra_domain::time::UtcMillis,
    ) -> Result<WriteSummary, StoreError> {
        self.commit_parts(events, &[], None, now)
    }

    /// Commits events, tool outcomes and a file checkpoint in one transaction.
    ///
    /// # Errors
    /// Returns [`StoreError`] for invalid events or database failures; nothing from the
    /// batch is written in that case.
    pub fn commit(
        &self,
        batch: &IngestBatch,
        now: veyra_domain::time::UtcMillis,
    ) -> Result<WriteSummary, StoreError> {
        self.commit_parts(
            &batch.events,
            &batch.tool_outcomes,
            batch.checkpoint.as_ref(),
            now,
        )
    }

    fn commit_parts(
        &self,
        events: &[PricedEvent],
        tool_outcomes: &[veyra_domain::tooling::ToolOutcome],
        checkpoint: Option<&FileCheckpoint>,
        now: veyra_domain::time::UtcMillis,
    ) -> Result<WriteSummary, StoreError> {
        let summary = write::commit(
            &mut *self.lock_writer()?,
            events,
            tool_outcomes,
            checkpoint,
            now,
        )?;
        if summary.inserted + summary.updated > 0 {
            self.data_version.fetch_add(1, Ordering::AcqRel);
        }
        Ok(summary)
    }

    /// Writes a consistent copy of the database to `destination` (`VACUUM INTO`).
    ///
    /// # Errors
    /// Returns [`StoreError`] if the copy fails (for example, the file exists).
    pub fn backup_to(&self, destination: &Path) -> Result<(), StoreError> {
        let target = destination.to_string_lossy().into_owned();
        self.lock_writer()?.execute("VACUUM INTO ?1", [target])?;
        Ok(())
    }

    fn lock_writer(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.writer.lock().map_err(|_| StoreError::Poisoned)
    }

    /// Every row of a query without parameters.
    fn read_all<T>(
        &self,
        sql: &str,
        map: fn(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, PortError> {
        Ok(self.with_reader(|connection| {
            let mut statement = connection.prepare_cached(sql)?;
            let rows = statement.query_map([], map)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?)
    }

    /// Runs a user's change in one write transaction, then marks the data as changed so
    /// cached views refresh.
    fn change<T>(
        &self,
        change: impl FnOnce(&Transaction<'_>) -> Result<T, PortError>,
    ) -> Result<T, PortError> {
        let mut writer = self.lock_writer()?;
        let transaction = writer.transaction().map_err(StoreError::from)?;
        let result = change(&transaction)?;
        transaction.commit().map_err(StoreError::from)?;
        self.data_version.fetch_add(1, Ordering::AcqRel);
        Ok(result)
    }

    /// Runs `f` on a read-only connection: the first free one, starting from the next in
    /// turn, or else the next in turn once it is free.
    fn with_reader<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let count = self.readers.len();
        let first = self.next_reader.fetch_add(1, Ordering::Relaxed) % count;
        for offset in 0..count {
            match self.readers[(first + offset) % count].try_lock() {
                Ok(reader) => return f(&reader),
                Err(TryLockError::WouldBlock) => {}
                Err(TryLockError::Poisoned(_)) => return Err(StoreError::Poisoned),
            }
        }
        let reader = self.readers[first]
            .lock()
            .map_err(|_| StoreError::Poisoned)?;
        f(&reader)
    }
}
