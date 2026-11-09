//! The parts of a Gemini CLI session record Veyra reads. Content fields (message text,
//! thoughts, tool arguments and results) are left out, so serde skips them unread.

use serde::Deserialize;

/// One record: the session metadata line, a message, a `$set` update, or (in files
/// written before v0.39) the whole session document with its `messages`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Record {
    pub session_id: Option<String>,
    /// `"main"` or `"subagent"`, on the metadata line.
    pub kind: Option<String>,
    /// The project directories, on the metadata line.
    pub directories: Option<Vec<String>>,
    /// The whole message list: the document of older versions, or an old checkpoint.
    pub messages: Option<Vec<Message>>,
    #[serde(rename = "$set")]
    pub set: Option<Set>,
    /// Message fields, on message lines.
    #[serde(flatten)]
    pub message: Message,
}

/// A metadata update; a resumed session may change its id, and old checkpoints carry
/// the whole message list.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Set {
    pub session_id: Option<String>,
    pub messages: Option<Vec<Message>>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Message {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub timestamp: Option<String>,
    pub model: Option<String>,
    pub tokens: Option<Tokens>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Token counts as the Gemini API reports them: `input` includes `cached`; `thoughts`
/// and `tool` (tool-use prompt tokens) are reported apart from `input` and `output`.
#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
    pub thoughts: u64,
    pub tool: u64,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct ToolCall {
    pub id: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
}
