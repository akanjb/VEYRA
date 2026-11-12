use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{Kpi, Scope};

/// Request for [`crate::Api::get_tools`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRequest {
    pub scope: Scope,
}

/// What a tool call does, grouped across applications' tool names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolKind {
    ReadSearch,
    Edit,
    Shell,
    Web,
    Subagent,
    Planning,
    Mcp,
    Other,
}

/// Everything the Tools & Agents page shows for a scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub kpis: ToolsKpis,
    /// The kinds called in the period, in display order.
    pub kinds: Vec<ToolKindRow>,
    /// One point per local day of the period, with calls aligned with `kinds`.
    pub daily: Vec<ToolsDay>,
    /// The most called tools, most called first.
    pub tools: Vec<ToolRow>,
    /// The MCP servers used, most called first.
    pub servers: Vec<McpServerRow>,
    /// The tools with the most failed calls, most first.
    pub failing: Vec<ToolRow>,
    pub failure_definition: String,
}

/// The Tools & Agents headline tiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolsKpis {
    pub calls: Kpi,
    pub calls_per_request: Kpi,
    pub failure_rate: Kpi,
    pub mcp_share: Kpi,
    pub subagent_share: Kpi,
}

/// Counts of tool calls. `failed` counts calls reported as errors among the
/// `with_outcome` calls whose outcome is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallCounts {
    #[ts(type = "number")]
    pub calls: u64,
    #[ts(type = "number")]
    pub failed: u64,
    #[ts(type = "number")]
    pub with_outcome: u64,
}

/// The calls of one kind in the period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolKindRow {
    pub kind: ToolKind,
    pub counts: ToolCallCounts,
}

/// One local day's calls per kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolsDay {
    pub date: String,
    /// Aligned with [`Tools::kinds`].
    #[ts(type = "number[]")]
    pub calls: Vec<u64>,
}

/// One tool's calls in the period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolRow {
    /// The name the application calls the tool by.
    pub name: String,
    /// The tool's own name: for an MCP tool, without its server.
    pub label: String,
    pub mcp_server: Option<String>,
    pub kind: ToolKind,
    pub counts: ToolCallCounts,
}

/// The calls to one MCP server's tools in the period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRow {
    pub server: String,
    /// Distinct tools of the server that were called.
    pub tools: u32,
    pub counts: ToolCallCounts,
}
