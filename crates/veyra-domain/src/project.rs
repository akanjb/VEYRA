//! Projects as the user names and groups them (ADR-027).
//!
//! A project is first identified by its normalised root path. Users can give it a
//! display name, and merge projects that are really one (a moved or renamed folder, a
//! second checkout). A merged project keeps its own identity, so merging can always be
//! undone exactly.

use crate::source::PortError;

/// A stored project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: i64,
    /// Normalised project root path.
    pub path: String,
    /// The name the user gave it, if any.
    pub display_name: Option<String>,
    /// The project this one is merged into, if any. Merges are flattened, so this is
    /// always a project that is not itself merged.
    pub merged_into: Option<i64>,
}

/// Why a project change was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectChangeError {
    #[error("project {0} does not exist")]
    NotFound(i64),
    #[error("a project cannot be merged into itself")]
    SelfMerge,
    #[error("project {0} is not merged into another project")]
    NotMerged(i64),
}

/// The longest display name a project may have, in characters.
pub const MAX_PROJECT_NAME_CHARS: usize = 80;

/// A display name as stored: trimmed, `None` when empty. Longer names are refused by the
/// caller before they reach storage.
#[must_use]
pub fn normalise_project_name(name: Option<&str>) -> Option<String> {
    name.map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
}

/// Reads and changes projects (implemented by the data layer).
pub trait ProjectRepository: Send + Sync {
    /// Every project, merged or not.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn list_projects(&self) -> Result<Vec<ProjectRecord>, PortError>;

    /// Sets or clears a project's display name.
    ///
    /// # Errors
    /// [`ProjectChangeError::NotFound`] inside [`PortError`] for an unknown project, or a
    /// persistence failure.
    fn rename_project(&self, id: i64, name: Option<&str>) -> Result<ProjectRecord, PortError>;

    /// Merges `from` (and every project already merged into it) into `into`, so their
    /// usage counts as `into`'s from then on, including what was recorded before.
    ///
    /// # Errors
    /// [`ProjectChangeError`] inside [`PortError`] for unknown projects or a merge into
    /// itself, or a persistence failure.
    fn merge_project(&self, from: i64, into: i64) -> Result<(), PortError>;

    /// Undoes a merge: the project's usage counts as its own again.
    ///
    /// # Errors
    /// [`ProjectChangeError`] inside [`PortError`] for an unknown or unmerged project, or a
    /// persistence failure.
    fn unmerge_project(&self, id: i64) -> Result<(), PortError>;
}

#[cfg(test)]
mod tests {
    use super::normalise_project_name;

    #[test]
    fn names_are_trimmed_and_empty_ones_cleared() {
        assert_eq!(
            normalise_project_name(Some("  Veyra ")),
            Some("Veyra".to_owned())
        );
        assert_eq!(normalise_project_name(Some("   ")), None);
        assert_eq!(normalise_project_name(None), None);
    }
}
