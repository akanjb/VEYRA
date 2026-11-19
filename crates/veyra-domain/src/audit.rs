//! The audit log: what the user did that takes data out of Veyra or changes it.
//!
//! An entry's detail describes the action without personal data: never a path, a
//! project name, a file name or any usage content.

use crate::source::PortError;
use crate::time::UtcMillis;

/// Who performed an audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditActor {
    /// The person using Veyra.
    User,
    /// Veyra itself, for example a scheduled task.
    System,
}

/// What was done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    /// A report was written to a file the user chose.
    Export,
    /// A backup was set to replace the database at the next start.
    Restore,
    /// Stored usage was deleted from Veyra.
    Delete,
}

impl AuditAction {
    /// The stable name stored for the action.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Restore => "restore",
            Self::Delete => "delete",
        }
    }

    /// The action with this stored name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "export" => Some(Self::Export),
            "restore" => Some(Self::Restore),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

/// One audited action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub at: UtcMillis,
    pub actor: AuditActor,
    pub action: AuditAction,
    /// A description without personal data, such as "per-project report, CSV, 12 rows".
    pub detail: String,
}

/// Where audited actions are recorded.
pub trait AuditLog: Send + Sync {
    /// Appends an entry.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn record(&self, entry: &AuditEntry) -> Result<(), PortError>;

    /// The most recent entries, newest first.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn audit_entries(&self, limit: u32) -> Result<Vec<AuditEntry>, PortError>;
}
