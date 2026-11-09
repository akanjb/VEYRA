//! Gemini CLI.
//!
//! Reads the chat recordings in `~/.gemini/tmp` (`GEMINI_CLI_HOME` relocates the home
//! directory; the macOS sandbox uses `~/.cache/.gemini/tmp`):
//! - `<project>/chats/session-*.jsonl`: append-only records (v0.39 and later);
//! - `<project>/chats/session-*.json`: one document rewritten on each update (earlier);
//! - `<project>/chats/<session>/<agent>.jsonl`: subagent sessions.
//!
//! Every other file is ignored, notably `logs.json` and checkpoints, which hold prompts.
//! The project folder comes from `<project>/.project_root`. The format is not
//! documented; see `docs/connectors/gemini-cli.md`.

mod convert;
mod record;

use std::path::{Path, PathBuf};

use veyra_domain::connector::{Capabilities, Compression, Reading, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::{
    ConnectorDescriptor, DiscoveryEnv, FileContext, ParseIssue, ParsedRecords, SourceConnector,
};

use crate::lines;
use crate::shared::{has_suffix, plain_components};
use convert::{FileFacts, Session};
use record::Record;

/// Bump when parsing changes so files are re-read with the new rules.
const PARSER_VERSION: u32 = 1;

/// The file naming a project's folder, next to its `chats` directory.
const PROJECT_ROOT_FILE: &str = ".project_root";

/// What Gemini CLI recordings report.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Partial(
        "Conversation requests only; background requests such as history compression are not recorded",
    ),
    cache: Support::Full,
    reasoning: Support::Full,
    tools: Support::Full,
    sessions: Support::Full,
    projects: Support::Full,
};

/// The Gemini CLI connector.
#[derive(Debug, Clone)]
pub struct GeminiCli {
    descriptor: ConnectorDescriptor,
}

impl GeminiCli {
    /// # Panics
    /// Never: the identifiers are valid constants.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: ConnectorDescriptor {
                id: ConnectorId::new("gemini-cli").unwrap_or_else(|_| unreachable!()),
                app: AppId::new("gemini-cli").unwrap_or_else(|_| unreachable!()),
                display_name: "Gemini CLI",
                parser_version: PARSER_VERSION,
                documented_format: false,
                capabilities: CAPABILITIES,
            },
        }
    }
}

impl Default for GeminiCli {
    fn default() -> Self {
        Self::new()
    }
}

/// A main session file: `session-*.jsonl`, or `session-*.json` from older versions.
fn is_session(name: &str) -> bool {
    name.starts_with("session-") && (has_suffix(name, ".jsonl") || has_suffix(name, ".json"))
}

impl SourceConnector for GeminiCli {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn default_roots(&self, env: &DiscoveryEnv) -> Vec<PathBuf> {
        let home = env
            .location_overrides
            .get("GEMINI_CLI_HOME")
            .or(env.home.as_ref());
        home.map(|home| {
            vec![
                home.join(".gemini").join("tmp"),
                // Where the macOS sandbox keeps them.
                home.join(".cache").join(".gemini").join("tmp"),
            ]
        })
        .unwrap_or_default()
    }

    fn is_source_file(&self, relative: &Path) -> bool {
        match plain_components(relative).as_deref() {
            Some([_project, "chats", file]) => is_session(file),
            Some([_project, "chats", _session, file]) => has_suffix(file, ".jsonl"),
            _ => false,
        }
    }

    fn reading(&self, _relative: &Path) -> Reading {
        // Messages are re-written as they change, and older versions rewrite the whole
        // document, so each file is read in full.
        Reading::WholeFile(Compression::None)
    }

    fn side_files(&self, relative: &Path) -> Vec<PathBuf> {
        match plain_components(relative).as_deref() {
            Some([project, "chats", ..]) => vec![Path::new(project).join(PROJECT_ROOT_FILE)],
            _ => Vec::new(),
        }
    }

    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords {
        let mut out = ParsedRecords::default();
        let mut session = Session::new(facts(context));
        if context
            .relative
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            // Older versions: one document holding every message.
            out.lines = 1;
            match serde_json::from_slice::<Record>(data) {
                Ok(record) => session.record(record),
                Err(_) => out.issues.push(ParseIssue {
                    kind: "malformed-json",
                    offset: context.base_offset,
                }),
            }
        } else {
            let relevant = lines::AnyOf::new([b"\"sessionId\"", b"\"tokens\"", b"\"toolCalls\""]);
            for (offset, line) in lines::with_offsets(data, context.base_offset) {
                out.lines += 1;
                if !relevant.matches(line) {
                    continue;
                }
                match serde_json::from_slice::<Record>(line) {
                    Ok(record) => session.record(record),
                    Err(_) => out.issues.push(ParseIssue {
                        kind: "malformed-json",
                        offset,
                    }),
                }
            }
        }
        (out.usage, out.tool_outcomes) = session.into_records();
        out
    }
}

/// What the path and the project's `.project_root` file say about a session file.
fn facts(context: &FileContext<'_>) -> FileFacts {
    let parent = match plain_components(context.relative).as_deref() {
        Some([_, "chats", session, _]) => Some((*session).to_owned()),
        _ => None,
    };
    let project_root = context
        .side_files
        .iter()
        .find(|f| {
            f.relative
                .file_name()
                .is_some_and(|n| n == PROJECT_ROOT_FILE)
        })
        .and_then(|f| std::str::from_utf8(&f.bytes).ok())
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned);
    FileFacts {
        parent,
        project_root,
    }
}

#[cfg(test)]
mod tests;
