//! The ingest pipeline (ADR-023): scoped read → parse → normalise → price → commit.
//!
//! Each chunk of complete lines is committed in one transaction together with the file's
//! new checkpoint, so a crash or cancellation never loses or double-counts data: the next
//! run resumes from the last committed checkpoint and re-applies nothing twice.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::time::Duration;

use veyra_domain::connector::{Compression, DatabaseQuery, Reading, SIDE_FILE_MAX_BYTES, SideFile};
use veyra_domain::pricing::price_event;
use veyra_domain::source::{FileContext, ParseIssue, SourceConnector};
use veyra_domain::time::UtcMillis;
use veyra_registry::Registry;
use veyra_store::{
    FileCheckpoint, IngestBatch, PricedEvent, RunStats, SourceRecord, Store, StoredCheckpoint,
    WriteSummary,
};

use crate::IngestError;
use crate::fs::{FoundFile, ScopedFs};
use crate::normalize::normalize;
use crate::snapshot::database_rows;
use crate::tail::{self, HEAD_LEN, ReadPlan, Restart};
use crate::whole::read_whole;

/// Deepest directory level searched below a source root.
const MAX_DEPTH: usize = 4;

/// Default number of bytes read and committed per step.
pub const DEFAULT_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

/// What the pipeline needs to run.
pub struct IngestContext<'a> {
    pub store: &'a Store,
    pub registry: &'a Registry,
    pub connectors: &'a [Box<dyn SourceConnector>],
    /// Checked between chunks; set to stop promptly (committed chunks are kept).
    pub cancel: &'a AtomicBool,
    pub chunk_bytes: u64,
    /// Optional pause after each committed chunk, to stay light on busy machines.
    pub throttle: Option<Duration>,
}

/// Totals of scanning one or more sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub sources: u64,
    pub files_seen: u64,
    pub files_read: u64,
    pub files_missing: u64,
    /// Files re-read from the start (truncated, replaced or parser changed).
    pub restarts: u64,
    pub lines: u64,
    pub inserted: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub superseded: u64,
    pub issues: u64,
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

impl ScanReport {
    /// Whether stored data changed.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.inserted + self.updated > 0
    }

    fn absorb(&mut self, other: &Self) {
        self.sources += other.sources;
        self.files_seen += other.files_seen;
        self.files_read += other.files_read;
        self.files_missing += other.files_missing;
        self.restarts += other.restarts;
        self.lines += other.lines;
        self.inserted += other.inserted;
        self.updated += other.updated;
        self.unchanged += other.unchanged;
        self.superseded += other.superseded;
        self.issues += other.issues;
    }

    fn record_write(&mut self, summary: WriteSummary) {
        self.inserted += count(summary.inserted);
        self.updated += count(summary.updated);
        self.unchanged += count(summary.unchanged);
        self.superseded += count(summary.superseded);
    }

    const fn stats(&self) -> RunStats {
        RunStats {
            files_read: self.files_read,
            lines_read: self.lines,
            events_inserted: self.inserted,
            events_updated: self.updated,
            events_unchanged: self.unchanged + self.superseded,
            issues: self.issues,
        }
    }
}

/// Scans every enabled source once, recording the run and each source's outcome.
///
/// # Errors
/// Returns [`IngestError::Store`] when the database fails, or [`IngestError::Cancelled`].
/// Problems confined to one source are recorded on that source instead.
pub fn scan_all(context: &IngestContext<'_>, trigger: &str) -> Result<ScanReport, IngestError> {
    let mut total = ScanReport::default();
    let enabled: Vec<SourceRecord> = context
        .store
        .sources()?
        .into_iter()
        .filter(|s| s.enabled)
        .collect();
    if enabled.is_empty() {
        return Ok(total);
    }
    let run_id = context.store.start_run(trigger, UtcMillis::now())?;
    let mut outcome = "ok";
    for source in &enabled {
        match scan_source(context, source, run_id) {
            Ok(report) => {
                total.absorb(&report);
                context
                    .store
                    .record_scan(source.id, UtcMillis::now(), None)?;
            }
            Err(IngestError::Cancelled) => {
                outcome = "cancelled";
                break;
            }
            Err(IngestError::Store(error)) => {
                let _ = context
                    .store
                    .finish_run(run_id, total.stats(), "failed", UtcMillis::now());
                return Err(IngestError::Store(error));
            }
            Err(error) => {
                outcome = "partial";
                context
                    .store
                    .record_scan(source.id, UtcMillis::now(), Some(&error.to_string()))?;
            }
        }
    }
    context
        .store
        .finish_run(run_id, total.stats(), outcome, UtcMillis::now())?;
    if outcome == "cancelled" {
        return Err(IngestError::Cancelled);
    }
    Ok(total)
}

