//! The subset of a Claude Code log line that Veyra reads. Unknown fields are ignored and
//! free-text content is skipped without being retained (`IgnoredAny`).

use std::fmt;

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub timestamp: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub is_sidechain: Option<bool>,
    pub cwd: Option<String>,
    pub version: Option<String>,
    pub entrypoint: Option<String>,
    pub request_id: Option<String>,
    pub message: Option<Message>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: Option<String>,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub usage: Option<Usage>,
    #[serde(default)]
    pub content: Content,
}

/// Message content: an array of blocks, or anything else (free text), which is skipped
/// without being copied. A custom visitor avoids `untagged`, which would buffer the text.
#[derive(Debug, Default)]
pub enum Content {
    Blocks(Vec<Block>),
    #[default]
    Other,
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ContentVisitor;

        impl<'de> Visitor<'de> for ContentVisitor {
            type Value = Content;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("message content")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Content, A::Error> {
                let mut blocks = Vec::new();
                while let Some(block) = seq.next_element::<Block>()? {
                    blocks.push(block);
                }
                Ok(Content::Blocks(blocks))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Content, A::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Content::Other)
            }

            fn visit_str<E: de::Error>(self, _: &str) -> Result<Content, E> {
                Ok(Content::Other)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Content, E> {
                Ok(Content::Other)
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Content, E> {
                Ok(Content::Other)
            }

            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Content, E> {
                Ok(Content::Other)
            }

            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Content, E> {
                Ok(Content::Other)
            }

            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Content, E> {
                Ok(Content::Other)
            }
        }

        deserializer.deserialize_any(ContentVisitor)
    }
}

/// A content block. Only identifiers, tool names and error flags are read.
#[derive(Debug, Deserialize)]
pub struct Block {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub tool_use_id: Option<String>,
    pub is_error: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation: Option<CacheCreation>,
    pub output_tokens_details: Option<OutputDetails>,
    pub service_tier: Option<String>,
    pub inference_geo: Option<String>,
    pub speed: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CacheCreation {
    pub ephemeral_5m_input_tokens: Option<u64>,
    pub ephemeral_1h_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct OutputDetails {
    pub thinking_tokens: Option<u64>,
}
