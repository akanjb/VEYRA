//! Contract tests of importing a chat app's data export (ADR-040).

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use serde_json::json;
use veyra_app::ApiError;
use veyra_app::dto::ImportExportRequest;
use veyra_domain::audit::{AuditAction, AuditLog};

use support::{api_with_picks, ingested};

/// A ChatGPT export with one question and one reply.
fn export() -> Vec<u8> {
    let message = |id: &str, role: &str| {
        json!({
            "id": id,
            "author": {"role": role},
            "create_time": 1_762_600_000.0,
            "content": {"content_type": "text", "parts": ["abcd".repeat(20)]},
            "metadata": {"model_slug": "gpt-4o"},
        })
    };
    json!([{
        "id": "conv-1",
        "title": "private title",
        "create_time": 1_762_500_000.0,
        "mapping": {
            "u1": {"id": "u1", "parent": null, "message": message("u1", "user"), "children": ["a1"]},
            "a1": {"id": "a1", "parent": "u1", "message": message("a1", "assistant"), "children": []},
        },
    }])
    .to_string()
    .into_bytes()
}

#[test]
fn an_export_is_imported_once_and_audited_without_the_file() {
    let data = ingested();
    let (api, picks) = api_with_picks(&data);
    picks.lock().unwrap().extend([export(), export()]);

    let first = api.import_export(ImportExportRequest {}).unwrap();
    assert!(first.imported);
    assert_eq!(first.app.as_deref(), Some("ChatGPT"));
    assert_eq!(
        (first.conversations, first.skipped, first.replies),
        (1, 0, 1)
    );
    assert_eq!((first.added, first.updated, first.unchanged), (1, 0, 0));

    let again = api.import_export(ImportExportRequest {}).unwrap();
    assert_eq!((again.added, again.updated, again.unchanged), (0, 0, 1));

    let entries = data.store.audit_entries(10).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.action == AuditAction::Import));
    assert!(entries[0].detail.contains("ChatGPT"));
    assert!(
        entries
            .iter()
            .all(|e| !e.detail.contains("private") && !e.detail.contains(".json")),
        "the audit names the app, never the file or its content"
    );
}

#[test]
fn cancelling_the_dialog_imports_nothing() {
    let data = ingested();
    let (api, _) = api_with_picks(&data);
    let result = api.import_export(ImportExportRequest {}).unwrap();
    assert!(!result.imported);
    assert_eq!(result.app, None);
    assert_eq!(
        data.store.audit_entries(10).unwrap().len(),
        0,
        "nothing is audited"
    );
}

#[test]
fn a_file_that_is_not_an_export_is_refused() {
    let data = ingested();
    let (api, picks) = api_with_picks(&data);
    picks
        .lock()
        .unwrap()
        .push(b"{\"not\": \"an export\"}".to_vec());
    let error = api.import_export(ImportExportRequest {}).unwrap_err();
    assert!(matches!(error, ApiError::Validation { .. }), "{error:?}");
    assert_eq!(
        data.store.audit_entries(10).unwrap().len(),
        0,
        "nothing is audited"
    );
}
