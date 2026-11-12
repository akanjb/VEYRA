//! Contract tests of the Tools & Agents page (ADR-034), on the ingested fixtures.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use veyra_app::dto::{OverviewRequest, ToolKind, ToolsRequest};

use support::{api, ingested, scope};

#[test]
fn the_page_counts_every_call_once() {
    let data = ingested();
    let api = api(&data);
    let page = api.get_tools(ToolsRequest { scope: scope() }).unwrap();

    // The fixtures call Bash, Grep and one MCP tool of the `github` server.
    assert_eq!(page.kpis.calls.value, Some(3.0));
    let by_kind: u64 = page.kinds.iter().map(|k| k.counts.calls).sum();
    let by_tool: u64 = page.tools.iter().map(|t| t.counts.calls).sum();
    let by_day: u64 = page.daily.iter().flat_map(|d| d.calls.iter()).sum();
    assert_eq!((by_kind, by_tool, by_day), (3, 3, 3));
    assert!(page.daily.iter().all(|d| d.calls.len() == page.kinds.len()));
    assert_eq!(
        page.kinds.iter().map(|k| k.kind).collect::<Vec<_>>(),
        [ToolKind::ReadSearch, ToolKind::Shell, ToolKind::Mcp],
        "kinds come in display order"
    );

    let mcp = page.tools.iter().find(|t| t.kind == ToolKind::Mcp).unwrap();
    assert_eq!(mcp.label, "create_issue");
    assert_eq!(page.servers.len(), 1);
    assert_eq!(page.servers[0].server, "github");
    assert_eq!(page.kpis.mcp_share.value, Some(1.0 / 3.0));

    // One call failed; the failing list holds exactly the tools with failures.
    assert_eq!(page.failing.len(), 1);
    assert_eq!(page.failing[0].counts.failed, 1);

    // Calls per request divides by the requests the Overview counts.
    let overview = api
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    let requests = overview.kpis.requests.value.unwrap();
    assert_eq!(page.kpis.calls_per_request.value, Some(3.0 / requests));
}
