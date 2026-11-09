//! Turns the records of one session file into usage records.
//!
//! A message is written again in full each time it changes (tokens arriving late, tool
//! status updates), so the last copy of each message id counts, once. Messages removed
//! by a rewind still count: their tokens were spent. Usage occasionally attaches to a
//! message without a model; it takes the model of the nearest message that has one.

use std::collections::HashMap;

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::source::RawUsage;
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::{ToolCall, ToolOutcome};

use super::record::{Message, Record, Tokens};
use crate::shared::utc_millis;

/// What the file's path says about the session.
#[derive(Debug, Default)]
pub struct FileFacts {
    /// The parent session id, for subagent files (`chats/<parent>/<agent>.jsonl`).
    pub parent: Option<String>,
    /// The project folder, from the project's `.project_root` file.
    pub project_root: Option<String>,
}

/// Accumulates one session file.
#[derive(Debug, Default)]
pub struct Session {
    facts: FileFacts,
    id: Option<String>,
    subagent: bool,
    directories: Vec<String>,
    /// Message ids in order of first appearance, and the last copy of each.
    order: Vec<String>,
    latest: HashMap<String, Message>,
}

impl Session {
    pub fn new(facts: FileFacts) -> Self {
        Self {
            subagent: facts.parent.is_some(),
            facts,
            ..Self::default()
        }
    }

    pub fn record(&mut self, record: Record) {
        if self.id.is_none() {
            self.id = record
                .session_id
                .or_else(|| record.set.as_ref().and_then(|s| s.session_id.clone()));
        }
        if record.kind.as_deref() == Some("subagent") {
            self.subagent = true;
        }
        if let Some(directories) = record.directories {
            self.directories = directories;
        }
        let listed = record
            .messages
            .into_iter()
            .chain(record.set.and_then(|s| s.messages))
            .flatten();
        for message in listed.chain(std::iter::once(record.message)) {
            self.message(message);
        }
    }

    fn message(&mut self, message: Message) {
        let Some(id) = message.id.clone() else { return };
        if !self.latest.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.latest.insert(id, message);
    }

    /// Usage of every model response, and the outcome of every finished tool call.
    pub fn into_records(self) -> (Vec<RawUsage>, Vec<ToolOutcome>) {
        let Some(session) = self.id.clone() else {
            return (Vec::new(), Vec::new());
        };
        let messages: Vec<&Message> = self.order.iter().map(|id| &self.latest[id]).collect();
        let models = nearest_models(&messages);
        let mut usage = Vec::new();
        let mut outcomes = Vec::new();
        for (message, model) in messages.iter().zip(models) {
            outcomes.extend(tool_outcomes(message));
            if let Some(raw) = self.usage(&session, message, model) {
                usage.push(raw);
            }
        }
        (usage, outcomes)
    }

    fn usage(&self, session: &str, message: &Message, model: Option<&str>) -> Option<RawUsage> {
        let tokens = message.tokens?;
        if message.kind.as_deref() != Some("gemini") {
            return None;
        }
        let occurred_at = message.timestamp.as_deref().and_then(utc_millis)?;
        let id = message.id.as_deref()?;
        Some(RawUsage {
            natural_key: format!("gemini:{session}:{id}"),
            precedence: 0,
            occurred_at,
            model: model.map(str::to_owned),
            provider: Some("google".to_owned()),
            gateway: None,
            tokens: canonical(tokens),
            requests: Some(1),
            reported_cost: None,
            interface: Interface::Cli,
            service_tier: ServiceTier::Standard,
            inference_geo_us: false,
            granularity: Granularity::Request,
            session_key: Some(format!("gemini:{session}")),
            parent_session_key: self.facts.parent.as_ref().map(|p| format!("gemini:{p}")),
            is_subagent: self.subagent,
            cwd: self
                .facts
                .project_root
                .clone()
                .or_else(|| self.directories.first().cloned()),
            tool_version: None,
            accuracy: AccuracyMap {
                tokens: Accuracy::Exact,
                cache: Accuracy::Exact,
                reasoning: Accuracy::Exact,
                requests: Accuracy::Exact,
                // Cost is always derived from tokens and published prices.
                cost: Accuracy::Estimated,
            },
            tool_calls: message
                .tool_calls
                .iter()
                .flatten()
                .filter_map(|call| {
                    Some(ToolCall {
                        id: call.id.clone()?,
                        name: call.name.clone()?,
                    })
                })
                .collect(),
        })
    }
}

/// Each message's model, or else the model of the nearest earlier message with one, or
/// else of the nearest later one.
fn nearest_models<'m>(messages: &[&'m Message]) -> Vec<Option<&'m str>> {
    let mut previous: Option<&str> = None;
    let mut models: Vec<Option<&str>> = messages
        .iter()
        .map(|m| {
            previous = m.model.as_deref().or(previous);
            previous
        })
        .collect();
    let mut next: Option<&str> = None;
    for (model, message) in models.iter_mut().zip(messages).rev() {
        next = message.model.as_deref().or(next);
        if model.is_none() {
            *model = next;
        }
    }
    models
}

/// Canonical tokens. `input` includes cached tokens; tool-use prompt tokens are billed
/// as input but reported apart from it; thoughts are reasoning, reported apart from
/// output.
fn canonical(tokens: Tokens) -> TokenCounts {
    let cached = tokens.cached.min(tokens.input);
    TokenCounts {
        input_uncached: Some(tokens.input - cached + tokens.tool),
        cache_read: Some(cached),
        output_total: Some(tokens.output + tokens.thoughts),
        reasoning: Some(tokens.thoughts),
        tool_tokens: Some(tokens.tool),
        ..TokenCounts::default()
    }
}

/// Outcomes of the message's finished tool calls.
fn tool_outcomes(message: &Message) -> impl Iterator<Item = ToolOutcome> + '_ {
    message.tool_calls.iter().flatten().filter_map(|call| {
        let is_error = match call.status.as_deref()? {
            "success" => false,
            "error" => true,
            _ => return None,
        };
        Some(ToolOutcome {
            tool_call_id: call.id.clone()?,
            is_error,
        })
    })
}
