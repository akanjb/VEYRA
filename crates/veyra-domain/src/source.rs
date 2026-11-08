//! The connector contract (ADR-002, ADR-023).
//!
//! A connector turns the bytes of one usage source into [`RawUsage`] records. It is pure:
//! it never touches the filesystem, network or database. The ingest pipeline finds
//! files, reads new bytes incrementally, normalises records and persists them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::accuracy::AccuracyMap;
use crate::event::{Granularity, ServiceTier};
use crate::identity::{AppId, ConnectorId, Interface};
use crate::money::NanoUsd;
use crate::time::UtcMillis;
use crate::tokens::TokenCounts;
use crate::tooling::{ToolCall, ToolOutcome};

/// How a source stores its records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    /// Append-only JSON Lines files, read incrementally from a byte offset.
    JsonLines,
}

/// Static facts about a connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorDescriptor {
    pub id: ConnectorId,
    /// The application whose usage this connector reads.
    pub app: AppId,
    pub display_name: &'static str,
    /// Bumped whenever parsing changes; files parsed by an older version are re-read.
    pub parser_version: u32,
    pub format: SourceFormat,
    /// Whether the vendor documents the format (false means it may change without notice).
    pub documented_format: bool,
}

/// Where connectors may look: the user's home directory and documented location
/// overrides only. Never credentials (ADR-014).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryEnv {
    pub home: Option<PathBuf>,
    /// Documented location environment variables, e.g. `CLAUDE_CONFIG_DIR`.
    pub location_overrides: BTreeMap<String, PathBuf>,
}

/// Where the bytes being parsed come from.
#[derive(Debug, Clone, Copy)]
pub struct FileContext<'a> {
    /// The source root the connector was configured with.
    pub root: &'a Path,
    /// The file's path relative to the root.
    pub relative: &'a Path,
    /// Byte offset of the first byte passed to the connector.
    pub base_offset: u64,
}

/// One usage observation, in canonical token semantics, before normalisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawUsage {
    /// Source-defined natural key; the same request seen twice has the same key.
    pub natural_key: String,
    /// How complete this observation is. A higher value supersedes a lower one; on a
    /// tie, the earlier `occurred_at` wins, so the result never depends on read order.
    pub precedence: i64,
    pub occurred_at: UtcMillis,
    /// The model string exactly as reported.
    pub model: Option<String>,
    /// The provider, when the source states it explicitly.
    pub provider: Option<String>,
    pub tokens: TokenCounts,
    pub requests: Option<u32>,
    pub reported_cost: Option<NanoUsd>,
    pub interface: Interface,
    pub service_tier: ServiceTier,
    pub inference_geo_us: bool,
    pub granularity: Granularity,
    pub session_key: Option<String>,
    pub parent_session_key: Option<String>,
    pub is_subagent: bool,
    /// Working directory, used to attribute the usage to a project.
    pub cwd: Option<String>,
    pub tool_version: Option<String>,
    pub accuracy: AccuracyMap,
    pub tool_calls: Vec<ToolCall>,
}

/// A recoverable problem with one record; counted in diagnostics, never fatal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseIssue {
    /// Stable identifier, e.g. `malformed-json`.
    pub kind: &'static str,
    /// Absolute byte offset of the offending line.
    pub offset: u64,
}

/// Everything a connector extracted from a run of complete lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedRecords {
    pub usage: Vec<RawUsage>,
    pub tool_outcomes: Vec<ToolOutcome>,
    pub issues: Vec<ParseIssue>,
    /// Lines read, including ones that carried no usage.
    pub lines: u64,
}

/// A usage source registered on this device, with the user's consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub id: i64,
    pub connector_id: String,
    pub root: String,
    /// Sources are read only after the user consents.
    pub enabled: bool,
    pub consented_at: Option<UtcMillis>,
    pub discovered_at: UtcMillis,
    pub last_scan_at: Option<UtcMillis>,
    pub last_success_at: Option<UtcMillis>,
    pub last_error: Option<String>,
}

/// Failure of an infrastructure port; details are for diagnostics only.
pub type PortError = Box<dyn std::error::Error + Send + Sync>;

/// Sources and consent, as persisted (implemented by the store).
pub trait SourceRepository: Send + Sync {
    /// Every registered source.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn sources(&self) -> Result<Vec<SourceRecord>, PortError>;

    /// Enables (recording consent the first time) or disables a source. `None` when the
    /// source does not exist.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn set_source_enabled(
        &self,
        id: i64,
        enabled: bool,
        now: UtcMillis,
    ) -> Result<Option<SourceRecord>, PortError>;
}

/// Control of background ingestion (implemented by the ingest worker).
pub trait IngestControl: Send + Sync {
    /// Scan every enabled source now.
    fn request_scan(&self);
    /// Sources were enabled or disabled: re-watch the enabled ones and scan.
    fn sources_changed(&self);
}

/// A usage source parser.
pub trait SourceConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;

    /// Candidate root directories, from documented defaults and location overrides.
    fn default_roots(&self, env: &DiscoveryEnv) -> Vec<PathBuf>;

    /// Whether a file (relative to a root) is a source file this connector reads. Must be
    /// a strict allow-list: everything else under the root is ignored.
    fn is_source_file(&self, relative: &Path) -> bool;

    /// Parses complete lines (each ending in `\n`) from one file.
    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords;
}
