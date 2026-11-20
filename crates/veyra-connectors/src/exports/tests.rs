#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

use std::io::{Cursor, Write as _};

use serde_json::{Value, json};
use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::import::{ExportImporter as _, ImportCounts, ImportError};
use veyra_domain::source::RawUsage;
use veyra_domain::time::UtcMillis;

use super::ChatGpt;
use super::chatgpt::MAX_CONTEXT;

/// A message node: `text` of 4 × `tokens` ASCII characters estimates to `tokens`.
fn node(id: &str, parent: Option<&str>, role: &str, tokens: usize, extra: Value) -> Value {
    let mut message = json!({
        "id": id,
        "author": {"role": role},
        "create_time": 1_762_600_000.25,
        "content": {"content_type": "text", "parts": ["abcd".repeat(tokens)]},
        "metadata": {},
    });
    if let (Value::Object(message), Value::Object(extra)) = (&mut message, extra) {
        message.extend(extra);
    }
    json!({"id": id, "parent": parent, "message": message, "children": []})
}

fn conversation(id: &str, nodes: Vec<Value>) -> Value {
    let mapping: serde_json::Map<String, Value> = nodes
        .into_iter()
        .map(|n| (n["id"].as_str().unwrap().to_owned(), n))
        .collect();
    json!({
        "id": id,
        "conversation_id": id,
        "title": "never read",
        "create_time": 1_762_500_000.0,
        "default_model_slug": "gpt-4o",
        "mapping": mapping,
    })
}

/// A user question, a reply, a follow-up and two answers to it (a regeneration).
fn chat() -> Value {
    conversation(
        "c1",
        vec![
            json!({"id": "root", "parent": null, "message": null, "children": ["u1"]}),
            node("u1", Some("root"), "user", 100, json!({})),
            node(
                "a1",
                Some("u1"),
                "assistant",
                40,
                json!({"metadata": {"model_slug": "gpt-5"}}),
            ),
            node("u2", Some("a1"), "user", 10, json!({})),
            node("a2", Some("u2"), "assistant", 7, json!({})),
            node(
                "a3",
                Some("u2"),
                "assistant",
                9,
                json!({"create_time": null}),
            ),
        ],
    )
}

