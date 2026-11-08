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
}
