//! Usage estimated from the official data exports of chat apps (ADR-040).
//!
//! Consumer chat apps report no tokens anywhere Veyra may read. Their data exports do
//! list each message with its time and, for some apps, the model. An importer reads
//! the archive the user picks, measures each message's text, keeps only identifiers,
//! times, models and estimated counts, and never keeps the text.

pub mod archive;
pub mod chatgpt;
pub mod estimate;

use veyra_domain::connector::{Capabilities, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::import::ExportImporter;
use veyra_domain::source::ConnectorDescriptor;

pub use chatgpt::ChatGpt;

/// Bump when reading changes; it records which rules produced an estimate.
const PARSER_VERSION: u32 = 1;

/// What chat exports report.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Partial("Estimated from message text"),
    cache: Support::Partial("Exports do not say what was cached"),
    reasoning: Support::Partial("Only reasoning summaries are exported"),
    tools: Support::Partial("Tool use is counted as text"),
    sessions: Support::Full,
    projects: Support::Partial("Chats carry no working folder"),
};

fn descriptor(app: &'static str, display_name: &'static str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(format!("{app}-export")).unwrap_or_else(|_| unreachable!()),
        app: AppId::new(app).unwrap_or_else(|_| unreachable!()),
        display_name,
        parser_version: PARSER_VERSION,
        documented_format: false,
        capabilities: CAPABILITIES,
    }
}

/// Every export format Veyra reads, for the ingest layer.
#[must_use]
pub fn all() -> Vec<Box<dyn ExportImporter>> {
    vec![Box::new(ChatGpt::new())]
}

/// The applications whose exports Veyra reads.
#[must_use]
pub fn descriptors() -> Vec<ConnectorDescriptor> {
    all().iter().map(|i| i.descriptor().clone()).collect()
}

#[cfg(test)]
mod tests;
