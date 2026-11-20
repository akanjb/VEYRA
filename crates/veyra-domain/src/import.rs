//! Usage read from the official data exports of chat apps (ADR-040).
//!
//! Consumer chat apps keep their usage on the provider's servers and report no tokens.
//! Their data exports do list every message, so an importer reads an archive the user
//! picks, keeps only identifiers, times, models and estimated token counts, and drops
//! the text as it goes. Connectors know the export formats; the ingest layer opens the
//! file and stores what they report.

use std::io::{Read, Seek};

use crate::source::{ConnectorDescriptor, RawUsage};

/// A file an importer can read and rewind: archives keep their index at the end.
pub trait ReadSeek: Read + Seek + Send {}

impl<T: Read + Seek + Send> ReadSeek for T {}

/// Why an import stopped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// No importer recognised the file.
    #[error("this file is not a data export Veyra can read")]
    NotRecognized,
    /// The file is an export but could not be read to the end.
    #[error("the export cannot be read: {0}")]
    Unreadable(String),
    /// The usage could not be stored.
    #[error("the usage cannot be stored: {0}")]
    Storage(String),
}

/// What an importer found in an export it recognised.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportCounts {
    /// Conversations read.
    pub conversations: u64,
    /// Conversations skipped because their shape was not understood.
    pub skipped: u64,
    /// Usage records produced, one per model reply.
    pub replies: u64,
}

/// Reads one app's export format (implemented by connectors).
pub trait ExportImporter: Send + Sync {
    /// The application the export belongs to.
    fn descriptor(&self) -> &ConnectorDescriptor;

    /// Reads `file` from its start and passes each usage record to `emit`. Returns
    /// `Ok(None)` when the file is not this importer's export.
    ///
    /// # Errors
    /// [`ImportError::Unreadable`] when the export is damaged part-way, or the error
    /// `emit` returned.
    fn import(
        &self,
        file: &mut dyn ReadSeek,
        emit: &mut dyn FnMut(RawUsage) -> Result<(), ImportError>,
    ) -> Result<Option<ImportCounts>, ImportError>;
}

/// The outcome of importing one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// The application whose export it was.
    pub app_name: &'static str,
    pub counts: ImportCounts,
    /// Records new to the database.
    pub inserted: u64,
    /// Records that replaced an earlier reading.
    pub updated: u64,
    /// Records already stored from an earlier import.
    pub unchanged: u64,
}

/// Imports a data export into the database (implemented by the ingest layer).
pub trait ArchiveImport: Send + Sync {
    /// The applications whose exports it reads.
    fn apps(&self) -> Vec<ConnectorDescriptor>;

    /// Finds the importer for `file` and stores its usage.
    ///
    /// # Errors
    /// [`ImportError::NotRecognized`] when no importer reads the file; otherwise why it
    /// stopped. Records stored before a failure stay stored.
    fn import(&self, file: &mut dyn ReadSeek) -> Result<ImportSummary, ImportError>;
}
