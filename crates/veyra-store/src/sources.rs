//! Usage sources, consent, per-file checkpoints and ingest diagnostics (FR-ING-02/04/08).

use std::collections::{HashMap, HashSet};

use rusqlite::{OptionalExtension, Row, params};
use veyra_domain::source::{PortError, SourceHealth, SourceRecord, SourceRepository};
use veyra_domain::time::UtcMillis;

use crate::sql::count_from;
use crate::{Store, StoreError};

impl SourceRepository for Store {
    fn sources(&self) -> Result<Vec<SourceRecord>, PortError> {
        Ok(Self::sources(self)?)
    }

    fn set_source_enabled(
        &self,
        id: i64,
        enabled: bool,
        now: UtcMillis,
    ) -> Result<Option<SourceRecord>, PortError> {
        Ok(Self::set_source_enabled(self, id, enabled, now)?)
    }

    fn health(&self, since: UtcMillis) -> Result<Vec<SourceHealth>, PortError> {
        Ok(self.source_health(since)?)
    }
}

/// A stored file checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCheckpoint {
    pub size: u64,
    pub modified_at: Option<UtcMillis>,
    pub offset: u64,
    pub head_hash: i64,
    pub head_len: u32,
    pub parser_version: u32,
}

/// Totals of one ingest run, for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunStats {
    pub files_read: u64,
    pub lines_read: u64,
    pub events_inserted: u64,
    pub events_updated: u64,
    pub events_unchanged: u64,
    pub issues: u64,
}

const SOURCE_COLUMNS: &str = "id, connector_id, root, enabled, consented_at, discovered_at, \
     last_scan_at, last_success_at, last_error";

fn source_from(row: &Row<'_>) -> rusqlite::Result<SourceRecord> {
    let millis = |v: Option<i64>| v.map(UtcMillis);
    Ok(SourceRecord {
        id: row.get(0)?,
        connector_id: row.get(1)?,
        root: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        consented_at: millis(row.get(4)?),
        discovered_at: UtcMillis(row.get(5)?),
        last_scan_at: millis(row.get(6)?),
        last_success_at: millis(row.get(7)?),
        last_error: row.get(8)?,
    })
}

fn to_i64(value: u64, what: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::OutOfRange(what))
}

