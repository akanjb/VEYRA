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

impl ApiError {
    /// An internal error. The details stay in the core's diagnostic output under a fresh
    /// correlation id; only the id crosses the IPC boundary.
    pub(crate) fn internal(details: &dyn std::fmt::Display) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let correlation_id = format!("{started:x}-{}", NEXT.fetch_add(1, Ordering::Relaxed));
        eprintln!("veyra: internal error {correlation_id}: {details}");
        Self::Internal { correlation_id }
    }
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
