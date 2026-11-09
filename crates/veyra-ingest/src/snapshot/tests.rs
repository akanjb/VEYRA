use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use veyra_domain::connector::DatabaseQuery;
use veyra_testkit::TempDir;

use super::database_rows_in;

/// A live database in WAL mode, as applications such as OpenCode keep theirs.
fn live_database(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("app.db");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE message (id TEXT, data TEXT, picture BLOB);
         INSERT INTO message VALUES ('m1', '{\"tokens\":{\"input\":5}}', x'00ff');",
    )
    .unwrap();
    path
}

const MESSAGES: DatabaseQuery = DatabaseQuery {
    tables: &["message"],
    sql: "SELECT id, json_extract(data, '$.tokens.input') AS input, picture FROM message",
};

const MISSING: DatabaseQuery = DatabaseQuery {
    tables: &["session_v2"],
    sql: "SELECT id FROM session_v2",
};

fn scratch_entries(scratch: &TempDir) -> usize {
    std::fs::read_dir(scratch.path()).unwrap().count()
}

#[test]
fn turns_rows_into_json_lines_and_deletes_the_snapshot() {
    let dir = TempDir::new("snapshot-rows");
    let scratch = TempDir::new("snapshot-scratch");
    let path = live_database(&dir);

    let rows = database_rows_in(&path, &[MESSAGES, MISSING], scratch.path()).unwrap();

    // Binary columns are left out; queries over missing tables are skipped.
    assert_eq!(
        String::from_utf8(rows).unwrap(),
        "{\"id\":\"m1\",\"input\":5}\n"
    );
    assert_eq!(scratch_entries(&scratch), 0, "the snapshot is deleted");
}

#[test]
fn reports_a_database_that_cannot_be_read() {
    let dir = TempDir::new("snapshot-bad");
    let scratch = TempDir::new("snapshot-bad-scratch");
    let path = dir.path().join("not-a-db.db");
    std::fs::write(&path, b"plain text, not SQLite").unwrap();

    let error = database_rows_in(&path, &[MESSAGES], scratch.path()).unwrap_err();

    assert_eq!(error.file_issue(), Some("unreadable-database"));
    assert_eq!(scratch_entries(&scratch), 0);
}

/// The owning application keeps writing, with no busy timeout at all, while Veyra
/// snapshots its database over and over: it must never be refused a write.
#[test]
fn never_blocks_the_owning_applications_writer() {
    let dir = TempDir::new("snapshot-writer");
    let scratch = TempDir::new("snapshot-writer-scratch");
    let path = live_database(&dir);
    let stop = Arc::new(AtomicBool::new(false));

    let writer = {
        let (path, stop) = (path.clone(), Arc::clone(&stop));
        std::thread::spawn(move || {
            let db = Connection::open(&path).unwrap();
            db.busy_timeout(std::time::Duration::ZERO).unwrap();
            let mut written = 0_u32;
            while !stop.load(Ordering::Relaxed) {
                db.execute("INSERT INTO message VALUES ('m', '{}', NULL)", [])
                    .expect("a write was refused while Veyra was reading");
                written += 1;
            }
            written
        })
    };
    for _ in 0..25 {
        database_rows_in(&path, &[MESSAGES], scratch.path()).unwrap();
    }
    stop.store(true, Ordering::Relaxed);

    assert!(writer.join().unwrap() > 0, "the writer kept writing");
    assert_eq!(scratch_entries(&scratch), 0);
}
