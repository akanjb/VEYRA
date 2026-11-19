//! Contract tests of the Storage page and of backups (ADR-033), on the ingested fixtures.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use std::sync::atomic::Ordering;

use veyra_app::ApiError;
use veyra_app::dto::{CreateBackupRequest, DeleteDataRequest, RestoreRequest, StorageRequest};
use veyra_domain::audit::{AuditAction, AuditLog};

use support::{api, api_with_restarts, ingested};

fn utc() -> StorageRequest {
    StorageRequest {
        time_zone: "UTC".to_owned(),
    }
}

#[test]
fn the_page_counts_what_is_stored() {
    let data = ingested();
    let page = api(&data).get_storage(utc()).unwrap();
    assert_eq!(page.events, 4, "the fixtures hold four requests");
    assert!(page.database_bytes > 0);
    assert!(page.tables.iter().any(|t| t.table == "usage_events"));
    let monthly: u64 = page.months.iter().map(|m| m.events).sum();
    assert_eq!(monthly, page.events);
    assert_eq!(page.months.last().unwrap().month, "2026-11");
    assert_eq!(page.backups.len(), 0);
    assert_eq!(page.kept_backups, 7);
}

#[test]
fn a_backup_is_previewed_then_restored_on_restart() {
    let data = ingested();
    let (api, _, restarts) = api_with_restarts(&data);
    let backup = api.create_backup(CreateBackupRequest {}).unwrap();
    assert_eq!(
        api.get_storage(utc()).unwrap().backups,
        vec![backup.clone()]
    );

    let preview = api
        .preview_restore(RestoreRequest {
            name: backup.name.clone(),
        })
        .unwrap();
    assert_eq!(preview.backup, backup);
    assert_eq!(preview.events, 4);
    assert_eq!(preview.current_events, 4);
    assert_eq!(
        restarts.load(Ordering::Relaxed),
        0,
        "a preview changes nothing"
    );

    api.restore_backup(RestoreRequest { name: backup.name })
        .unwrap();
    assert_eq!(restarts.load(Ordering::Relaxed), 1);
    let audit = data.store.audit_entries(1).unwrap();
    assert_eq!(audit[0].action, AuditAction::Restore);
}

#[test]
fn unknown_or_outside_backups_are_refused() {
    let data = ingested();
    let api = api(&data);
    for name in ["veyra-1.db", "../veyra.db", "veyra.db"] {
        let request = || RestoreRequest {
            name: name.to_owned(),
        };
        assert!(matches!(
            api.preview_restore(request()),
            Err(ApiError::NotFound { .. })
        ));
        assert!(matches!(
            api.restore_backup(request()),
            Err(ApiError::NotFound { .. })
        ));
    }
    assert!(matches!(
        api.get_storage(StorageRequest {
            time_zone: "Nowhere/Else".to_owned()
        }),
        Err(ApiError::Validation { .. })
    ));
}

fn deletion(start: Option<&str>, end: Option<&str>, connector: Option<&str>) -> DeleteDataRequest {
    DeleteDataRequest {
        time_zone: "UTC".to_owned(),
        start: start.map(str::to_owned),
        end: end.map(str::to_owned),
        connector_id: connector.map(str::to_owned),
    }
}

#[test]
fn stored_usage_is_deleted_after_a_backup_and_audited() {
    let data = ingested();
    let api = api(&data);
    let everything = || deletion(None, None, Some("claude-code"));
    let preview = api.preview_delete(everything()).unwrap();
    assert_eq!(preview.events, 4);
    assert!(preview.first_event_at <= preview.last_event_at);
    assert_eq!(
        api.preview_delete(deletion(Some("2000-01-01"), Some("2000-01-31"), None))
            .unwrap()
            .events,
        0
    );

    let deleted = api.delete_data(everything()).unwrap();
    assert_eq!(deleted.events, 4);
    let page = api.get_storage(utc()).unwrap();
    assert_eq!(page.events, 0);
    assert_eq!(page.backups, vec![deleted.backup.clone()]);
    let preview = api
        .preview_restore(RestoreRequest {
            name: deleted.backup.name,
        })
        .unwrap();
    assert_eq!(preview.events, 4, "the backup undoes the deletion");
    let audit = data.store.audit_entries(1).unwrap();
    assert_eq!(audit[0].action, AuditAction::Delete);
}

#[test]
fn invalid_deletions_are_refused_without_deleting() {
    let data = ingested();
    let api = api(&data);
    for request in [
        deletion(Some("2026-11-09"), Some("2026-11-08"), None),
        deletion(Some("November"), None, None),
        deletion(None, None, Some("no-such-connector")),
        DeleteDataRequest {
            time_zone: "Nowhere/Else".to_owned(),
            ..deletion(None, None, None)
        },
    ] {
        assert!(matches!(
            api.delete_data(request),
            Err(ApiError::Validation { .. })
        ));
    }
    assert_eq!(api.get_storage(utc()).unwrap().events, 4);
}
