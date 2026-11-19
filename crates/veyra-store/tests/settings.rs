//! Stored settings and the audit log (ADR-032).

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use veyra_domain::audit::{AuditAction, AuditActor, AuditEntry, AuditLog};
use veyra_domain::settings::SettingsRepository;
use veyra_domain::time::UtcMillis;

use support::TempDb;

#[test]
fn a_setting_keeps_the_value_stored_first() {
    let db = TempDb::new();
    assert_eq!(db.store.setting("k").unwrap(), None);
    assert_eq!(db.store.setting_or_init("k", "\"a\"").unwrap(), "\"a\"");
    assert_eq!(db.store.setting_or_init("k", "\"b\"").unwrap(), "\"a\"");
    assert_eq!(db.store.setting("k").unwrap().as_deref(), Some("\"a\""));
}

#[test]
fn setting_a_value_replaces_the_stored_one() {
    let db = TempDb::new();
    db.store.set_setting("k", "true").unwrap();
    db.store.set_setting("k", "false").unwrap();
    assert_eq!(db.store.setting("k").unwrap().as_deref(), Some("false"));
}

#[test]
fn audit_entries_come_back_newest_first() {
    let db = TempDb::new();
    for (at, detail) in [(1, "first"), (2, "second")] {
        db.store
            .record(&AuditEntry {
                at: UtcMillis(at),
                actor: AuditActor::User,
                action: AuditAction::Export,
                detail: detail.to_owned(),
            })
            .unwrap();
    }
    let entries = db.store.audit_entries(1).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].detail, "second");
    assert_eq!(db.store.audit_entries(10).unwrap().len(), 2);
}