impl Store {
    /// Records a discovered source; new sources start disabled. Returns the stored row.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn register_source(
        &self,
        connector_id: &str,
        root: &str,
        now: UtcMillis,
    ) -> Result<SourceRecord, StoreError> {
        let writer = self.lock_writer()?;
        writer.execute(
            "INSERT INTO sources (connector_id, root, discovered_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT (connector_id, root) DO NOTHING",
            params![connector_id, root, now.0],
        )?;
        Ok(writer.query_row(
            &format!("SELECT {SOURCE_COLUMNS} FROM sources WHERE connector_id = ?1 AND root = ?2"),
            params![connector_id, root],
            source_from,
        )?)
    }

    /// Ingestion health of every source: file counts, last read, and the issues found by
    /// runs started at or after `since`.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn source_health(&self, since: UtcMillis) -> Result<Vec<SourceHealth>, StoreError> {
        self.with_reader(|connection| {
            let mut health: Vec<SourceHealth> = connection
                .prepare_cached(
                    "SELECT s.id, \
                     COALESCE(SUM(f.state = 'active'), 0), COALESCE(SUM(f.state = 'missing'), 0), \
                     MAX(f.last_ingested_at) \
                     FROM sources s LEFT JOIN source_files f ON f.source_id = s.id \
                     GROUP BY s.id ORDER BY s.id",
                )?
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                    ))
                })?
                .map(|row| {
                    let (source_id, active, missing, last) = row?;
                    Ok(SourceHealth {
                        source_id,
                        files_active: count_from(Some(active), "file count")?.unwrap_or(0),
                        files_missing: count_from(Some(missing), "file count")?.unwrap_or(0),
                        last_ingested_at: last.map(UtcMillis),
                        recent_issues: Vec::new(),
                    })
                })
                .collect::<Result<_, StoreError>>()?;
            let mut statement = connection.prepare_cached(
                "SELECT source_id, kind, SUM(occurrences) FROM ingest_issues \
                 WHERE run_id IN (SELECT id FROM ingest_runs WHERE started_at >= ?1) \
                 GROUP BY source_id, kind ORDER BY source_id, kind",
            )?;
            let rows = statement.query_map([since.0], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            for row in rows {
                let (source_id, kind, occurrences) = row?;
                if let Some(entry) = health.iter_mut().find(|h| h.source_id == source_id) {
                    let occurrences = count_from(Some(occurrences), "issue count")?.unwrap_or(0);
                    entry.recent_issues.push((kind, occurrences));
                }
            }
            Ok(health)
        })
    }

    /// Every known source.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn sources(&self) -> Result<Vec<SourceRecord>, StoreError> {
        self.with_reader(|connection| {
            let mut statement = connection
                .prepare_cached(&format!("SELECT {SOURCE_COLUMNS} FROM sources ORDER BY id"))?;
            let rows = statement.query_map([], source_from)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Enables or disables a source. Enabling records consent the first time.
    /// Returns `None` when the source does not exist.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn set_source_enabled(
        &self,
        id: i64,
        enabled: bool,
        now: UtcMillis,
    ) -> Result<Option<SourceRecord>, StoreError> {
        let writer = self.lock_writer()?;
        writer.execute(
            "UPDATE sources SET enabled = ?2, \
             consented_at = CASE WHEN ?2 = 1 THEN COALESCE(consented_at, ?3) ELSE consented_at END \
             WHERE id = ?1",
            params![id, i64::from(enabled), now.0],
        )?;
        Ok(writer
            .query_row(
                &format!("SELECT {SOURCE_COLUMNS} FROM sources WHERE id = ?1"),
                [id],
                source_from,
            )
            .optional()?)
    }

    /// Records the outcome of scanning a source.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn record_scan(
        &self,
        id: i64,
        now: UtcMillis,
        error: Option<&str>,
    ) -> Result<(), StoreError> {
        self.lock_writer()?.execute(
            "UPDATE sources SET last_scan_at = ?2, last_error = ?3, \
             last_success_at = CASE WHEN ?3 IS NULL THEN ?2 ELSE last_success_at END WHERE id = ?1",
            params![id, now.0, error],
        )?;
        Ok(())
    }

    /// Checkpoints of a source's files, keyed by relative path.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure or unreadable values.
    pub fn checkpoints(
        &self,
        source_id: i64,
    ) -> Result<HashMap<String, StoredCheckpoint>, StoreError> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT path, size, modified_at, byte_offset, head_hash, head_len, parser_version \
                 FROM source_files WHERE source_id = ?1",
            )?;
            let rows = statement.query_map([source_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, u32>(6)?,
                ))
            })?;
            let mut map = HashMap::new();
            for row in rows {
                let (path, size, modified, offset, head_hash, head_len, parser_version) = row?;
                let unsigned = |v: i64| {
                    u64::try_from(v)
                        .map_err(|_| StoreError::Corrupt("negative file position".into()))
                };
                map.insert(
                    path,
                    StoredCheckpoint {
                        size: unsigned(size)?,
                        modified_at: modified.map(UtcMillis),
                        offset: unsigned(offset)?,
                        head_hash,
                        head_len,
                        parser_version,
                    },
                );
            }
            Ok(map)
        })
    }

    /// Marks files that no longer exist as missing. Their events are kept: Veyra retains
    /// history after a tool deletes its own logs (FR-ING-12). Returns files newly marked.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn mark_missing_files(
        &self,
        source_id: i64,
        present: &HashSet<String>,
    ) -> Result<usize, StoreError> {
        let mut writer = self.lock_writer()?;
        let transaction = writer.transaction()?;
        let tracked: Vec<String> = transaction
            .prepare("SELECT path FROM source_files WHERE source_id = ?1 AND state = 'active'")?
            .query_map([source_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut marked = 0;
        for path in tracked.iter().filter(|p| !present.contains(*p)) {
            marked += transaction.execute(
                "UPDATE source_files SET state = 'missing' WHERE source_id = ?1 AND path = ?2",
                params![source_id, path],
            )?;
        }
        transaction.commit()?;
        Ok(marked)
    }

    /// Starts an ingest run record. Returns its id.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn start_run(&self, trigger: &str, now: UtcMillis) -> Result<i64, StoreError> {
        let writer = self.lock_writer()?;
        writer.execute(
            "INSERT INTO ingest_runs (trigger, started_at) VALUES (?1, ?2)",
            params![trigger, now.0],
        )?;
        Ok(writer.last_insert_rowid())
    }

    /// Completes an ingest run record.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure or out-of-range totals.
    pub fn finish_run(
        &self,
        run_id: i64,
        stats: RunStats,
        outcome: &str,
        now: UtcMillis,
    ) -> Result<(), StoreError> {
        self.lock_writer()?.execute(
            "UPDATE ingest_runs SET finished_at = ?2, files_read = ?3, lines_read = ?4, \
             events_inserted = ?5, events_updated = ?6, events_unchanged = ?7, issues = ?8, \
             outcome = ?9 WHERE id = ?1",
            params![
                run_id,
                now.0,
                to_i64(stats.files_read, "files read")?,
                to_i64(stats.lines_read, "lines read")?,
                to_i64(stats.events_inserted, "events inserted")?,
                to_i64(stats.events_updated, "events updated")?,
                to_i64(stats.events_unchanged, "events unchanged")?,
                to_i64(stats.issues, "issues")?,
                outcome,
            ],
        )?;
        Ok(())
    }

    /// Adds parse issues of one file to a run (counted per kind; no content stored).
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure.
    pub fn record_issues(
        &self,
        run_id: i64,
        source_id: i64,
        path: &str,
        issues: &[veyra_domain::source::ParseIssue],
    ) -> Result<(), StoreError> {
        if issues.is_empty() {
            return Ok(());
        }
        let mut writer = self.lock_writer()?;
        let transaction = writer.transaction()?;
        {
            let mut upsert = transaction.prepare_cached(
                "INSERT INTO ingest_issues (run_id, source_id, path, kind, occurrences, first_offset) \
                 VALUES (?1, ?2, ?3, ?4, 1, ?5) ON CONFLICT (run_id, source_id, path, kind) \
                 DO UPDATE SET occurrences = occurrences + 1, \
                 first_offset = MIN(first_offset, excluded.first_offset)",
            )?;
            for issue in issues {
                upsert.execute(params![
                    run_id,
                    source_id,
                    path,
                    issue.kind,
                    to_i64(issue.offset, "issue offset")?
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}
