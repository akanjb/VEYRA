//! Importing a chat app's data export (ADR-040): finds the connector that reads the
//! file and stores its usage in batches, so memory stays bounded however large the
//! export is.

use std::sync::{Arc, Mutex, PoisonError};

use veyra_domain::import::{
    ArchiveImport, ExportImporter, ImportCounts, ImportError, ImportSummary, ReadSeek,
};
use veyra_domain::source::{ConnectorDescriptor, RawUsage};
use veyra_registry::Registry;
use veyra_store::Store;

use crate::ChangeListener;
use crate::push::store_usage;

/// Records written per transaction.
const BATCH: usize = 2_000;

/// Everything the importer needs to read exports and store their usage.
pub struct ImportParts {
    pub store: Arc<Store>,
    pub registry: Arc<Registry>,
    pub importers: Vec<Box<dyn ExportImporter>>,
    /// Called after an import changed stored data.
    pub on_change: ChangeListener,
}

/// Imports data exports with the connectors it was given.
pub struct Importer {
    store: Arc<Store>,
    registry: Arc<Registry>,
    importers: Vec<Box<dyn ExportImporter>>,
    on_change: Mutex<ChangeListener>,
}

impl Importer {
    #[must_use]
    pub fn new(parts: ImportParts) -> Self {
        Self {
            store: parts.store,
            registry: parts.registry,
            importers: parts.importers,
            on_change: Mutex::new(parts.on_change),
        }
    }
}

/// Collects records and writes them a batch at a time.
struct Batches<'a> {
    importer: &'a Importer,
    descriptor: &'a ConnectorDescriptor,
    pending: Vec<RawUsage>,
    summary: ImportSummary,
}

impl Batches<'_> {
    fn push(&mut self, usage: RawUsage) -> Result<(), ImportError> {
        self.pending.push(usage);
        if self.pending.len() >= BATCH {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), ImportError> {
        let pending = std::mem::take(&mut self.pending);
        let usage = pending.into_iter().map(|u| (u, self.descriptor));
        let (_, report) = store_usage(&self.importer.store, &self.importer.registry, usage)
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        self.summary.inserted += report.inserted;
        self.summary.updated += report.updated;
        self.summary.unchanged += report.unchanged;
        if report.changed() {
            let on_change = self
                .importer
                .on_change
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            on_change(&report);
        }
        Ok(())
    }
}

impl ArchiveImport for Importer {
    fn apps(&self) -> Vec<ConnectorDescriptor> {
        self.importers
            .iter()
            .map(|i| i.descriptor().clone())
            .collect()
    }

    fn import(&self, file: &mut dyn ReadSeek) -> Result<ImportSummary, ImportError> {
        for importer in &self.importers {
            let descriptor = importer.descriptor();
            let mut batches = Batches {
                importer: self,
                descriptor,
                pending: Vec::new(),
                summary: ImportSummary {
                    app_name: descriptor.display_name,
                    counts: ImportCounts::default(),
                    inserted: 0,
                    updated: 0,
                    unchanged: 0,
                },
            };
            let read = importer.import(file, &mut |usage| batches.push(usage));
            // What was read before a failure is kept.
            let flushed = batches.flush();
            if let Some(counts) = read? {
                flushed?;
                batches.summary.counts = counts;
                return Ok(batches.summary);
            }
        }
        Err(ImportError::NotRecognized)
    }
}
