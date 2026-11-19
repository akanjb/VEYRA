//! Deleting stored usage by time range and connector (ADR-037): exactly the events in
//! scope go, with their summaries, and the rest stays as it was.

#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

mod support;

use std::sync::Arc;

use veyra_domain::identity::ConnectorId;
use veyra_domain::storage::{DeletionScope, StorageRepository};
use veyra_domain::time::UtcMillis;
use veyra_store::{Storage, Store};

use support::{HOUR, NOV_8, TempDb, bundled_catalog, event, priced, tokens};

const DAY: i64 = 24 * HOUR;

/// Stores one event an hour apart for each `(key, connector, at)`.
fn write(store: &Store, events: &[(&str, &str, i64)]) {
    let catalog = bundled_catalog();
    let events: Vec<_> = events
        .iter()
        .map(|(key, connector, at)| {
            let mut e = event(key, *at, tokens(100, 10));
            e.source.connector = ConnectorId::new(*connector).unwrap();
            priced(e, &catalog)
        })
        .collect();
    store.write_events(&events, UtcMillis(NOV_8)).unwrap();
}

fn storage(dir: &TempDb) -> Storage {
    let database = dir.path().join("delete.db");
    let store = Arc::new(Store::open(&database).unwrap());
    write(
        &store,
        &[
            ("a", "one", NOV_8),
            ("b", "two", NOV_8 + HOUR),
            ("c", "one", NOV_8 + DAY),
            ("d", "two", NOV_8 + DAY + HOUR),
        ],
    );
    Storage::new(store, database, dir.path().join("backups"))
}

#[test]
fn deletes_only_the_events_in_scope_and_their_summaries() {
    let dir = TempDb::new();
    let storage = storage(&dir);
    let first_day = DeletionScope {
        from: Some(UtcMillis(NOV_8)),
        until: Some(UtcMillis(NOV_8 + DAY)),
        connector: Some("one".to_owned()),
    };
    let preview = storage.deletion_preview(&first_day).unwrap();
    assert_eq!(preview.events, 1);
    assert_eq!(preview.first_event_at, Some(UtcMillis(NOV_8)));

    assert_eq!(storage.delete_events(&first_day).unwrap(), 1);
    assert_eq!(storage.deletion_preview(&first_day).unwrap().events, 0);
    // The summaries the pages read agree with the events left.
    let footprint = storage.footprint().unwrap();
    assert_eq!(footprint.events, 3);
    assert_eq!(footprint.first_event_at, Some(UtcMillis(NOV_8 + HOUR)));

    let everything = DeletionScope::default();
    assert_eq!(storage.deletion_preview(&everything).unwrap().events, 3);
    assert_eq!(storage.delete_events(&everything).unwrap(), 3);
    let footprint = storage.footprint().unwrap();
    assert_eq!((footprint.events, footprint.first_event_at), (0, None));
}

#[test]
fn open_bounds_widen_the_scope() {
    let dir = TempDb::new();
    let storage = storage(&dir);
    let since_second_day = DeletionScope {
        from: Some(UtcMillis(NOV_8 + DAY)),
        ..DeletionScope::default()
    };
    assert_eq!(
        storage.deletion_preview(&since_second_day).unwrap().events,
        2
    );
    let connector_two = DeletionScope {
        connector: Some("two".to_owned()),
        ..DeletionScope::default()
    };
    assert_eq!(storage.delete_events(&connector_two).unwrap(), 2);
    assert_eq!(storage.footprint().unwrap().events, 2);
}