/// Reads one source's new data. Disabled sources are never touched.
///
/// # Errors
/// Returns [`IngestError`] when the root is missing, the connector is unknown, the
/// database fails or the scan is cancelled. Unreadable files are counted as issues.
pub fn scan_source(
    context: &IngestContext<'_>,
    source: &SourceRecord,
    run_id: i64,
) -> Result<ScanReport, IngestError> {
    let mut report = ScanReport::default();
    if !source.enabled {
        return Ok(report);
    }
    let connector = context
        .connectors
        .iter()
        .find(|c| c.descriptor().id.as_str() == source.connector_id)
        .ok_or_else(|| IngestError::UnknownConnector(source.connector_id.clone()))?;
    let fs = ScopedFs::new(Path::new(&source.root))?;
    let files = fs.find_files(MAX_DEPTH, &|relative| connector.is_source_file(relative));
    let present: HashSet<String> = files.iter().map(|f| f.relative.clone()).collect();
    report.sources = 1;
    report.files_seen = count(files.len());
    report.files_missing = count(context.store.mark_missing_files(source.id, &present)?);
    let checkpoints = context.store.checkpoints(source.id)?;
    for file in &files {
        let reader = FileReader {
            context,
            fs: &fs,
            connector: connector.as_ref(),
            source_id: source.id,
            run_id,
            file,
            side_files: side_files(&fs, connector.as_ref(), &file.relative),
        };
        if let Err(error) = reader.read(checkpoints.get(&file.relative), &mut report) {
            let Some(kind) = error.file_issue() else {
                return Err(error);
            };
            report.issues += 1;
            let issue = ParseIssue { kind, offset: 0 };
            context
                .store
                .record_issues(run_id, source.id, &file.relative, &[issue])?;
        }
    }
    Ok(report)
}

/// The side files a connector asked for that exist and can be read (at most
/// [`SIDE_FILE_MAX_BYTES`] each).
fn side_files(fs: &ScopedFs, connector: &dyn SourceConnector, relative: &str) -> Vec<SideFile> {
    connector
        .side_files(Path::new(relative))
        .into_iter()
        .filter_map(|path| {
            let bytes = fs.read_at(path.to_str()?, 0, SIDE_FILE_MAX_BYTES).ok()?;
            Some(SideFile {
                relative: path,
                bytes,
            })
        })
        .collect()
}

/// Reads one file, as its connector declares, from its checkpoint.
struct FileReader<'r, 'c> {
    context: &'r IngestContext<'c>,
    fs: &'r ScopedFs,
    connector: &'r dyn SourceConnector,
    source_id: i64,
    run_id: i64,
    file: &'r FoundFile,
    side_files: Vec<SideFile>,
}

/// What one run of complete lines produces, committed as one transaction.
struct Step {
    batch: IngestBatch,
    issues: Vec<ParseIssue>,
    lines: u64,
}

impl FileReader<'_, '_> {
    fn read(
        &self,
        checkpoint: Option<&StoredCheckpoint>,
        report: &mut ScanReport,
    ) -> Result<(), IngestError> {
        match self.connector.reading(Path::new(&self.file.relative)) {
            Reading::AppendLines => self.read_lines(checkpoint, report),
            Reading::WholeFile(compression) => self.read_whole(compression, checkpoint, report),
            Reading::Database(queries) => self.read_database(queries, checkpoint, report),
        }
    }

    /// Re-reads the whole file when it changed since the last read.
    fn read_whole(
        &self,
        compression: Compression,
        checkpoint: Option<&StoredCheckpoint>,
        report: &mut ScanReport,
    ) -> Result<(), IngestError> {
        let version = (self.file.len, self.file.modified);
        if tail::unchanged(version, checkpoint, self.parser_version()) {
            return Ok(());
        }
        let data = read_whole(self.fs, &self.file.relative, compression)?;
        self.read_all(&data, version, checkpoint, report)
    }

