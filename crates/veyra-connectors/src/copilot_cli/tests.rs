//! GitHub Copilot CLI connector tests against the synthetic session fixtures.

use std::path::{Path, PathBuf};

use veyra_domain::event::Granularity;
use veyra_domain::source::{DiscoveryEnv, FileContext, ParsedRecords, SourceConnector};
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::ToolOutcome;
use veyra_testkit::copilot_home;

use super::CopilotCli;

const NEW: &str = "session-state/s-new/events.jsonl";
const OLD: &str = "session-state/s-old/events.jsonl";
const CRASHED: &str = "session-state/s-crash/events.jsonl";

fn parse(relative: &str) -> ParsedRecords {
    let root = copilot_home();
    let data = std::fs::read(root.join(relative)).unwrap();
    CopilotCli::new().parse(
        &FileContext {
            root: &root,
            relative: Path::new(relative),
            base_offset: 0,
            side_files: &[],
        },
        &data,
    )
}

fn tokens(input: u64, cached: u64, output: u64, reasoning: u64) -> TokenCounts {
    TokenCounts {
        input_uncached: Some(input),
        cache_read: Some(cached),
        cache_write_unspecified: Some(0),
        output_total: Some(output),
        reasoning: Some(reasoning),
        ..TokenCounts::default()
    }
}

#[test]
fn reads_only_session_event_logs() {
    let copilot = CopilotCli::new();
    for accepted in [NEW, OLD, "session-state/s-flat.jsonl"] {
        assert!(copilot.is_source_file(Path::new(accepted)), "{accepted}");
    }
    for refused in [
        "logs/process-1794150000000-42.log",
        "config.json",
        "session-store.db",
        "history-session-state/old.json",
        "session-state/s-new/workspace.yaml",
        "session-state/.jsonl",
        "../session-state/s-new/events.jsonl",
    ] {
        assert!(!copilot.is_source_file(Path::new(refused)), "{refused}");
    }
    let mut env = DiscoveryEnv {
        home: Some(PathBuf::from("/home/dev")),
        ..DiscoveryEnv::default()
    };
    assert_eq!(
        copilot.default_roots(&env),
        [PathBuf::from("/home/dev/.copilot")]
    );
    env.location_overrides
        .insert("COPILOT_HOME".to_owned(), PathBuf::from("/data/copilot"));
    assert_eq!(
        copilot.default_roots(&env),
        [PathBuf::from("/data/copilot")]
    );
}

#[test]
fn cumulative_summaries_count_only_what_each_run_added() {
    let parsed = parse(NEW);
    let [first, second] = parsed.usage.as_slice() else {
        panic!("one model per run: {:?}", parsed.usage);
    };
    // Run one: Sonnet. Run two: Sonnet unchanged (not counted again), GPT-5.5 added.
    assert_eq!(first.model.as_deref(), Some("claude-sonnet-4.5"));
    assert_eq!(first.tokens, tokens(30_000, 60_000, 3_000, 0));
    assert_eq!(first.requests, Some(5));
    assert_eq!(second.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(second.tokens, tokens(4_000, 8_000, 800, 200));
    assert_eq!(second.requests, Some(2));

    assert_eq!(first.gateway.as_deref(), Some("github-copilot"));
    assert_eq!(first.provider, None, "the vendor comes from the model");
    assert_eq!(first.granularity, Granularity::Bucket);
    assert_eq!(first.session_key.as_deref(), Some("copilot:s-new"));
    assert_eq!(first.cwd.as_deref(), Some("/home/dev/web"));
    assert_eq!(first.tool_version.as_deref(), Some("1.0.100"));
    assert_eq!(first.tool_calls[0].name, "bash");
    assert_eq!(second.tool_calls[0].name, "view");
    assert_eq!(
        parsed.tool_outcomes,
        [
            ToolOutcome {
                tool_call_id: "tc1".into(),
                is_error: false
            },
            ToolOutcome {
                tool_call_id: "tc2".into(),
                is_error: true
            },
        ]
    );
    let kinds: Vec<&str> = parsed.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["malformed-json"]);
}

#[test]
fn older_versions_count_every_summary_in_full() {
    let parsed = parse(OLD);
    let totals: Vec<_> = parsed.usage.iter().map(|u| u.tokens.total()).collect();
    // The second summary is larger, but 0.0.430 summaries each cover one run.
    assert_eq!(totals, [Some(1_100), Some(3_250)]);
    assert_eq!(parsed.usage[1].tokens, tokens(2_000, 1_000, 250, 0));
}

#[test]
fn a_session_that_never_ended_has_no_usage() {
    let parsed = parse(CRASHED);
    assert_eq!(parsed.usage, []);
    assert_eq!(parsed.issues, []);
}
