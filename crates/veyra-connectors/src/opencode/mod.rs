//! OpenCode.
//!
//! Reads `opencode.db` (or `opencode-<channel>.db` for preview channels) in OpenCode's
//! data directory: `$XDG_DATA_HOME/opencode`, else `~/.local/share/opencode` on every
//! operating system. `OPENCODE_DB` may relocate the file. The database belongs to
//! OpenCode and is live, so the pipeline reads it through a private snapshot
//! (ADR-015). Sessions stored as JSON files by versions before 1.2 are not read; OpenCode
//! moves them into the database. The format is not documented; see
//! `docs/connectors/opencode.md`.

mod convert;
mod queries;

use std::path::{Path, PathBuf};

use veyra_domain::connector::{Capabilities, Reading, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::{
    ConnectorDescriptor, DiscoveryEnv, FileContext, ParseIssue, ParsedRecords, SourceConnector,
};

use crate::lines;
use crate::shared::has_suffix;
use convert::{Row, Rows};

/// Bump when parsing changes so the database is re-read with the new rules.
const PARSER_VERSION: u32 = 1;

/// What the OpenCode database reports.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Full,
    cache: Support::Full,
    reasoning: Support::Full,
    tools: Support::Full,
    sessions: Support::Full,
    projects: Support::Full,
};

/// The OpenCode connector.
#[derive(Debug, Clone)]
pub struct OpenCode {
    descriptor: ConnectorDescriptor,
}

impl OpenCode {
    /// # Panics
    /// Never: the identifiers are valid constants.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: ConnectorDescriptor {
                id: ConnectorId::new("opencode").unwrap_or_else(|_| unreachable!()),
                app: AppId::new("opencode").unwrap_or_else(|_| unreachable!()),
                display_name: "OpenCode",
                parser_version: PARSER_VERSION,
                documented_format: false,
                capabilities: CAPABILITIES,
            },
        }
    }
}

impl Default for OpenCode {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceConnector for OpenCode {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn default_roots(&self, env: &DiscoveryEnv) -> Vec<PathBuf> {
        let overrides = &env.location_overrides;
        if let Some(parent) = overrides
            .get("OPENCODE_DB")
            .filter(|db| db.is_absolute())
            .and_then(|db| db.parent())
        {
            return vec![parent.to_path_buf()];
        }
        overrides
            .get("XDG_DATA_HOME")
            .cloned()
            .or_else(|| {
                env.home
                    .as_ref()
                    .map(|home| home.join(".local").join("share"))
            })
            .map(|data| data.join("opencode"))
            .into_iter()
            .collect()
    }

    fn is_source_file(&self, relative: &Path) -> bool {
        let Some(name) = relative
            .to_str()
            .filter(|_| relative.components().count() == 1)
        else {
            return false;
        };
        // `opencode-next.db` is an import source merged into `opencode.db`.
        name == "opencode.db"
            || (name.starts_with("opencode-")
                && has_suffix(name, ".db")
                && name != "opencode-next.db")
    }

    fn reading(&self, _relative: &Path) -> Reading {
        Reading::Database(queries::QUERIES)
    }

    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords {
        let mut out = ParsedRecords::default();
        let mut rows = Rows::default();
        for (offset, line) in lines::with_offsets(data, context.base_offset) {
            out.lines += 1;
            let added = serde_json::from_slice::<Row>(line).is_ok_and(|row| rows.add(row));
            if !added {
                out.issues.push(ParseIssue {
                    kind: "invalid-record",
                    offset,
                });
            }
        }
        out.tool_outcomes = std::mem::take(&mut rows.outcomes);
        out.usage = rows.into_usage();
        out
    }
}

#[cfg(test)]
mod tests;
