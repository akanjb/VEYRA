//! Claude Code (CLI, desktop Code tab, IDE extensions).
//!
//! Reads `~/.claude/projects/<project>/<session>.jsonl` and
//! `~/.claude/projects/<project>/<session>/subagents/<agent>.jsonl` (`CLAUDE_CONFIG_DIR`
//! relocates `~/.claude`). Every other file under the root (tool outputs, images, memory
//! files) is ignored. The line format is undocumented; see `docs/connectors/claude-code.md`
//! for the observed structure and the rules applied.

mod convert;
mod record;

use std::path::{Path, PathBuf};

use veyra_domain::connector::{Capabilities, Reading, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::{
    ConnectorDescriptor, DiscoveryEnv, FileContext, ParseIssue, ParsedRecords, SourceConnector,
};

use crate::lines;
use crate::shared::{BestByKey, plain_components};

/// Bump when parsing changes so files are re-read with the new rules.
const PARSER_VERSION: u32 = 1;

/// What Claude Code logs report.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Full,
    cache: Support::Full,
    reasoning: Support::Partial("Counted within output; not reported separately"),
    tools: Support::Full,
    sessions: Support::Full,
    projects: Support::Full,
};

/// The Claude Code connector.
#[derive(Debug, Clone)]
pub struct ClaudeCode {
    descriptor: ConnectorDescriptor,
}

impl ClaudeCode {
    /// # Panics
    /// Never: the identifiers are valid constants.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: ConnectorDescriptor {
                id: ConnectorId::new("claude-code").unwrap_or_else(|_| unreachable!()),
                app: AppId::new("claude-code").unwrap_or_else(|_| unreachable!()),
                display_name: "Claude Code",
                parser_version: PARSER_VERSION,
                documented_format: false,
                capabilities: CAPABILITIES,
            },
        }
    }
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceConnector for ClaudeCode {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn default_roots(&self, env: &DiscoveryEnv) -> Vec<PathBuf> {
        let config = env
            .location_overrides
            .get("CLAUDE_CONFIG_DIR")
            .cloned()
            .or_else(|| env.home.as_ref().map(|home| home.join(".claude")));
        config.map(|dir| dir.join("projects")).into_iter().collect()
    }

    fn is_source_file(&self, relative: &Path) -> bool {
        // Exactly `<non-empty stem>.jsonl`, as Claude Code writes it.
        let is_jsonl = |name: &str| {
            let name = Path::new(name);
            name.extension().is_some_and(|ext| ext == "jsonl")
                && name.file_stem().is_some_and(|stem| !stem.is_empty())
        };
        match plain_components(relative).as_deref() {
            Some([_project, file]) => is_jsonl(file),
            Some([_project, _session, "subagents", file]) => is_jsonl(file),
            _ => false,
        }
    }

    fn reading(&self, _relative: &Path) -> Reading {
        Reading::AppendLines
    }

    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords {
        let file = convert::FileKind::of(context.relative);
        let mut out = ParsedRecords::default();
        let mut best = BestByKey::default();
        // Exact prefilter: only these lines can carry usage or tool outcomes.
        let relevant = lines::AnyOf::new([b"\"usage\"", b"\"tool_result\""]);

        for (offset, line) in lines::with_offsets(data, context.base_offset) {
            out.lines += 1;
            if !relevant.matches(line) {
                continue;
            }
            let Ok(record) = serde_json::from_slice::<record::Line>(line) else {
                out.issues.push(ParseIssue {
                    kind: "malformed-json",
                    offset,
                });
                continue;
            };
            out.tool_outcomes.extend(convert::tool_outcomes(&record));
            match convert::usage(&record, &file) {
                Ok(Some(usage)) => best.add(usage),
                Ok(None) => {}
                Err(kind) => out.issues.push(ParseIssue { kind, offset }),
            }
        }
        out.usage = best.into_records();
        out
    }
}

#[cfg(test)]
mod tests;
