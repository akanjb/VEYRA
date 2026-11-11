//! Forward-only schema migrations tracked with `PRAGMA user_version` (ADR-003).
//!
//! Each migration runs in its own transaction together with the version bump, so an
//! interrupted upgrade leaves the database at the last complete version.

use rusqlite::Connection;

use crate::StoreError;

/// Migration SQL, in order. Index `i` upgrades version `i` to `i + 1`. Never edit a
/// released migration; add a new one.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_ingestion.sql"),
    include_str!("../migrations/0003_write_path.sql"),
    include_str!("../migrations/0004_request_sizes.sql"),
    include_str!("../migrations/0005_project_merges.sql"),
];

/// The schema version this build creates and understands.
#[expect(clippy::cast_possible_truncation, reason = "a handful of migrations")]
pub const LATEST_SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// Applies every pending migration. Returns the resulting version.
pub fn migrate(connection: &mut Connection) -> Result<u32, StoreError> {
    let current: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > LATEST_SCHEMA_VERSION {
        return Err(StoreError::SchemaTooNew {
            found: current,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version =
            u32::try_from(index + 1).map_err(|_| StoreError::OutOfRange("schema version"))?;
        let apply = |connection: &mut Connection| -> Result<(), rusqlite::Error> {
            let transaction = connection.transaction()?;
            transaction.execute_batch(sql)?;
            transaction.pragma_update(None, "user_version", version)?;
            transaction.commit()
        };
        apply(connection).map_err(|source| StoreError::Migration { version, source })?;
    }
    Ok(LATEST_SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{LATEST_SCHEMA_VERSION, migrate};
    use crate::StoreError;
    use crate::connection::register_functions;

    /// An in-memory database with the functions migrations call.
    fn memory() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        register_functions(&connection).unwrap();
        connection
    }

    #[test]
    fn migrates_a_fresh_database_and_is_idempotent() {
        let mut connection = memory();
        assert_eq!(migrate(&mut connection).unwrap(), LATEST_SCHEMA_VERSION);
        assert_eq!(migrate(&mut connection).unwrap(), LATEST_SCHEMA_VERSION);
        let tables: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'table' \
                 AND name IN ('usage_events', 'event_costs', 'rollup_15m', 'sessions')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 4);
    }

    #[test]
    fn upgrades_a_version_one_database_in_place() {
        let mut connection = memory();
        connection.execute_batch(super::MIGRATIONS[0]).unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        connection
            .execute("INSERT INTO sessions (key) VALUES ('claude-code:kept')", [])
            .unwrap();

        assert_eq!(migrate(&mut connection).unwrap(), LATEST_SCHEMA_VERSION);
        let kept: i64 = connection
            .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kept, 1, "existing data survives the upgrade");
        let default: String = connection
            .query_row(
                "SELECT dflt_value FROM pragma_table_info('usage_events') WHERE name = 'precedence'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(default, "0", "existing events get the lowest precedence");
        let dropped: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'usage_events_occurred_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dropped, 0);
    }

    #[test]
    fn rekeys_tool_calls_without_losing_them() {
        let mut connection = memory();
        connection.execute_batch(super::MIGRATIONS[0]).unwrap();
        connection.execute_batch(super::MIGRATIONS[1]).unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
        // A stand-alone row: no parent event exists in this scratch database.
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .unwrap();
        connection
            .execute(
                "INSERT INTO tool_calls (event_id, tool_use_id, tool_name, is_error) \
                 VALUES (7, 'toolu_kept', 'Bash', 1)",
                [],
            )
            .unwrap();

        assert_eq!(migrate(&mut connection).unwrap(), LATEST_SCHEMA_VERSION);
        let kept: (i64, String, i64) = connection
            .query_row(
                "SELECT event_id, tool_name, is_error FROM tool_calls WHERE tool_use_id = 'toolu_kept'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(kept, (7, "Bash".to_owned(), 1));
        let schema: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name = 'tool_calls'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(schema.contains("WITHOUT ROWID"));
        let index: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name IN \
                 ('tool_calls_event', 'event_costs_catalog_version')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            index, 1,
            "the event index is kept, the catalogue index is gone"
        );
    }

    #[test]
    fn refuses_databases_from_newer_versions() {
        let mut connection = memory();
        connection
            .pragma_update(None, "user_version", LATEST_SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            migrate(&mut connection),
            Err(StoreError::SchemaTooNew { .. })
        ));
    }
}
