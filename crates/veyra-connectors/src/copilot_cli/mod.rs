//! GitHub Copilot CLI.
//!
//! Reads the session event logs in `~/.copilot` (`COPILOT_HOME` relocates it):
//! `session-state/<session>/events.jsonl`, and `session-state/<session>.jsonl` from
//! versions before 0.0.380. Debug logs, configuration and the session store are never
//! read. Usage is only written as a per-model summary when a session ends, so it is
//! recorded per session run rather than per request. The format is documented only in
//! part; see `docs/connectors/copilot-cli.md`.

mod convert;
mod record;

use std::path::{Path, PathBuf};

use veyra_domain::connector::{Capabilities, Compression, Reading, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::{
    ConnectorDescriptor, DiscoveryEnv, FileContext, ParseIssue, ParsedRecords, SourceConnector,
};

use crate::lines;
use crate::shared::plain_components;
use convert::Session;
use record::Event;

/// Bump when parsing changes so files are re-read with the new rules.
const PARSER_VERSION: u32 = 1;

/// What Copilot CLI session logs report.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Partial(
        "Totals per session and model, written when a session ends; a session that crashed is not counted",
    ),
    cache: Support::Partial(
        "Cache reads; whether cache writes are counted in input is not documented",
    ),
    reasoning: Support::Partial("Only when Copilot reports it"),
    tools: Support::Full,
    sessions: Support::Partial("Subagent usage is included in its session"),
    projects: Support::Full,
};

/// The GitHub Copilot CLI connector.
#[derive(Debug, Clone)]
pub struct CopilotCli {
    descriptor: ConnectorDescriptor,
}

impl CopilotCli {
    /// # Panics
    /// Never: the identifiers are valid constants.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: ConnectorDescriptor {
                id: ConnectorId::new("copilot-cli").unwrap_or_else(|_| unreachable!()),
                app: AppId::new("copilot-cli").unwrap_or_else(|_| unreachable!()),
                display_name: "GitHub Copilot CLI",
                parser_version: PARSER_VERSION,
                documented_format: false,
                capabilities: CAPABILITIES,
            },
        }
    }
}

impl Default for CopilotCli {
    fn default() -> Self {
        Self::new()
    }
}

/// The session id a file belongs to, from its path.
fn session_of(relative: &Path) -> Option<String> {
    match plain_components(relative).as_deref() {
        Some(["session-state", session, "events.jsonl"]) => Some((*session).to_owned()),
        Some(["session-state", file]) => file.strip_suffix(".jsonl").map(str::to_owned),
        _ => None,
    }
}

impl SourceConnector for CopilotCli {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn default_roots(&self, env: &DiscoveryEnv) -> Vec<PathBuf> {
        env.location_overrides
            .get("COPILOT_HOME")
            .cloned()
            .or_else(|| env.home.as_ref().map(|home| home.join(".copilot")))
            .into_iter()
            .collect()
    }

    fn is_source_file(&self, relative: &Path) -> bool {
        session_of(relative).is_some_and(|session| !session.is_empty())
    }

    fn reading(&self, _relative: &Path) -> Reading {
        // How a summary counts depends on the summaries before it.
        Reading::WholeFile(Compression::None)
    }

    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords {
        let mut out = ParsedRecords::default();
        let mut session = Session::new(session_of(context.relative));
        let relevant = lines::AnyOf::new([b"\"session.", b"\"tool.execution_"]);
        for (offset, line) in lines::with_offsets(data, context.base_offset) {
            out.lines += 1;
            if !relevant.matches(line) {
                continue;
            }
            match serde_json::from_slice::<Event>(line) {
                Ok(event) => session.event(event),
                Err(_) => out.issues.push(ParseIssue {
                    kind: "malformed-json",
                    offset,
                }),
            }
        }
        out.usage = std::mem::take(&mut session.usage);
        out.tool_outcomes = session.outcomes;
        out
    }
}

#[cfg(test)]
mod tests;