    /// Re-reads the database through a snapshot when it or its write-ahead log changed.
    fn read_database(
        &self,
        queries: &[DatabaseQuery],
        checkpoint: Option<&StoredCheckpoint>,
        report: &mut ScanReport,
    ) -> Result<(), IngestError> {
        // An empty log holds no changes (opening the database may leave one behind);
        // emptying a log means writing its changes into the database file, which shows.
        let wal = self
            .fs
            .stat(&format!("{}-wal", self.file.relative))?
            .filter(|(len, _)| *len > 0);
        let version = match wal {
            Some((len, modified)) => (self.file.len + len, self.file.modified.max(modified)),
            None => (self.file.len, self.file.modified),
        };
        if tail::unchanged(version, checkpoint, self.parser_version()) {
            return Ok(());
        }
        let path = self.fs.verified_path(&self.file.relative)?;
        let rows = database_rows(&path, queries)?;
        self.read_all(&rows, version, checkpoint, report)
    }

    /// Parses complete content in one step, committed with a checkpoint recording the
    /// content's `(size, modified)` version.
    fn read_all(
        &self,
        data: &[u8],
        (size, modified): (u64, Option<UtcMillis>),
        checkpoint: Option<&StoredCheckpoint>,
        report: &mut ScanReport,
    ) -> Result<(), IngestError> {
        if checkpoint.is_some() {
            report.restarts += 1;
        }
        report.files_read += 1;
        let mut step = Step {
            batch: IngestBatch::default(),
            issues: Vec::new(),
            lines: 0,
        };
        self.parse(data, 0, &mut step);
        step.batch.checkpoint = Some(FileCheckpoint {
            source_id: self.source_id,
            path: self.file.relative.clone(),
            size,
            modified_at: modified,
            offset: size,
            head_hash: tail::fingerprint(&[]),
            head_len: 0,
            parser_version: self.parser_version(),
        });
        self.commit(&step, report)
    }

    fn parser_version(&self) -> u32 {
        self.connector.descriptor().parser_version
    }

    /// Reads an append-only file from the checkpoint to the last complete line. A helper thread reads,
    /// parses, normalises and prices the next chunk while this thread commits the
    /// previous one; steps are committed strictly in file order.
    fn read_lines(
        &self,
        checkpoint: Option<&StoredCheckpoint>,
        report: &mut ScanReport,
    ) -> Result<(), IngestError> {
        let parser_version = self.parser_version();
        let (plan, restart) = tail::plan(self.file, checkpoint, parser_version, |len| {
            self.head(len)
                .ok()
                .filter(|head| head.1 == len)
                .map(|head| head.0)
        });
        let ReadPlan::From(position) = plan else {
            return Ok(());
        };
        if !matches!(restart, None | Some(Restart::New)) {
            report.restarts += 1;
        }
        let head = match (restart, checkpoint) {
            (None, Some(stored)) => (stored.head_hash, stored.head_len),
            _ => (tail::fingerprint(&[]), 0),
        };
        report.files_read += 1;
        std::thread::scope(|scope| {
            let (sender, receiver) = mpsc::sync_channel(1);
            scope.spawn(move || self.produce(position, head, &sender));
            let mut outcome = Ok(());
            for step in &receiver {
                if let Err(error) = step.and_then(|step| self.commit(&step, report)) {
                    outcome = Err(error);
                    break;
                }
            }
            // A producer still running stops at its next send.
            drop(receiver);
            outcome
        })
    }

    /// Produces steps until the last complete line, an error or cancellation.
    fn produce(
        &self,
        mut position: u64,
        mut head: (i64, u32),
        steps: &SyncSender<Result<Step, IngestError>>,
    ) {
        let chunk = self.context.chunk_bytes.max(1);
        loop {
            match self.next_step(&mut position, &mut head, chunk) {
                Ok(Some((step, at_end))) => {
                    if steps.send(Ok(step)).is_err() || at_end {
                        return;
                    }
                }
                Ok(None) => return,
                Err(error) => {
                    let _ = steps.send(Err(error));
                    return;
                }
            }
        }
    }

