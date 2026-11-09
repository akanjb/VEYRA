//! The connector contract (ADR-002, ADR-023).
//!
//! A connector turns the bytes of one usage source into [`RawUsage`] records. It is pure:
//! it never touches the filesystem, network or database. The ingest pipeline finds
//! files, reads new bytes incrementally, normalises records and persists them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::accuracy::AccuracyMap;
use crate::connector::{Capabilities, Reading, SideFile};
use crate::event::{Granularity, ServiceTier};
use crate::identity::{AppId, ConnectorId, Interface};
use crate::money::NanoUsd;
use crate::time::UtcMillis;
use crate::tokens::TokenCounts;
use crate::tooling::{ToolCall, ToolOutcome};

/// Static facts about a connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorDescriptor {
    pub id: ConnectorId,
    /// The application whose usage this connector reads.
    pub app: AppId,
    pub display_name: &'static str,
    /// Bumped whenever parsing changes; files parsed by an older version are re-read.
    pub parser_version: u32,
    /// Whether the vendor documents the format (false means it may change without notice).
    pub documented_format: bool,
    /// What this source can report, shown to the user.
    pub capabilities: Capabilities,
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
    /// The side files the connector asked for (see [`SourceConnector::side_files`]) that
    /// exist.
    pub side_files: &'a [SideFile],
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
    /// The service that routed the request to the provider, e.g. `github-copilot`.
    pub gateway: Option<String>,
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

    /// Ingestion health of every registered source, with issues found since `since`.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn health(&self, since: UtcMillis) -> Result<Vec<SourceHealth>, PortError>;
}

/// How ingestion of one source is going.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceHealth {
    pub source_id: i64,
    /// Files being read.
    pub files_active: u64,
    /// Files that disappeared; their events are kept.
    pub files_missing: u64,
    /// When a file of this source was last read.
    pub last_ingested_at: Option<UtcMillis>,
    /// Problems found by runs started in the requested window, per kind:
    /// `(kind, occurrences)`.
    pub recent_issues: Vec<(String, u64)>,
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

    /// How a source file (relative to the root) is read.
    fn reading(&self, relative: &Path) -> Reading;

    /// Small files, relative to the root, that the parser needs as context for a source
    /// file. Missing ones are skipped.
    fn side_files(&self, _relative: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Parses data read as [`reading`](Self::reading) declares: complete lines of an
    /// append-only file, a whole file, or one JSON line per database row.
    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords;
}
