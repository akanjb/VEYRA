//! Gemini CLI connector tests against the synthetic recording fixtures.

use std::path::{Path, PathBuf};

use veyra_domain::connector::SideFile;
use veyra_domain::source::{DiscoveryEnv, FileContext, ParsedRecords, RawUsage, SourceConnector};
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::ToolOutcome;
use veyra_testkit::gemini_tmp;

use super::GeminiCli;

const CURRENT: &str = "shop/chats/session-2026-11-08T09-00-abcdef12.jsonl";
const OLD_DOCUMENT: &str = "shop/chats/session-2026-11-01T08-00-11112222.json";
const RESUMED: &str = "shop/chats/session-2026-11-01T08-00-11112222.jsonl";
const SUBAGENT: &str = "shop/chats/abcdef12-0000-4000-8000-000000000001/agent-7.jsonl";
const MAIN_SESSION: &str = "gemini:abcdef12-0000-4000-8000-000000000001";

/// Parses a fixture with the side files the pipeline would pass.
fn parse(relative: &str) -> ParsedRecords {
    let root = gemini_tmp();
    let connector = GeminiCli::new();
    let side_files: Vec<SideFile> = connector
        .side_files(Path::new(relative))
        .into_iter()
        .filter_map(|path| {
            let bytes = std::fs::read(root.join(&path)).ok()?;
            Some(SideFile {
                relative: path,
                bytes,
            })
        })
        .collect();
    let data = std::fs::read(root.join(relative)).unwrap();
    connector.parse(
        &FileContext {
            root: &root,
            relative: Path::new(relative),
            base_offset: 0,
            side_files: &side_files,
        },
        &data,
    )
}

fn keys(usage: &[RawUsage]) -> Vec<&str> {
    usage.iter().map(|u| u.natural_key.as_str()).collect()
}

#[test]
fn reads_only_recordings() {
    let gemini = GeminiCli::new();
    for accepted in [CURRENT, OLD_DOCUMENT, RESUMED, SUBAGENT] {
        assert!(gemini.is_source_file(Path::new(accepted)), "{accepted}");
    }
    for refused in [
        "shop/logs.json",
        "shop/checkpoint-saved.json",
        "shop/.project_root",
        "shop/chats/session-2026-11-08T09-00-abcdef12.jsonl.tmp-123",
        "shop/chats/session-2026-11-01T08-00-11112222.json.unreadable-1793527200",
        "shop/chats/notes.jsonl",
        "../shop/chats/session-x.jsonl",
    ] {
        assert!(!gemini.is_source_file(Path::new(refused)), "{refused}");
    }
    assert_eq!(
        gemini.side_files(Path::new(SUBAGENT)),
        [PathBuf::from("shop/.project_root")]
    );
}

#[test]
fn finds_the_recordings_from_the_home_or_gemini_cli_home() {
    let gemini = GeminiCli::new();
    let mut env = DiscoveryEnv {
        home: Some(PathBuf::from("/home/dev")),
        ..DiscoveryEnv::default()
    };
    assert_eq!(
        gemini.default_roots(&env)[0],
        PathBuf::from("/home/dev/.gemini/tmp")
    );
    env.location_overrides
        .insert("GEMINI_CLI_HOME".to_owned(), PathBuf::from("/data/g"));
    assert_eq!(
        gemini.default_roots(&env)[0],
        PathBuf::from("/data/g/.gemini/tmp")
    );
}

#[test]
fn counts_the_last_copy_of_each_response_once() {
    let parsed = parse(CURRENT);
    assert_eq!(
        keys(&parsed.usage),
        [
            format!("{MAIN_SESSION}:g1"),
            format!("{MAIN_SESSION}:g2"),
            format!("{MAIN_SESSION}:g3"),
        ]
    );
    let [g1, g2, g3] = parsed.usage.as_slice() else {
        unreachable!()
    };
    // input 1200 includes 800 cached; 10 tool-use prompt tokens are billed as input;
    // 150 thoughts are reasoning on top of 40 output.
    assert_eq!(
        g1.tokens,
        TokenCounts {
            input_uncached: Some(410),
            cache_read: Some(800),
            output_total: Some(190),
            reasoning: Some(150),
            tool_tokens: Some(10),
            ..TokenCounts::default()
        }
    );
    assert_eq!(g1.tokens.total(), Some(1400), "Gemini's own total");
    assert_eq!(g1.model.as_deref(), Some("gemini-2.5-pro"));
    assert_eq!(g1.tool_calls[0].name, "read_file");
    assert_eq!(g1.cwd.as_deref(), Some("/home/dev/shop"));
    assert_eq!(g1.session_key.as_deref(), Some(MAIN_SESSION));
    assert_eq!(g2.model.as_deref(), Some("gemini-2.5-pro"), "nearest model");
    assert_eq!(g2.tokens.total(), Some(2100));
    assert_eq!(g3.model.as_deref(), Some("gemini-2.5-flash"));
    assert_eq!(
        parsed.tool_outcomes,
        [
            ToolOutcome {
                tool_call_id: "c1".into(),
                is_error: false
            },
            ToolOutcome {
                tool_call_id: "c2".into(),
                is_error: true
            },
        ]
    );
    let kinds: Vec<&str> = parsed.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["malformed-json"]);
}

#[test]
fn older_documents_and_their_resumed_copies_share_message_keys() {
    let old = parse(OLD_DOCUMENT);
    let resumed = parse(RESUMED);
    let session = "gemini:11112222-0000-4000-8000-000000000002";
    assert_eq!(keys(&old.usage), [format!("{session}:h1")]);
    assert_eq!(
        keys(&resumed.usage),
        [format!("{session}:h1"), format!("{session}:h2")]
    );
    assert_eq!(old.usage[0].tokens, resumed.usage[0].tokens);
    assert_eq!(resumed.usage[1].tokens.cache_read, Some(200));
}

#[test]
fn subagent_sessions_link_to_their_parent() {
    let parsed = parse(SUBAGENT);
    let [usage] = parsed.usage.as_slice() else {
        panic!("{:?}", parsed.usage)
    };
    assert!(usage.is_subagent);
    assert_eq!(usage.parent_session_key.as_deref(), Some(MAIN_SESSION));
    assert_eq!(
        usage.session_key.as_deref(),
        Some("gemini:77770000-0000-4000-8000-000000000007")
    );
    assert_eq!(usage.cwd.as_deref(), Some("/home/dev/shop"));
}