fn run(bytes: Vec<u8>) -> (Result<Option<ImportCounts>, ImportError>, Vec<RawUsage>) {
    let mut out = Vec::new();
    let result = ChatGpt::new().import(&mut Cursor::new(bytes), &mut |u| {
        out.push(u);
        Ok(())
    });
    (result, out)
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn key(usage: &RawUsage) -> &str {
    usage.natural_key.rsplit(':').next().unwrap()
}

#[test]
fn each_reply_counts_its_own_text_as_output_and_everything_above_as_input() {
    let (result, usage) = run(json!([chat()]).to_string().into_bytes());
    assert_eq!(
        result.unwrap(),
        Some(ImportCounts {
            conversations: 1,
            skipped: 0,
            replies: 3,
        })
    );
    let find = |id: &str| usage.iter().find(|u| key(u) == id).unwrap();

    let a1 = find("a1");
    assert_eq!(a1.natural_key, "chatgpt:c1:a1");
    assert_eq!(a1.session_key.as_deref(), Some("chatgpt:c1"));
    assert_eq!(a1.model.as_deref(), Some("gpt-5"));
    assert_eq!(a1.provider.as_deref(), Some("openai"));
    assert_eq!(a1.occurred_at, UtcMillis(1_762_600_000_250));
    assert_eq!(a1.tokens.input_uncached, Some(100));
    assert_eq!(a1.tokens.output_total, Some(40));
    assert_eq!(a1.tokens.reasoning, Some(0));
    assert_eq!(a1.requests, Some(1));
    assert_eq!(a1.accuracy, AccuracyMap::uniform(Accuracy::Estimated));

    // Both answers to the follow-up were sent the same context.
    for id in ["a2", "a3"] {
        assert_eq!(find(id).tokens.input_uncached, Some(150), "{id}");
        assert_eq!(
            find(id).model.as_deref(),
            Some("gpt-4o"),
            "the default model"
        );
    }
    assert_eq!(
        find("a3").occurred_at,
        UtcMillis(1_762_500_000_000),
        "the conversation's time"
    );
}

#[test]
fn reads_the_archive_as_the_provider_sends_it() {
    let conversations = json!([chat()]).to_string();
    let archive = zip(&[
        ("chat.html", b"<html></html>"),
        ("shared_conversations.json", b"[{\"not\": \"this\"}]"),
        ("conversations.json", conversations.as_bytes()),
    ]);
    let (result, usage) = run(archive);
    assert_eq!(result.unwrap().unwrap().replies, 3);
    assert_eq!(usage.len(), 3);
}

#[test]
fn other_files_are_not_recognised() {
    let claude = json!([{"uuid": "x", "name": "", "chat_messages": []}]).to_string();
    for bytes in [
        claude.as_bytes().to_vec(),
        b"%PDF-1.7".to_vec(),
        b"[]".to_vec(),
        Vec::new(),
        zip(&[("users.json", b"[]")]),
    ] {
        let (result, usage) = run(bytes);
        assert_eq!(result, Ok(None));
        assert!(usage.is_empty());
    }
}

#[test]
fn a_conversation_that_cannot_be_read_is_skipped() {
    let export = json!([chat(), {"id": "c2", "mapping": "damaged"}, chat()]);
    let (result, usage) = run(export.to_string().into_bytes());
    let counts = result.unwrap().unwrap();
    assert_eq!((counts.conversations, counts.skipped), (2, 1));
    assert_eq!(usage.len(), 6);
}

#[test]
fn a_truncated_export_is_reported() {
    let mut export = json!([chat(), chat()]).to_string();
    export.truncate(export.len() - 20);
    let (result, _) = run(export.into_bytes());
    assert!(matches!(result, Err(ImportError::Unreadable(_))));
}

#[test]
fn reasoning_summaries_count_as_reasoning_and_context_is_capped() {
    let long = usize::try_from(MAX_CONTEXT).unwrap();
    let export = json!([conversation(
        "c1",
        vec![
            node("u1", None, "user", long + 5, json!({})),
            node(
                "t1",
                Some("u1"),
                "assistant",
                0,
                json!({"content": {"content_type": "thoughts", "thoughts": [
                    {"summary": "abcd", "content": "abcdabcd"}
                ]}}),
            ),
        ],
    )]);
    let (_, usage) = run(export.to_string().into_bytes());
    let tokens = usage[0].tokens;
    assert_eq!(tokens.input_uncached, Some(MAX_CONTEXT));
    assert_eq!((tokens.output_total, tokens.reasoning), (Some(3), Some(3)));
}

#[test]
fn empty_replies_and_other_roles_record_nothing() {
    let export = json!([conversation(
        "c1",
        vec![
            node("s1", None, "system", 5, json!({})),
            node("u1", Some("s1"), "user", 5, json!({})),
            node("a1", Some("u1"), "assistant", 0, json!({})),
            node("t1", Some("a1"), "tool", 5, json!({})),
        ],
    )]);
    let (result, usage) = run(export.to_string().into_bytes());
    assert_eq!(result.unwrap().unwrap().replies, 0);
    assert!(usage.is_empty());
}

#[test]
fn a_storage_failure_stops_the_import() {
    let mut calls = 0;
    let result = ChatGpt::new().import(
        &mut Cursor::new(json!([chat(), chat()]).to_string().into_bytes()),
        &mut |_| {
            calls += 1;
            Err(ImportError::Storage("full".into()))
        },
    );
    assert_eq!(result, Err(ImportError::Storage("full".into())));
    assert_eq!(calls, 1);
}
