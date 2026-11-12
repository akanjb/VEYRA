//! The database's footprint, backups and restore (ADR-033).

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use std::sync::Arc;

use veyra_domain::storage::StorageRepository;
use veyra_domain::time::UtcMillis;
use veyra_store::{KEPT_BACKUPS, Storage, Store, apply_pending_restore};

use support::{HOUR, NOV_8, TempDb, bundled_catalog, event, priced, tokens};

fn write(store: &Store, keys: &[&str]) {
    let catalog = bundled_catalog();
    let events: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let at = NOV_8 + i64::try_from(i).unwrap() * HOUR;
            priced(event(key, at, tokens(100, 10)), &catalog)
        })
        .collect();
    store.write_events(&events, UtcMillis(NOV_8)).unwrap();
}

#[test]
fn a_restored_backup_round_trips() {
    let dir = TempDb::new();
    let database = dir.path().join("restore.db");
    let backups = dir.path().join("backups");
    let store = Arc::new(Store::open(&database).unwrap());
    write(&store, &["a", "b"]);
    let storage = Storage::new(Arc::clone(&store), database.clone(), backups);

    let footprint = storage.footprint().unwrap();
    assert_eq!(footprint.events, 2);
    assert_eq!(footprint.first_event_at, Some(UtcMillis(NOV_8)));
    assert_eq!(footprint.last_event_at, Some(UtcMillis(NOV_8 + HOUR)));
    assert!(
        footprint
            .tables
            .iter()
            .any(|t| t.table == "usage_events" && t.bytes > 0)
    );

    let backup = storage.create_backup(UtcMillis(1)).unwrap();
    write(&store, &["c"]);
    let contents = storage.backup_contents(&backup.name).unwrap();
    assert_eq!(contents.events, 2, "the backup holds what was stored then");

    storage.schedule_restore(&backup.name).unwrap();
    drop(storage);
    drop(store);
    assert!(apply_pending_restore(&database).unwrap());
    assert!(!apply_pending_restore(&database).unwrap(), "applied once");
    let restored = Arc::new(Store::open(&database).unwrap());
    let storage = Storage::new(restored, database, dir.path().join("backups"));
    assert_eq!(storage.footprint().unwrap().events, 2);
}

#[test]
fn only_the_newest_backups_are_kept() {
    let dir = TempDb::new();
    let store = Arc::new(Store::open(&dir.path().join("keep.db")).unwrap());
    let storage = Storage::new(store, dir.path().join("keep.db"), dir.path().join("b"));
    let count = i64::try_from(KEPT_BACKUPS).unwrap() + 2;
    for at in 1..=count {
        storage.create_backup(UtcMillis(at)).unwrap();
    }
    let kept = storage.backups().unwrap();
    assert_eq!(kept.len(), KEPT_BACKUPS);
    assert_eq!(kept[0].created_at, UtcMillis(count), "newest first");
    // A daily backup is not taken again within a day of the newest.
    assert!(
        storage
            .back_up_daily(UtcMillis(count + 1))
            .unwrap()
            .is_none()
    );
    assert!(
        storage
            .back_up_daily(UtcMillis(count + 86_400_000))
            .unwrap()
            .is_some()
    );
}

#[test]
fn names_outside_the_backup_folder_are_refused() {
    let dir = TempDb::new();
    let store = Arc::new(Store::open(&dir.path().join("x.db")).unwrap());
    let storage = Storage::new(store, dir.path().join("x.db"), dir.path().join("b"));
    for name in ["../x.db", "veyra-1.db", "x.db"] {
        assert!(storage.backup_contents(name).is_err(), "{name}");
        assert!(storage.schedule_restore(name).is_err(), "{name}");
    }
}
