//! Codex (CLI, IDE extension and desktop app).
//!
//! Reads the rollout files in `$CODEX_HOME` (default `~/.codex`):
//! `sessions/YYYY/MM/DD/rollout-*.jsonl` and `archived_sessions/rollout-*.jsonl`, either
//! of which may be compressed to `.jsonl.zst`. Everything else under the root is ignored,
//! including the history file and the application's own databases, which hold prompts.
//! The format is not documented; see `docs/connectors/codex.md`.

mod convert;
mod record;

use std::path::{Path, PathBuf};

use veyra_domain::connector::{Capabilities, Compression, Reading, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::{
    ConnectorDescriptor, DiscoveryEnv, FileContext, ParseIssue, ParsedRecords, SourceConnector,
};
use veyra_domain::time::UtcMillis;

use crate::lines;
use crate::shared::{has_suffix, plain_components, utc_millis};
use convert::Rollout;
use record::{Event, Line, ResponseItem, SessionMeta, TurnContext, UsageRecord};
use serde_json::value::RawValue;

/// Bump when parsing changes so files are re-read with the new rules.
const PARSER_VERSION: u32 = 1;

/// What Codex rollouts report.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Full,
    cache: Support::Full,
    reasoning: Support::Full,
    tools: Support::Full,
    sessions: Support::Full,
    projects: Support::Full,
};

/// The Codex connector.
#[derive(Debug, Clone)]
pub struct Codex {
    descriptor: ConnectorDescriptor,
}

impl Codex {
    /// # Panics
    /// Never: the identifiers are valid constants.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: ConnectorDescriptor {
                id: ConnectorId::new("codex").unwrap_or_else(|_| unreachable!()),
                app: AppId::new("codex").unwrap_or_else(|_| unreachable!()),
                display_name: "Codex",
                parser_version: PARSER_VERSION,
                documented_format: false,
                capabilities: CAPABILITIES,
            },
        }
    }
}

impl Default for Codex {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a file name is a rollout, plain or compressed.
fn is_rollout(name: &str) -> bool {
    name.starts_with("rollout-")
        && (has_suffix(name, ".jsonl") || has_suffix(name, ".jsonl.zst"))
        && name.len() > "rollout-.jsonl".len()
}

impl SourceConnector for Codex {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn default_roots(&self, env: &DiscoveryEnv) -> Vec<PathBuf> {
        env.location_overrides
            .get("CODEX_HOME")
            .cloned()
            .or_else(|| env.home.as_ref().map(|home| home.join(".codex")))
            .into_iter()
            .collect()
    }

    fn is_source_file(&self, relative: &Path) -> bool {
        let digits =
            |part: &str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
        match plain_components(relative).as_deref() {
            Some(["sessions", year, month, day, file]) => {
                digits(year, 4) && digits(month, 2) && digits(day, 2) && is_rollout(file)
            }
            Some(["archived_sessions", file]) => is_rollout(file),
            _ => false,
        }
    }

    fn reading(&self, relative: &Path) -> Reading {
        let compressed = relative
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zst"));
        Reading::WholeFile(if compressed {
            Compression::Zstd
        } else {
            Compression::None
        })
    }

    fn parse(&self, context: &FileContext<'_>, data: &[u8]) -> ParsedRecords {
        let mut out = ParsedRecords::default();
        let mut rollout = Rollout::default();
        let mut has_meta = false;
        // Exact prefilter: only these line types carry what Veyra reads.
        let relevant = lines::AnyOf::new([
            b"\"session_meta\"",
            b"\"turn_context\"",
            b"\"token_usage_record\"",
            b"\"event_msg\"",
            b"_call\"",
        ]);
        for (offset, bytes) in lines::with_offsets(data, context.base_offset) {
            out.lines += 1;
            if !relevant.matches(bytes) {
                continue;
            }
            let Ok(line) = serde_json::from_slice::<Line<'_>>(bytes) else {
                out.issues.push(ParseIssue {
                    kind: "malformed-json",
                    offset,
                });
                continue;
            };
            let at = line.timestamp.as_deref().and_then(utc_millis);
            match apply(&mut rollout, &line, at, &mut has_meta) {
                Ok(()) => {}
                Err(kind) => out.issues.push(ParseIssue { kind, offset }),
            }
        }
        if !has_meta && out.lines > 0 {
            out.issues.push(ParseIssue {
                kind: "missing-session-meta",
                offset: context.base_offset,
            });
            return out;
        }
        out.tool_outcomes = std::mem::take(&mut rollout.outcomes);
        out.usage = rollout.into_usage();
        out
    }
}

/// A payload as the record type it must be.
fn decode<'a, T: serde::Deserialize<'a>>(text: &'a str) -> Result<T, &'static str> {
    serde_json::from_str(text).map_err(|_| "invalid-record")
}

/// Applies one line to the file's state; the error names the problem.
fn apply(
    rollout: &mut Rollout,
    line: &Line<'_>,
    at: Option<UtcMillis>,
    has_meta: &mut bool,
) -> Result<(), &'static str> {
    let payload = || line.payload.map(RawValue::get).ok_or("missing-payload");
    match line.kind.as_str() {
        "session_meta" => {
            let meta: SessionMeta = decode(payload()?)?;
            *has_meta = true;
            rollout.session_meta(meta);
        }
        "turn_context" => {
            let turn: TurnContext = decode(payload()?)?;
            rollout.turn_context(turn.model, turn.cwd);
        }
        "response_item" => {
            let item: ResponseItem = decode(payload()?)?;
            rollout.response_item(&item);
        }
        "token_usage_record" => {
            let record: UsageRecord = decode(payload()?)?;
            let at = at.ok_or("invalid-timestamp")?;
            rollout.usage_record(
                at,
                record.thread_id.as_deref(),
                record.response_id.as_deref(),
                record.usage,
            );
        }
        "event_msg" => {
            let event: Event = decode(payload()?)?;
            let at = at.ok_or("invalid-timestamp")?;
            rollout.event(at, event);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
