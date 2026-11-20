//! The ChatGPT data export (Settings → Data controls → Export data).
//!
//! The archive's `conversations.json` lists every conversation as a tree of messages
//! (`mapping`), each with its author's role, creation time and, for replies, the model
//! that wrote it. Every assistant message becomes one Estimated record:
//! - output: the message's own text;
//! - input: the text of every message above it in the tree, which is what the model
//!   was sent, capped at [`MAX_CONTEXT`] because ChatGPT trims long conversations.
//!
//! Text is read only to be measured, one conversation at a time, and is never kept.
//! Images and files count nothing, so long multimodal chats are underestimated.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;
use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::import::{ExportImporter, ImportCounts, ImportError, ReadSeek};
use veyra_domain::source::{ConnectorDescriptor, RawUsage};
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;

use super::archive::{each_document, each_element};
use super::{descriptor, estimate};

/// Largest context counted for one reply.
pub const MAX_CONTEXT: u64 = 128_000;

/// Content fields that name things rather than hold text.
const NOT_TEXT: [&str; 3] = ["content_type", "asset_pointer", "language"];

#[derive(Deserialize)]
struct Conversation {
    id: Option<String>,
    #[serde(rename = "conversation_id")]
    alt_id: Option<String>,
    create_time: Option<f64>,
    default_model_slug: Option<String>,
    /// Required: what tells this export apart from others with the same file name.
    mapping: HashMap<String, Node>,
}

#[derive(Deserialize)]
struct Node {
    parent: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: String,
    author: Author,
    create_time: Option<f64>,
    content: Option<Value>,
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Author {
    role: String,
}

#[derive(Deserialize)]
struct Metadata {
    model_slug: Option<String>,
}

/// Reads ChatGPT data exports.
#[derive(Debug, Clone)]
pub struct ChatGpt {
    descriptor: ConnectorDescriptor,
}

impl ChatGpt {
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: descriptor("chatgpt", "ChatGPT"),
        }
    }
}

