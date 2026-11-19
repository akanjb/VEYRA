//! Claude Code connector tests against the synthetic v2.1 fixtures.

use std::path::{Path, PathBuf};

use veyra_domain::accuracy::Accuracy;
use veyra_domain::event::ServiceTier;
use veyra_domain::identity::Interface;
use veyra_domain::source::{DiscoveryEnv, FileContext, ParsedRecords, RawUsage, SourceConnector};
use veyra_domain::time::UtcMillis;
use veyra_domain::tooling::ToolOutcome;
use veyra_testkit::claude_code_projects as projects;

use super::ClaudeCode;
use super::convert::STOP_BIT;

const SESSION: &str = "11111111-1111-4111-8111-111111111111";

fn parse(relative: &str) -> ParsedRecords {
    let root = projects();
    let data = std::fs::read(root.join(relative)).unwrap();
    ClaudeCode::new().parse(
        &FileContext {
            root: &root,
            relative: Path::new(relative),
            base_offset: 0,
            side_files: &[],
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
            side_files: &[],
        },
        &data[split..],
    );
    assert_eq!(second.issues.len(), 1);
    assert_eq!(second.issues[0].offset, main_session().issues[0].offset);
}

/// A documented `api_request` OpenTelemetry record (ADR-038), with `extra` attributes.
fn api_request(extra: &[(&str, &str)]) -> veyra_domain::telemetry::TelemetryRecord {
    let mut attributes: std::collections::HashMap<String, String> = [
        ("event.name", "api_request"),
        ("event.timestamp", "2026-11-08T10:00:03.000Z"),
        ("request_id", "req_A"),
        ("session.id", SESSION),
        ("model", "claude-opus-5-5"),
        ("input_tokens", "4"),
        ("output_tokens", "120"),
        ("cache_read_tokens", "10000"),
        ("cache_creation_tokens", "2000"),
        ("cache_creation_5m_tokens", "2000"),
        ("cache_creation_1h_tokens", "0"),
        ("cost_usd", "0.5"),
        ("prompt", "never read"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    attributes.extend(
        extra
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
    );
    veyra_domain::telemetry::TelemetryRecord {
        service: "claude-code".to_owned(),
        service_version: Some("2.1.400".to_owned()),
        time: None,
        attributes,
    }
}

#[test]
fn telemetry_gives_the_same_request_key_and_tokens_as_the_log() {
    let connector = ClaudeCode::new();
    assert_eq!(connector.telemetry_service(), Some("claude-code"));
    let pushed = connector.telemetry_usage(&api_request(&[])).unwrap();
    let logged = record(&main_session(), "req_A").clone();
    assert_eq!(pushed.natural_key, logged.natural_key);
    assert_eq!(pushed.tokens.input_uncached, logged.tokens.input_uncached);
    assert_eq!(pushed.tokens.cache_read, logged.tokens.cache_read);
    assert_eq!(pushed.tokens.cache_write_short, Some(2_000));
    assert_eq!(pushed.tokens.cache_write_long, Some(0));
    assert_eq!(pushed.tokens.output_total, logged.tokens.output_total);
    assert_eq!(pushed.session_key, logged.session_key);
    assert_eq!(pushed.occurred_at, at(3));
    assert_eq!(pushed.tool_version.as_deref(), Some("2.1.400"));
    // Claude Code's own estimate is not a charge, and nothing names a project.
    assert_eq!((pushed.reported_cost, pushed.cwd), (None, None));
    // The completed log line, which knows the project, always supersedes it.
    assert!(pushed.precedence < logged.precedence);
}

#[test]
fn telemetry_reads_only_api_requests_with_a_request_id() {
    let connector = ClaudeCode::new();
    let other = api_request(&[("event.name", "user_prompt")]);
    assert!(connector.telemetry_usage(&other).is_none());
    let mut anonymous = api_request(&[]);
    anonymous.attributes.remove("request_id");
    assert!(connector.telemetry_usage(&anonymous).is_none());
}

#[test]
fn telemetry_maps_fast_mode_and_subagents() {
    let usage = ClaudeCode::new()
        .telemetry_usage(&api_request(&[("speed", "fast"), ("agent_id", "a1")]))
        .unwrap();
    assert_eq!(usage.service_tier, ServiceTier::Fast);
    assert!(usage.is_subagent);
    assert_eq!(
        usage.session_key.as_deref(),
        Some(format!("claude-code:{SESSION}:a1").as_str())
    );
    assert_eq!(
        usage.parent_session_key.as_deref(),
        Some(format!("claude-code:{SESSION}").as_str())
    );
}
