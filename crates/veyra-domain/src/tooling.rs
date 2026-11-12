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

/// What a tool call does, from the tool's name. Each application names its tools
/// differently; this groups them so they can be compared across applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ToolCategory {
    /// Reading files and searching the project.
    ReadSearch,
    /// Creating and editing files.
    Edit,
    /// Running commands.
    Shell,
    /// Fetching pages and searching the web.
    Web,
    /// Handing work to a subagent.
    Subagent,
    /// Keeping a task list or plan.
    Planning,
    /// A tool provided by an MCP server.
    Mcp,
    Other,
}

/// Built-in tool names per category, lower case, across the supported applications.
const CATEGORY_NAMES: [(ToolCategory, &[&str]); 6] = [
    (
        ToolCategory::ReadSearch,
        &[
            "read",
            "glob",
            "grep",
            "ls",
            "read_file",
            "read_many_files",
            "list_directory",
            "search_file_content",
            "view",
            "notebookread",
        ],
    ),
    (
        ToolCategory::Edit,
        &[
            "edit",
            "multiedit",
            "write",
            "notebookedit",
            "apply_patch",
            "write_file",
            "replace",
            "str_replace",
        ],
    ),
    (
        ToolCategory::Shell,
        &[
            "bash",
            "bashoutput",
            "killshell",
            "shell",
            "exec_command",
            "write_stdin",
            "local_shell",
            "run_shell_command",
        ],
    ),
    (
        ToolCategory::Web,
        &[
            "webfetch",
            "websearch",
            "web_fetch",
            "web_search",
            "google_web_search",
        ],
    ),
    (ToolCategory::Subagent, &["task", "agent"]),
    (
        ToolCategory::Planning,
        &[
            "todowrite",
            "todoread",
            "update_plan",
            "exitplanmode",
            "save_memory",
        ],
    ),
];

impl ToolCategory {
    /// Every category, in the order pages show them.
    pub const ALL: [Self; 8] = [
        Self::ReadSearch,
        Self::Edit,
        Self::Shell,
        Self::Web,
        Self::Subagent,
        Self::Planning,
        Self::Mcp,
        Self::Other,
    ];

    /// The category of the tool called `name`.
    #[must_use]
    pub fn of(name: &str) -> Self {
        if mcp_parts(name).is_some() {
            return Self::Mcp;
        }
        let lower = name.to_ascii_lowercase();
        CATEGORY_NAMES
            .iter()
            .find(|(_, names)| names.contains(&lower.as_str()))
            .map_or(Self::Other, |(category, _)| *category)
    }

    /// The stable name stored and sent to the dashboard.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReadSearch => "read_search",
            Self::Edit => "edit",
            Self::Shell => "shell",
            Self::Web => "web",
            Self::Subagent => "subagent",
            Self::Planning => "planning",
            Self::Mcp => "mcp",
            Self::Other => "other",
        }
    }

    /// The category with the stable name `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolCategory, mcp_parts};

    #[test]
    fn groups_tools_across_applications() {
        assert_eq!(ToolCategory::of("Read"), ToolCategory::ReadSearch);
        assert_eq!(ToolCategory::of("read_file"), ToolCategory::ReadSearch);
        assert_eq!(ToolCategory::of("apply_patch"), ToolCategory::Edit);
        assert_eq!(ToolCategory::of("Bash"), ToolCategory::Shell);
        assert_eq!(ToolCategory::of("exec_command"), ToolCategory::Shell);
        assert_eq!(ToolCategory::of("WebFetch"), ToolCategory::Web);
        assert_eq!(ToolCategory::of("Task"), ToolCategory::Subagent);
        assert_eq!(ToolCategory::of("TodoWrite"), ToolCategory::Planning);
        assert_eq!(
            ToolCategory::of("mcp__github__get_issue"),
            ToolCategory::Mcp
        );
        assert_eq!(ToolCategory::of("something_new"), ToolCategory::Other);
        for category in ToolCategory::ALL {
            assert_eq!(ToolCategory::from_name(category.name()), Some(category));
        }
    }

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