impl Default for ChatGpt {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportImporter for ChatGpt {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn import(
        &self,
        file: &mut dyn ReadSeek,
        emit: &mut dyn FnMut(RawUsage) -> Result<(), ImportError>,
    ) -> Result<Option<ImportCounts>, ImportError> {
        let mut counts = ImportCounts::default();
        let found = each_document(file, is_conversations, &mut |reader| {
            each_element(reader, &mut |element| {
                conversation(element, &mut counts, emit)
            })
        });
        match found {
            Ok(true) if counts.conversations > 0 => Ok(Some(counts)),
            Ok(_) | Err(ImportError::NotRecognized) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// `conversations.json`, or one of its numbered parts in large exports.
fn is_conversations(name: &str) -> bool {
    name.starts_with("conversations")
        && std::path::Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

fn conversation(
    element: &RawValue,
    counts: &mut ImportCounts,
    emit: &mut dyn FnMut(RawUsage) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    let parsed = serde_json::from_str::<Conversation>(element.get());
    let Some((id, conversation)) = parsed
        .ok()
        .and_then(|c| Some((c.id.clone().or_else(|| c.alt_id.clone())?, c)))
    else {
        // The first element decides whether this is a ChatGPT export at all.
        if counts.conversations == 0 && counts.skipped == 0 {
            return Err(ImportError::NotRecognized);
        }
        counts.skipped += 1;
        return Ok(());
    };
    counts.conversations += 1;
    for usage in replies(&id, &conversation) {
        counts.replies += 1;
        emit(usage)?;
    }
    Ok(())
}

/// One record per assistant message, in time order.
fn replies(id: &str, conversation: &Conversation) -> Vec<RawUsage> {
    let own: HashMap<&str, u64> = conversation
        .mapping
        .iter()
        .map(|(key, node)| {
            let tokens = node
                .message
                .as_ref()
                .and_then(|m| m.content.as_ref())
                .map_or(0, text_tokens);
            (key.as_str(), tokens)
        })
        .collect();
    let mut above = HashMap::new();
    let mut out: Vec<RawUsage> = conversation
        .mapping
        .iter()
        .filter_map(|(key, node)| {
            let message = node.message.as_ref()?;
            let output = own.get(key.as_str()).copied().unwrap_or(0);
            if message.author.role != "assistant" || output == 0 {
                return None;
            }
            let at = message.create_time.or(conversation.create_time)?;
            let input = context(conversation, &own, &mut above, node.parent.as_deref());
            let model = message
                .metadata
                .as_ref()
                .and_then(|m| m.model_slug.clone())
                .or_else(|| conversation.default_model_slug.clone());
            Some(record(id, message, model, at, input, output))
        })
        .collect();
    out.sort_by_key(|u| (u.occurred_at, u.natural_key.clone()));
    out
}

/// Estimated tokens of every text field in a message's content.
fn text_tokens(content: &Value) -> u64 {
    match content {
        Value::String(text) => estimate::tokens(text),
        Value::Array(items) => items.iter().map(text_tokens).sum(),
        Value::Object(fields) => fields
            .iter()
            .filter(|(name, _)| !NOT_TEXT.contains(&name.as_str()))
            .map(|(_, value)| text_tokens(value))
            .sum(),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    }
}

/// Tokens of `node` and every message above it, remembered per node. Walks up without
/// recursion, and stops at a missing parent or after more steps than there are nodes,
/// which only a damaged export with a loop can need.
fn context<'a>(
    conversation: &'a Conversation,
    own: &HashMap<&'a str, u64>,
    above: &mut HashMap<&'a str, u64>,
    node: Option<&'a str>,
) -> u64 {
    let mut path = Vec::new();
    let mut base = 0;
    let mut next = node;
    while let Some(key) = next {
        if let Some(&known) = above.get(key) {
            base = known;
            break;
        }
        if path.len() > conversation.mapping.len() {
            break;
        }
        path.push(key);
        next = conversation
            .mapping
            .get(key)
            .and_then(|n| n.parent.as_deref());
    }
    for key in path.into_iter().rev() {
        base += own.get(key).copied().unwrap_or(0);
        above.insert(key, base);
    }
    base.min(MAX_CONTEXT)
}

fn record(
    conversation: &str,
    message: &Message,
    model: Option<String>,
    at: f64,
    input: u64,
    output: u64,
) -> RawUsage {
    let reasoning = matches!(
        message
            .content
            .as_ref()
            .and_then(|c| c.get("content_type"))
            .and_then(Value::as_str),
        Some("thoughts" | "reasoning_recap")
    );
    let tokens = TokenCounts {
        input_uncached: Some(input),
        output_total: Some(output),
        reasoning: Some(if reasoning { output } else { 0 }),
        ..TokenCounts::default()
    };
    RawUsage {
        natural_key: format!("chatgpt:{conversation}:{}", message.id),
        precedence: i64::try_from(input + output).unwrap_or(i64::MAX),
        occurred_at: seconds(at),
        model,
        provider: Some("openai".to_owned()),
        gateway: None,
        tokens,
        requests: Some(1),
        reported_cost: None,
        // The export does not say which client was used.
        interface: Interface::Browser,
        service_tier: ServiceTier::Standard,
        inference_geo_us: false,
        granularity: Granularity::Request,
        session_key: Some(format!("chatgpt:{conversation}")),
        parent_session_key: None,
        is_subagent: false,
        cwd: None,
        tool_version: None,
        accuracy: AccuracyMap::uniform(Accuracy::Estimated),
        tool_calls: Vec::new(),
    }
}

/// Unix seconds with a fraction, as ChatGPT writes them.
#[allow(
    clippy::cast_possible_truncation,
    reason = "clamped to a range far inside i64"
)]
fn seconds(at: f64) -> UtcMillis {
    let millis = (at * 1000.0).round();
    UtcMillis(if millis.is_finite() {
        millis.clamp(0.0, 1e15) as i64
    } else {
        0
    })
}
