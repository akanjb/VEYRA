//! Claude Code (CLI, desktop Code tab, IDE extensions).
//!
//! Reads `~/.claude/projects/<project>/<session>.jsonl` and
//! `~/.claude/projects/<project>/<session>/subagents/<agent>.jsonl` (`CLAUDE_CONFIG_DIR`
//! relocates `~/.claude`). Every other file under the root (tool outputs, images, memory
//! files) is ignored. The line format is undocumented; see `docs/connectors/claude-code.md`
//! for the observed structure and the rules applied.

mod convert;
mod record;

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::{
    ConnectorDescriptor, DiscoveryEnv, FileContext, ParseIssue, ParsedRecords, RawUsage,
    SourceConnector, SourceFormat,
};

use crate::lines;

/// Bump when parsing changes so files are re-read with the new rules.
const PARSER_VERSION: u32 = 1;

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
                format: SourceFormat::JsonLines,
                documented_format: false,
            },
        }
    }
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self::new()
    }
}

/// Plain path components, or `None` if the path has any other kind of component.
fn plain_components(path: &Path) -> Option<Vec<&str>> {
    path.components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect()
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

    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords {
        let file = convert::FileKind::of(context.relative);
        let mut out = ParsedRecords::default();
        let mut best: HashMap<String, usize> = HashMap::new();
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
                Ok(Some(usage)) => merge(&mut out.usage, &mut best, usage),
                Ok(None) => {}
                Err(kind) => out.issues.push(ParseIssue { kind, offset }),
            }
        }
        out
    }
}

/// Keeps one record per request: the most complete observation, with tool calls from
/// every line of the request combined (each streamed line carries one content block).
fn merge(records: &mut Vec<RawUsage>, best: &mut HashMap<String, usize>, candidate: RawUsage) {
    let Some(&index) = best.get(&candidate.natural_key) else {
        best.insert(candidate.natural_key.clone(), records.len());
        records.push(candidate);
        return;
    };
    let current = &mut records[index];
    let mut calls = std::mem::take(&mut current.tool_calls);
    for call in &candidate.tool_calls {
        if !calls.iter().any(|c| c.id == call.id) {
            calls.push(call.clone());
        }
    }
    let better = candidate.precedence > current.precedence
        || (candidate.precedence == current.precedence
            && candidate.occurred_at < current.occurred_at);
    if better {
        *current = candidate;
    }
    current.tool_calls = calls;
}

#[cfg(test)]
mod tests;
