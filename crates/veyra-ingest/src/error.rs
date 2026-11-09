use std::path::PathBuf;

use veyra_store::StoreError;

/// Error raised while ingesting a source.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("access to {0} is not allowed")]
    Denied(PathBuf),
    #[error("{0} is larger than Veyra reads in one piece")]
    Oversize(PathBuf),
    #[error("cannot snapshot the database {path}: {message}")]
    Snapshot { path: PathBuf, message: String },
    #[error("source root {0} is not a directory")]
    MissingRoot(PathBuf),
    #[error("no connector named `{0}`")]
    UnknownConnector(String),
    #[error("ingestion was cancelled")]
    Cancelled,
}

impl IngestError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }

    /// The diagnostics kind of a failure confined to one file, which is counted and
    /// skipped; `None` for failures that stop the scan.
    #[must_use]
    pub const fn file_issue(&self) -> Option<&'static str> {
        match self {
            Self::Io { .. } | Self::Denied(_) => Some("unreadable-file"),
            Self::Oversize(_) => Some("oversize-file"),
            Self::Snapshot { .. } => Some("unreadable-database"),
            Self::Store(_) | Self::MissingRoot(_) | Self::UnknownConnector(_) | Self::Cancelled => {
                None
            }
        }
    }
}
