//! Importing a chat export into a real database (ADR-040): the records are stored once,
//! a second import of the same export adds nothing, and other files are refused.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::sums::BaseSums;
use veyra_domain::import::{ArchiveImport, ImportError};
use veyra_domain::time::UtcMillis;
use veyra_ingest::{ImportParts, Importer};
use veyra_registry::Registry;
use veyra_store::Store;
use veyra_testkit::TempDir;

fn export() -> Vec<u8> {
    let message = |id: &str, role: &str, text: &str| {
        json!({
            "id": id,
            "author": {"role": role},
            "create_time": 1_762_600_000.0,
            "content": {"content_type": "text", "parts": [text]},
            "metadata": {"model_slug": "gpt-4o"},
        })
    };
    json!([{
        "id": "c1",
        "create_time": 1_762_600_000.0,
        "mapping": {
            "u1": {"parent": null, "message": message("u1", "user", &"word ".repeat(80))},
            "a1": {"parent": "u1", "message": message("a1", "assistant", "abcdabcd")},
        },
    }])
    .to_string()
    .into_bytes()
}

fn totals(store: &Store) -> BaseSums {
    let rows = store
        .read_base_sums(&BaseSumsQuery {
            from: UtcMillis(0),
            until: UtcMillis(i64::MAX / 2),
            filters: Filters::new(),
            group_by: vec![],
            by_bucket: false,
        })
        .unwrap();
    BaseSums::total(rows.iter().map(|r| &r.sums))
}

#[test]
fn imports_an_export_once_and_refuses_other_files() {
    let dir = TempDir::new("veyra-import-test");
    let store = Arc::new(Store::open(&dir.path().join("veyra.db")).unwrap());
    let changes = Arc::new(AtomicU64::new(0));
    let seen = Arc::clone(&changes);
    let importer = Importer::new(ImportParts {
        store: Arc::clone(&store),
        registry: Arc::new(Registry::bundled().unwrap()),
        importers: veyra_connectors::exports::all(),
        on_change: Box::new(move |_| {
            seen.fetch_add(1, Ordering::Relaxed);
        }),
    });

    let first = importer.import(&mut Cursor::new(export())).unwrap();
    assert_eq!(first.app_name, "ChatGPT");
    assert_eq!((first.counts.conversations, first.counts.replies), (1, 1));
    assert_eq!((first.inserted, first.unchanged), (1, 0));
    let sums = totals(&store);
    assert_eq!(sums.events, 1);
    assert_eq!(sums.tokens.output_total, Some(2));

    let again = importer.import(&mut Cursor::new(export())).unwrap();
    assert_eq!((again.inserted, again.unchanged), (0, 1));
    assert_eq!(totals(&store).events, 1);
    assert_eq!(
        changes.load(Ordering::Relaxed),
        1,
        "only the first changed data"
    );

    assert_eq!(
        importer.import(&mut Cursor::new(b"not an export".to_vec())),
        Err(ImportError::NotRecognized)
    );
}
