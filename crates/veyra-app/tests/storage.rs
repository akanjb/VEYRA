//! Contract tests of the Storage page and of backups (ADR-033), on the ingested fixtures.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use std::sync::atomic::Ordering;

use veyra_app::ApiError;
use veyra_app::dto::{CreateBackupRequest, RestoreRequest, StorageRequest};
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