    /// Reads and prepares the next step; `None` when no complete line remains. Also
    /// returns whether the end of the file was reached.
    fn next_step(
        &self,
        position: &mut u64,
        head: &mut (i64, u32),
        chunk: u64,
    ) -> Result<Option<(Step, bool)>, IngestError> {
        if self.context.cancel.load(Ordering::Relaxed) {
            return Err(IngestError::Cancelled);
        }
        let start = *position;
        let buffer = self.fs.read_at(&self.file.relative, start, chunk)?;
        let at_end = count(buffer.len()) < chunk;
        let (lines, consumed) = tail::complete_lines(&buffer);
        let mut step = Step {
            batch: IngestBatch::default(),
            issues: Vec::new(),
            lines: 0,
        };
        let next = if consumed > 0 {
            self.parse(lines, start, &mut step);
            start + count(consumed)
        } else if at_end {
            return Ok(None); // Only a partial line remains; it is read once complete.
        } else {
            step.issues.push(ParseIssue {
                kind: "oversize-line",
                offset: start,
            });
            match self.next_line_start(start + count(buffer.len()), chunk)? {
                Some(next) => next,
                None => return Ok(None),
            }
        };
        if head.1 != head_len_at(next) {
            *head = self.head(head_len_at(next))?;
        }
        step.batch.checkpoint = Some(FileCheckpoint {
            source_id: self.source_id,
            path: self.file.relative.clone(),
            size: self.file.len.max(next),
            modified_at: self.file.modified,
            offset: next,
            head_hash: head.0,
            head_len: head.1,
            parser_version: self.parser_version(),
        });
        *position = next;
        Ok(Some((step, at_end)))
    }

    /// Commits a step: its events, tool outcomes and checkpoint in one transaction.
    fn commit(&self, step: &Step, report: &mut ScanReport) -> Result<(), IngestError> {
        report.lines += step.lines;
        report.record_write(self.context.store.commit(&step.batch, UtcMillis::now())?);
        report.issues += count(step.issues.len());
        self.context.store.record_issues(
            self.run_id,
            self.source_id,
            &self.file.relative,
            &step.issues,
        )?;
        if let Some(pause) = self.context.throttle {
            std::thread::sleep(pause);
        }
        Ok(())
    }

    /// Parses complete lines, normalising and pricing every usage record.
    fn parse(&self, lines: &[u8], base_offset: u64, step: &mut Step) {
        let descriptor = self.connector.descriptor();
        let file_context = FileContext {
            root: self.fs.root(),
            relative: Path::new(&self.file.relative),
            base_offset,
            side_files: &self.side_files,
        };
        let parsed = self.connector.parse(&file_context, lines);
        step.lines = parsed.lines;
        step.issues.extend(parsed.issues);
        step.batch.tool_outcomes = parsed.tool_outcomes;
        let registry = self.context.registry;
        for raw in parsed.usage {
            match normalize(&raw, descriptor, registry) {
                Ok(event) => {
                    let cost = price_event(&event, registry.catalog());
                    step.batch.events.push(PricedEvent {
                        event,
                        cost,
                        precedence: raw.precedence,
                        tool_calls: raw.tool_calls,
                    });
                }
                Err(_) => step.issues.push(ParseIssue {
                    kind: "invalid-record",
                    offset: base_offset,
                }),
            }
        }
    }

    /// Fingerprint of the file's first `len` bytes, and how many bytes were hashed.
    fn head(&self, len: u32) -> Result<(i64, u32), IngestError> {
        let bytes = self.fs.read_at(&self.file.relative, 0, u64::from(len))?;
        let read = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        Ok((tail::fingerprint(&bytes), read))
    }

    /// The offset just past the next newline at or after `from`, if there is one.
    fn next_line_start(&self, mut from: u64, chunk: u64) -> Result<Option<u64>, IngestError> {
        loop {
            let buffer = self.fs.read_at(&self.file.relative, from, chunk)?;
            if let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
                return Ok(Some(from + count(index) + 1));
            }
            if count(buffer.len()) < chunk {
                return Ok(None);
            }
            from += count(buffer.len());
        }
    }
}

/// How many leading bytes are fingerprinted once `offset` bytes have been read.
fn head_len_at(offset: u64) -> u32 {
    u32::try_from(offset.min(HEAD_LEN)).unwrap_or(u32::MAX)
}
