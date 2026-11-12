//! The tool rollup (ADR-034): counts per tool that follow every write, including calls
//! and outcomes that arrive after their request was stored.

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::port::{RangeQuery, ToolReader};
use veyra_analytics::tools::{ToolCounts, ToolUse};
use veyra_domain::time::UtcMillis;
use veyra_domain::tooling::{ToolCall, ToolCategory, ToolOutcome};
use veyra_store::{IngestBatch, PricedEvent, Store};

use support::{HOUR, NOV_8, TempDb, bundled_catalog, event, priced, tokens};

fn call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}

fn request(key: &str, at: i64, calls: Vec<ToolCall>, subagent: bool) -> PricedEvent {
    let mut stored = event(key, at, tokens(100, 10));
    stored.context.is_subagent = subagent;
    let mut request = priced(stored, &bundled_catalog());
    request.tool_calls = calls;
    request
}

fn commit(store: &Store, events: Vec<PricedEvent>, outcomes: &[(&str, bool)]) {
    store
        .commit(
            &IngestBatch {
                events,
                tool_outcomes: outcomes
                    .iter()
                    .map(|(id, is_error)| ToolOutcome {
                        tool_call_id: (*id).to_owned(),
                        is_error: *is_error,
                    })
                    .collect(),
                checkpoint: None,
            },
            UtcMillis(NOV_8),
        )
        .unwrap();
}

fn day(filters: Filters) -> RangeQuery {
    RangeQuery {
        from: UtcMillis(NOV_8),
        until: UtcMillis(NOV_8 + 24 * HOUR),
        filters,
    }
}

fn counts(uses: &[ToolUse], tool: &str) -> ToolCounts {
    uses.iter().find(|u| u.tool == tool).unwrap().counts
}

#[test]
fn counts_follow_late_calls_and_outcomes() {
    let db = TempDb::new();
    commit(
        &db.store,
        vec![
            request(
                "a",
                NOV_8,
                vec![call("t1", "Bash"), call("t2", "Read")],
                false,
            ),
            request(
                "b",
                NOV_8 + HOUR,
                vec![call("t3", "mcp__github__get_issue")],
                true,
            ),
        ],
        &[("t1", true)],
    );
    let uses = db.store.tool_uses(&day(Filters::new())).unwrap();
    assert_eq!(
        counts(&uses, "Bash"),
        ToolCounts {
            calls: 1,
            failed: 1,
            with_outcome: 1,
            in_subagents: 0
        }
    );
    let mcp = uses
        .iter()
        .find(|u| u.category == ToolCategory::Mcp)
        .unwrap();
    assert_eq!(mcp.mcp_server.as_deref(), Some("github"));
    assert_eq!(mcp.counts.in_subagents, 1);

    // A later batch: an outcome for a stored call, and a new call on an unchanged request.
    commit(
        &db.store,
        vec![request(
            "a",
            NOV_8,
            vec![call("t1", "Bash"), call("t2", "Read"), call("t4", "Bash")],
            false,
        )],
        &[("t2", false), ("t1", true)],
    );
    let uses = db.store.tool_uses(&day(Filters::new())).unwrap();
    assert_eq!(counts(&uses, "Bash").calls, 2);
    assert_eq!(counts(&uses, "Read").with_outcome, 1);
    assert_eq!(counts(&uses, "Read").failed, 0);

    let buckets = db.store.category_calls(&day(Filters::new())).unwrap();
    let total: u64 = buckets.iter().map(|b| b.calls).sum();
    assert_eq!(total, 4);
    assert_eq!(buckets[0].bucket_start, UtcMillis(NOV_8));
}

#[test]
fn filters_apply_to_tool_counts() {
    let db = TempDb::new();
    commit(
        &db.store,
        vec![request("a", NOV_8, vec![call("t1", "Bash")], false)],
        &[],
    );
    let elsewhere =
        Filters::new().with(Dimension::App, [Some("an-application-not-seen".to_owned())]);
    assert_eq!(db.store.tool_uses(&day(elsewhere)).unwrap(), Vec::new());
    let outside = RangeQuery {
        from: UtcMillis(NOV_8 + HOUR),
        until: UtcMillis(NOV_8 + 2 * HOUR),
        filters: Filters::new(),
    };
    assert_eq!(db.store.category_calls(&outside).unwrap(), Vec::new());
}
