//! The closed set of errors a use case may return across the IPC boundary.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An error returned by an [`crate::Api`] use case.
///
/// Serialised as a tagged union (`{ "kind": "...", ... }`) so the dashboard can branch
/// on `kind` exhaustively. Internal details are logged under the correlation id and
/// never sent to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApiError {
    /// The request was well-formed but violated a business rule.
    #[error("validation failed: {message}")]
    Validation { message: String },
    /// The requested entity does not exist.
    #[error("not found: {message}")]
    NotFound { message: String },
    /// A usage source could not be read.
    #[error("source unavailable: {message}")]
    SourceUnavailable { message: String },
    /// The operation conflicts with work already in progress.
    #[error("busy: {message}")]
    Busy { message: String },
    /// An unexpected failure; details are in the log under `correlation_id`.
    #[error("internal error (correlation id {correlation_id})")]
    Internal {
        #[serde(rename = "correlationId")]
        correlation_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::ApiError;

    #[test]
    fn serialises_as_tagged_union_with_camel_case_fields() {
        let error = ApiError::Internal {
            correlation_id: "abc".to_owned(),
        };
        let json = serde_json::to_value(&error).unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "internal", "correlationId": "abc" })
        );
    }
}
