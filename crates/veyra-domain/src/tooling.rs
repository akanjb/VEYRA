//! Tool calls made by agents, as metadata only: a tool's name and whether it failed.
//! Tool inputs and outputs are never stored (PRD NFR-03).

/// One tool invocation requested by a model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCall {
    /// The source's id for this invocation, used to attach its outcome later.
    pub id: String,
    pub name: String,
}

/// The outcome of a tool invocation, reported in a later record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolOutcome {
    pub tool_call_id: String,
    pub is_error: bool,
}

/// Splits an MCP tool name of the form `mcp__<server>__<tool>` into `(server, tool)`.
#[must_use]
pub fn mcp_parts(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    (!server.is_empty() && !tool.is_empty()).then_some((server, tool))
}

#[cfg(test)]
mod tests {
    use super::mcp_parts;

    #[test]
    fn splits_mcp_tool_names() {
        assert_eq!(
            mcp_parts("mcp__github__create_issue"),
            Some(("github", "create_issue"))
        );
        assert_eq!(mcp_parts("mcp__a__b__c"), Some(("a", "b__c")));
        assert_eq!(mcp_parts("Bash"), None);
        assert_eq!(mcp_parts("mcp__server"), None);
        assert_eq!(mcp_parts("mcp____tool"), None);
    }
}
