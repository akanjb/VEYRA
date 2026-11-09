//! Codex connector tests against the synthetic rollout fixtures.

use std::path::{Path, PathBuf};

use veyra_domain::accuracy::Accuracy;
use veyra_domain::connector::{Compression, Reading};
use veyra_domain::identity::Interface;
use veyra_domain::source::{DiscoveryEnv, FileContext, ParsedRecords, RawUsage, SourceConnector};
use veyra_domain::tokens::TokenCounts;
use veyra_domain::tooling::ToolOutcome;
use veyra_testkit::codex_home;

use super::Codex;

const CURRENT: &str =
    "sessions/2026/11/08/rollout-2026-11-08T10-00-00-0190aaaa-0000-7000-8000-000000000001.jsonl";
const LEGACY: &str =
    "sessions/2026/11/08/rollout-2026-11-08T11-00-00-0190aaaa-0000-7000-8000-000000000002.jsonl";
const FORK: &str =
    "sessions/2026/11/08/rollout-2026-11-08T12-00-00-0190aaaa-0000-7000-8000-000000000003.jsonl";
const SUBAGENT: &str =
    "archived_sessions/rollout-2026-11-07T09-00-00-0190aaaa-0000-7000-8000-000000000004.jsonl";

fn parse(relative: &str) -> ParsedRecords {
    let root = codex_home();
    let data = std::fs::read(root.join(relative)).unwrap();
    Codex::new().parse(
        &FileContext {
            root: &root,
            relative: Path::new(relative),
            base_offset: 0,
            side_files: &[],
        },
        &data,
    )
}

fn total(usage: &[RawUsage]) -> u64 {
    usage
        .iter()
        .map(|u| {
            let t = &u.tokens;
            [
                t.input_uncached,
                t.cache_read,
                t.cache_write_unspecified,
                t.output_total,
            ]
            .iter()
            .map(|n| n.unwrap_or(0))
            .sum::<u64>()
        })
        .sum()
}

fn tokens(input: u64, cached: u64, written: u64, output: u64, reasoning: u64) -> TokenCounts {
    TokenCounts {
        input_uncached: Some(input),
        cache_read: Some(cached),
        cache_write_unspecified: Some(written),
        output_total: Some(output),
        reasoning: Some(reasoning),
        ..TokenCounts::default()
    }
}

#[test]
fn reads_only_rollouts_and_decompresses_archives() {
    let codex = Codex::new();
    for accepted in [CURRENT, SUBAGENT, "archived_sessions/rollout-x.jsonl.zst"] {
        assert!(codex.is_source_file(Path::new(accepted)), "{accepted}");
    }
    for refused in [
        "history.jsonl",
        "auth.json",
        "sessions/2026/11/08/notes.txt",
        "sessions/2026/11/rollout-x.jsonl",
        "sessions/26/11/08/rollout-x.jsonl",
        "state_5.sqlite",
        "../sessions/2026/11/08/rollout-x.jsonl",
    ] {
        assert!(!codex.is_source_file(Path::new(refused)), "{refused}");
    }
    assert_eq!(
        codex.reading(Path::new("archived_sessions/rollout-x.jsonl.zst")),
        Reading::WholeFile(Compression::Zstd)
    );
    assert_eq!(
        codex.reading(Path::new(CURRENT)),
        Reading::WholeFile(Compression::None)
    );
}

#[test]
fn finds_the_home_from_codex_home_or_the_user_home() {
    let codex = Codex::new();
    let home = DiscoveryEnv {
        home: Some(PathBuf::from("/home/dev")),
        ..DiscoveryEnv::default()
    };
    assert_eq!(
        codex.default_roots(&home),
        [PathBuf::from("/home/dev/.codex")]
    );
    let mut moved = home;
    moved
        .location_overrides
        .insert("CODEX_HOME".to_owned(), PathBuf::from("/data/codex"));
    assert_eq!(codex.default_roots(&moved), [PathBuf::from("/data/codex")]);
}

#[test]
fn current_rollouts_count_each_response_once_from_its_record() {
    let parsed = parse(CURRENT);
    let [first, second] = parsed.usage.as_slice() else {
        panic!(
            "two responses, not the token_count totals too: {:?}",
            parsed.usage
        );
    };
    assert_eq!(first.natural_key, "codex:resp_1");
    assert_eq!(first.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(first.provider.as_deref(), Some("openai"));
    assert_eq!(first.tokens, tokens(200, 800, 0, 50, 20));
    assert_eq!(first.interface, Interface::Cli);
    assert_eq!(
        first.session_key.as_deref(),
        Some("codex:0190aaaa-0000-7000-8000-000000000001")
    );
    assert_eq!(first.cwd.as_deref(), Some("/home/dev/demo"));
    assert_eq!(first.tool_version.as_deref(), Some("0.167.0"));
    assert_eq!(first.accuracy.requests, Accuracy::Exact);
    let names: Vec<&str> = first.tool_calls.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["exec_command"]);

    // Cache writes are part of the input count: 2000 = 400 + 1500 + 100.
    assert_eq!(second.model.as_deref(), Some("gpt-5.3-codex"));
    assert_eq!(second.tokens, tokens(400, 1500, 100, 120, 60));
    assert_eq!(second.tool_calls[0].name, "apply_patch");

    assert_eq!(
        parsed.tool_outcomes,
        [
            ToolOutcome {
                tool_call_id: "call_1".into(),
                is_error: false
            },
            ToolOutcome {
                tool_call_id: "call_2".into(),
                is_error: true
            },
        ]
    );
    let kinds: Vec<&str> = parsed.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["malformed-json"]);
}

#[test]
fn older_rollouts_count_increases_that_add_up_to_the_final_total() {
    let parsed = parse(LEGACY);
    // A repeated total and a rate-limit-only update add nothing.
    assert_eq!(parsed.usage.len(), 2);
    assert_eq!(total(&parsed.usage), 3300, "the final running total");
    assert_eq!(parsed.usage[0].tokens, tokens(1000, 0, 0, 100, 0));
    assert_eq!(parsed.usage[1].tokens, tokens(1000, 1000, 0, 200, 50));
    assert_eq!(parsed.usage[0].interface, Interface::Ide);
    assert_eq!(parsed.usage[0].accuracy.requests, Accuracy::Partial);
    assert_eq!(parsed.usage[0].tool_calls[0].name, "local_shell");
    assert_eq!(
        parsed.tool_outcomes,
        [ToolOutcome {
            tool_call_id: "call_s1".into(),
            is_error: false
        }]
    );
}

#[test]
fn forks_skip_copied_totals_and_resets_start_a_new_baseline() {
    let parsed = parse(FORK);
    // Own turn: 4180 - 3300 = 880. Then the total was overwritten (a reset), and the
    // next response added 630.
    let totals: Vec<u64> = parsed
        .usage
        .iter()
        .map(|u| total(std::slice::from_ref(u)))
        .collect();
    assert_eq!(totals, [880, 630]);
}

#[test]
fn subagent_threads_link_to_their_parent_and_skip_copied_records() {
    let parsed = parse(SUBAGENT);
    let [usage] = parsed.usage.as_slice() else {
        panic!("the copied parent record is skipped: {:?}", parsed.usage);
    };
    assert_eq!(usage.natural_key, "codex:resp_s1");
    assert!(usage.is_subagent);
    assert_eq!(
        usage.parent_session_key.as_deref(),
        Some("codex:0190aaaa-0000-7000-8000-000000000001")
    );
    assert_eq!(usage.interface, Interface::Desktop);
}
