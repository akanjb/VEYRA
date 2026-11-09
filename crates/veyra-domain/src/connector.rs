//! How the ingest pipeline reads a connector's sources, and what a connector can report.
//!
//! Connectors stay pure parsers (ADR-023): they declare how their files are read, and
//! the pipeline does the reading, decompression and database snapshots.

use std::path::PathBuf;

/// How the pipeline reads one source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// Append-only JSON Lines, read incrementally from the last complete line.
    AppendLines,
    /// Read in full whenever the file changes: for documents rewritten in place, and for
    /// formats whose records depend on earlier lines (running totals, re-appended
    /// records). The parser receives the whole, decompressed content at offset 0.
    WholeFile(Compression),
    /// A live SQLite database owned by another application, read through a private
    /// snapshot that is deleted afterwards (ADR-015). Each result row of each query is
    /// passed to the parser as one JSON object line (column name to value).
    Database(&'static [DatabaseQuery]),
}

/// How a whole file is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Zstd,
}

/// One read-only query of a database snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseQuery {
    /// Tables the query reads. The query is skipped when any is missing, because each
    /// version of the source application has a different set of tables.
    pub tables: &'static [&'static str],
    /// Selects only metadata columns and JSON fields, never whole content columns.
    pub sql: &'static str,
}

/// A small file next to a source file that the parser needs for context, such as the
/// file naming a session's project folder. Read by the pipeline, at most
/// [`SIDE_FILE_MAX_BYTES`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideFile {
    /// Path relative to the source root.
    pub relative: PathBuf,
    pub bytes: Vec<u8>,
}

/// The largest side file passed to a parser.
pub const SIDE_FILE_MAX_BYTES: u64 = 4_096;

/// How completely a connector reports one kind of information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Full,
    /// Reported with a limitation, stated in plain words for the user.
    Partial(&'static str),
    None,
}

/// What a connector can report, shown on Sources & Health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Token counts, and at what grain.
    pub tokens: Support,
    /// Cache reads and writes.
    pub cache: Support,
    /// Reasoning tokens.
    pub reasoning: Support,
    /// Tool names and whether each call succeeded.
    pub tools: Support,
    /// Sessions, and subagent sessions linked to their parent.
    pub sessions: Support,
    /// The project folder each session worked in.
    pub projects: Support,
}
