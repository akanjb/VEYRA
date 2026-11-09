//! OpenCode connector tests on query rows. The queries themselves run against the
//! fixture database in the ingest tests.

use std::path::{Path, PathBuf};

use veyra_domain::accuracy::Accuracy;
use veyra_domain::connector::Reading;
use veyra_domain::source::{DiscoveryEnv, FileContext, ParsedRecords, SourceConnector};
use veyra_domain::tooling::ToolOutcome;

use super::OpenCode;

fn parse(rows: &[&str]) -> ParsedRecords {
    let data = rows.join("\n") + "\n";
    OpenCode::new().parse(
        &FileContext {
            root: Path::new("/data/opencode"),
            relative: Path::new("opencode.db"),
            base_offset: 0,
            side_files: &[],
        },
        data.as_bytes(),
    )
}

const OLD_V1: &str = r#"{"kind":"usage","schema":1,"id":"msg_old","session":"ses_old","directory":"/home/dev/api","version":"1.3.5","at":1794142820000,"model":"claude-sonnet-4-5","provider":"anthropic","input":100,"output":60,"reasoning":20,"cache_read":1000,"cache_write":50}"#;
const OLD_V2: &str = r#"{"kind":"usage","schema":2,"id":"msg_old","session":"ses_old","directory":"/home/dev/api","version":"1.3.5","at":1794142820000,"model":"claude-sonnet-4-5","provider":"anthropic","input":100,"output":40,"reasoning":20,"cache_read":1000,"cache_write":50}"#;
const COPILOT: &str = r#"{"kind":"usage","schema":1,"id":"msg_copilot","session":"ses_copilot","directory":"/home/dev/api","version":"1.5.0","at":1794143120000,"model":"claude-sonnet-4.5","provider":"github-copilot","input":200,"output":30,"reasoning":10,"cache_read":0,"cache_write":0}"#;
const SUBAGENT: &str = r#"{"kind":"usage","schema":2,"id":"msg_sub","session":"ses_sub","parent":"ses_new","directory":"C:/work/app","version":"2.2.5","at":1794146480000,"model":"gpt-5.5","provider":"openai","input":150,"output":20,"reasoning":0,"cache_read":0,"cache_write":0}"#;
const TOOL_OK: &str =
    r#"{"kind":"tool","message":"msg_old","call":"call_a","name":"bash","status":"completed"}"#;
const TOOL_FAILED: &str =
    r#"{"kind":"tool","message":"msg_sub","call":"call_c","name":"grep","status":"error"}"#;

#[test]
fn reads_only_the_database_through_a_snapshot() {
    let opencode = OpenCode::new();
    for accepted in ["opencode.db", "opencode-beta.db"] {
        assert!(opencode.is_source_file(Path::new(accepted)), "{accepted}");
    }
    for refused in [
        "opencode-next.db",
        "opencode.db-wal",
        "storage/session/p/s.json",
        "log/opencode.db",
        "auth.json",
    ] {
        assert!(!opencode.is_source_file(Path::new(refused)), "{refused}");
    }
    assert!(matches!(
        opencode.reading(Path::new("opencode.db")),
        Reading::Database(queries) if queries.len() == 5
    ));
}

#[test]
fn finds_the_data_directory_on_every_system() {
    let opencode = OpenCode::new();
    let mut env = DiscoveryEnv {
        home: Some(PathBuf::from("/home/dev")),
        ..DiscoveryEnv::default()
    };
    assert_eq!(
        opencode.default_roots(&env),
        [PathBuf::from("/home/dev/.local/share/opencode")]
    );
    env.location_overrides
        .insert("XDG_DATA_HOME".to_owned(), PathBuf::from("/xdg"));
    assert_eq!(
        opencode.default_roots(&env),
        [PathBuf::from("/xdg/opencode")]
    );
    let db = if cfg!(windows) {
        "D:\\oc\\custom.db"
    } else {
        "/oc/custom.db"
    };
    env.location_overrides
        .insert("OPENCODE_DB".to_owned(), PathBuf::from(db));
    assert_eq!(
        opencode.default_roots(&env),
        [PathBuf::from(db).parent().unwrap().to_path_buf()]
    );
}

#[test]
fn an_imported_message_counts_once_with_reasoning_inside_output_either_way() {
    // 1.3.5 counted reasoning within output (60); the 2.0 copy apart from it (40 + 20).
    let parsed = parse(&[OLD_V1, OLD_V2, TOOL_OK]);
    let [usage] = parsed.usage.as_slice() else {
        panic!("{:?}", parsed.usage)
    };
    assert_eq!(usage.natural_key, "opencode:msg_old");
    assert_eq!(usage.precedence, 2, "the newer copy");
    assert_eq!(usage.tokens.output_total, Some(60));
    assert_eq!(usage.tokens.reasoning, Some(20));
    assert_eq!(usage.tokens.input_uncached, Some(100));
    assert_eq!(usage.tokens.cache_read, Some(1000));
    assert_eq!(usage.tokens.cache_write_unspecified, Some(50));
    assert_eq!(usage.tool_calls[0].name, "bash");
    assert_eq!(usage.session_key.as_deref(), Some("opencode:ses_old"));
    assert_eq!(usage.cwd.as_deref(), Some("/home/dev/api"));
    assert_eq!(
        parsed.tool_outcomes,
        [ToolOutcome {
            tool_call_id: "call_a".into(),
            is_error: false
        }]
    );
}

#[test]
fn requests_through_github_copilot_name_it_as_the_gateway() {
    let parsed = parse(&[COPILOT]);
    let usage = &parsed.usage[0];
    assert_eq!(usage.gateway.as_deref(), Some("github-copilot"));
    assert_eq!(usage.provider, None, "the vendor comes from the model");
    assert_eq!(usage.model.as_deref(), Some("claude-sonnet-4.5"));
    // 1.5.0 counts reasoning apart from output.
    assert_eq!(usage.tokens.output_total, Some(40));
}

#[test]
fn subagent_sessions_link_to_their_parent() {
    let parsed = parse(&[SUBAGENT, TOOL_FAILED]);
    let usage = &parsed.usage[0];
    assert!(usage.is_subagent);
    assert_eq!(
        usage.parent_session_key.as_deref(),
        Some("opencode:ses_new")
    );
    assert_eq!(usage.tool_calls[0].name, "grep");
    assert!(parsed.tool_outcomes[0].is_error);
}

#[test]
fn early_versions_raw_values_are_partial_and_bad_rows_are_counted() {
    let early = OLD_V1.replace("1.3.5", "1.0.50");
    let parsed = parse(&[&early, r#"{"kind":"usage","id":"no-session"}"#, "not json"]);
    assert_eq!(parsed.usage[0].accuracy.tokens, Accuracy::Partial);
    let kinds: Vec<&str> = parsed.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["invalid-record", "invalid-record"]);
}
