//! Claude Code connector tests against the synthetic v2.1 fixtures.

use std::path::{Path, PathBuf};

use veyra_domain::accuracy::Accuracy;
use veyra_domain::event::ServiceTier;
use veyra_domain::identity::Interface;
use veyra_domain::source::{DiscoveryEnv, FileContext, ParsedRecords, RawUsage, SourceConnector};
use veyra_domain::time::UtcMillis;
use veyra_domain::tooling::ToolOutcome;

use super::ClaudeCode;
use super::convert::STOP_BIT;

const SESSION: &str = "11111111-1111-4111-8111-111111111111";

fn projects() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude-code/v2.1/projects")
}

fn parse(relative: &str) -> ParsedRecords {
    let root = projects();
    let data = std::fs::read(root.join(relative)).unwrap();
    ClaudeCode::new().parse(
        &FileContext {
            root: &root,
            relative: Path::new(relative),
            base_offset: 0,
        },
        &data,
    )
}

fn main_session() -> ParsedRecords {
    parse(&format!("C--work-demo/{SESSION}.jsonl"))
}

fn record<'a>(records: &'a ParsedRecords, request: &str) -> &'a RawUsage {
    records
        .usage
        .iter()
        .find(|r| r.natural_key == format!("anthropic:{request}"))
        .unwrap_or_else(|| panic!("no record for {request}"))
}

/// 2026-11-08T10:00:00Z plus seconds.
fn at(seconds: i64) -> UtcMillis {
    UtcMillis(1_794_132_000_000 + seconds * 1_000)
}

#[test]
fn keeps_one_record_per_request_and_skips_synthetic_lines() {
    let parsed = main_session();
    let keys: Vec<_> = parsed
        .usage
        .iter()
        .map(|r| r.natural_key.as_str())
        .collect();
    assert_eq!(
        keys,
        ["anthropic:req_A", "anthropic:req_B", "anthropic:req_C"]
    );
    assert_eq!(parsed.lines, 11);
}

#[test]
fn the_completed_line_wins_over_streaming_partials() {
    let parsed = main_session();
    let a = record(&parsed, "req_A");
    assert_eq!(
        a.tokens.output_total,
        Some(120),
        "final line, not the 5-token partial"
    );
    assert_eq!(a.tokens.reasoning, Some(40));
    assert_eq!(a.tokens.input_uncached, Some(4));
    assert_eq!(a.tokens.cache_read, Some(10_000));
    assert_eq!(a.tokens.cache_write_short, Some(2_000));
    assert_eq!(a.tokens.cache_write_long, Some(0));
    assert_eq!(a.occurred_at, at(3));
    assert_eq!(a.precedence, STOP_BIT + 12_124);
}

#[test]
fn a_superseded_anomalous_copy_never_wins() {
    // req_B has two completed copies; the first reports thinking > output and a cache
    // split that does not add up. The larger, consistent copy is kept.
    let b = record(&main_session(), "req_B").clone();
    assert_eq!(b.tokens.output_total, Some(80));
    assert_eq!(b.tokens.reasoning, Some(30));
    assert_eq!(
        (b.tokens.cache_write_short, b.tokens.cache_write_long),
        (Some(200), Some(100))
    );
    assert_eq!(b.accuracy.reasoning, Accuracy::Exact);
}

#[test]
fn inconsistent_values_are_left_unreported_not_invented() {
    // req_C's cache split (100 + 100) does not equal its total (500).
    let c = record(&main_session(), "req_C").clone();
    assert_eq!(c.tokens.cache_write_unspecified, Some(500));
    assert_eq!(
        (c.tokens.cache_write_short, c.tokens.cache_write_long),
        (None, None)
    );
    assert_eq!(c.service_tier, ServiceTier::Fast);
    assert!(c.inference_geo_us);
}

