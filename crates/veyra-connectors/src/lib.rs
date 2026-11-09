//! Usage-source connectors.
//!
//! Each connector implements [`SourceConnector`]: a pure parser from a source's bytes to
//! raw usage records. Connectors never read files, open databases or use the network;
//! the ingest pipeline does that and hands them complete lines (ADR-023).

pub mod claude_code;
mod lines;
mod shared;

use veyra_domain::source::SourceConnector;

/// Every connector shipped in this build.
#[must_use]
pub fn all() -> Vec<Box<dyn SourceConnector>> {
    vec![Box::new(claude_code::ClaudeCode::new())]
}