#[test]
fn attributes_session_project_interface_and_version() {
    let a = record(&main_session(), "req_A").clone();
    assert_eq!(
        a.session_key.as_deref(),
        Some(format!("claude-code:{SESSION}").as_str())
    );
    assert_eq!(a.parent_session_key, None);
    assert!(!a.is_subagent);
    assert_eq!(a.cwd.as_deref(), Some("C:\\work\\demo"));
    assert_eq!(a.interface, Interface::Cli);
    assert_eq!(a.tool_version.as_deref(), Some("2.1.300"));
    assert_eq!(a.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(a.accuracy.cost, Accuracy::Estimated);
}

#[test]
fn collects_tool_calls_and_outcomes_without_content() {
    let parsed = main_session();
    let names: Vec<_> = parsed
        .usage
        .iter()
        .flat_map(|r| r.tool_calls.iter().map(|c| c.name.as_str()))
        .collect();
    assert_eq!(names, ["Bash", "mcp__github__create_issue"]);
    assert_eq!(
        parsed.tool_outcomes,
        [
            ToolOutcome {
                tool_call_id: "toolu_A1".to_owned(),
                is_error: true
            },
            ToolOutcome {
                tool_call_id: "toolu_C1".to_owned(),
                is_error: false
            },
        ]
    );
}

#[test]
fn reports_malformed_lines_with_their_offset() {
    let parsed = main_session();
    assert_eq!(parsed.issues.len(), 1);
    assert_eq!(parsed.issues[0].kind, "malformed-json");
    let data = std::fs::read(projects().join(format!("C--work-demo/{SESSION}.jsonl"))).unwrap();
    let offset = usize::try_from(parsed.issues[0].offset).unwrap();
    assert!(data[offset..].starts_with(b"{\"type\":\"assistant\",\"sessionId\""));
    assert!(
        std::str::from_utf8(&data[offset..offset + 200])
            .unwrap()
            .contains("req_BROKEN")
    );
}

#[test]
fn subagent_files_link_to_their_parent_session() {
    let parsed = parse(&format!("C--work-demo/{SESSION}/subagents/agent-a1.jsonl"));
    let d = record(&parsed, "req_D");
    assert!(d.is_subagent);
    assert_eq!(
        d.session_key.as_deref(),
        Some(format!("claude-code:{SESSION}:a1").as_str())
    );
    assert_eq!(
        d.parent_session_key.as_deref(),
        Some(format!("claude-code:{SESSION}").as_str())
    );
    assert_eq!(d.model.as_deref(), Some("claude-sonnet-5-5"));
}

#[test]
fn no_content_ever_leaves_the_parser() {
    for file in [
        format!("C--work-demo/{SESSION}.jsonl"),
        format!("C--work-demo/{SESSION}/subagents/agent-a1.jsonl"),
    ] {
        let rendered = format!("{:?}", parse(&file));
        assert!(
            !rendered.contains("VEYRA-CANARY"),
            "content leaked from {file}"
        );
    }
}

#[test]
fn reads_only_session_and_subagent_logs() {
    let connector = ClaudeCode::new();
    for (path, expected) in [
        ("project/session.jsonl", true),
        ("project/session/subagents/agent-1.jsonl", true),
        ("project/session/tool-results/out.txt", false),
        ("project/session/tool-results/result.jsonl", false),
        ("project/memory/notes.jsonl", false),
        ("project/memory/MEMORY.md", false),
        ("session.jsonl", false),
        ("project/.jsonl", false),
        ("project/../other/session.jsonl", false),
    ] {
        assert_eq!(
            connector.is_source_file(Path::new(path)),
            expected,
            "{path}"
        );
    }
}

#[test]
fn finds_the_default_root_or_the_documented_override() {
    let connector = ClaudeCode::new();
    let mut env = DiscoveryEnv {
        home: Some(PathBuf::from("/home/dev")),
        ..DiscoveryEnv::default()
    };
    assert_eq!(
        connector.default_roots(&env),
        [PathBuf::from("/home/dev/.claude/projects")]
    );
    env.location_overrides.insert(
        "CLAUDE_CONFIG_DIR".to_owned(),
        PathBuf::from("/data/claude"),
    );
    assert_eq!(
        connector.default_roots(&env),
        [PathBuf::from("/data/claude/projects")]
    );
}

#[test]
fn chunked_reads_report_absolute_offsets() {
    let root = projects();
    let relative = format!("C--work-demo/{SESSION}.jsonl");
    let data = std::fs::read(root.join(&relative)).unwrap();
    let split = data
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .nth(6)
        .unwrap()
        .0
        + 1;
    let second = ClaudeCode::new().parse(
        &FileContext {
            root: &root,
            relative: Path::new(&relative),
            base_offset: split as u64,
        },
        &data[split..],
    );
    assert_eq!(second.issues.len(), 1);
    assert_eq!(second.issues[0].offset, main_session().issues[0].offset);
}
