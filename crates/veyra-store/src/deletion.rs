//! Deleting Veyra's own stored usage by time range and connector (FR-STO-04). Costs and
//! tool calls go with their events (`ON DELETE CASCADE`), and every rollup level is
//! rebuilt for the buckets the deleted events were in, in the same transaction. Nothing
//! here touches the tools' files.

use std::collections::BTreeSet;

use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};
use veyra_domain::source::PortError;
use veyra_domain::storage::{BackupContents, DeletionScope};
use veyra_domain::time::UtcMillis;

use crate::{Store, StoreError, rollup};

/// The `WHERE` clause selecting the events of `scope`, and its parameters. Bounds are
/// matched on the bucket, which the bucket index serves; they fall on bucket edges.
fn condition(scope: &DeletionScope) -> (String, Vec<Value>) {
    let mut terms = vec!["1".to_owned()];
    let mut values = Vec::new();
    for (term, value) in [
        ("bucket_start >= ?", scope.from.map(|at| Value::from(at.0))),
        ("bucket_start < ?", scope.until.map(|at| Value::from(at.0))),
        ("connector_id = ?", scope.connector.clone().map(Value::from)),
    ] {
        if let Some(value) = value {
            terms.push(term.to_owned());
            values.push(value);
        }
    }
    (terms.join(" AND "), values)
}

fn preview(connection: &Connection, scope: &DeletionScope) -> Result<BackupContents, StoreError> {
    let (condition, values) = condition(scope);
    let (events, first, last) = connection.query_row(
        &format!(
            "SELECT COUNT(*), MIN(occurred_at), MAX(occurred_at) FROM usage_events \
             WHERE {condition}"
        ),
        params_from_iter(values),
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        },
    )?;
    Ok(BackupContents {
        events: u64::try_from(events).unwrap_or(0),
        first_event_at: first.map(UtcMillis),
        last_event_at: last.map(UtcMillis),
    })
}

impl Store {
    /// The stored events `scope` covers.
    pub(crate) fn deletion_preview(
        &self,
        scope: &DeletionScope,
    ) -> Result<BackupContents, PortError> {
        Ok(self.with_reader(|connection| preview(connection, scope))?)
    }

    /// Deletes the events `scope` covers and rebuilds their buckets. Returns the events
    /// deleted.
    pub(crate) fn delete_events(&self, scope: &DeletionScope) -> Result<u64, PortError> {
        self.change(|transaction| {
            let (condition, values) = condition(scope);
            let mut statement = transaction
                .prepare(&format!(
                    "SELECT DISTINCT bucket_start FROM usage_events WHERE {condition}"
                ))
                .map_err(StoreError::from)?;
            let buckets = statement
                .query_map(params_from_iter(values.iter()), |row| row.get::<_, i64>(0))
                .map_err(StoreError::from)?
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(StoreError::from)?;
            drop(statement);
            let deleted = transaction
                .execute(
                    &format!("DELETE FROM usage_events WHERE {condition}"),
                    params_from_iter(values),
                )
                .map_err(StoreError::from)?;
            rollup::rebuild(transaction, &buckets)?;
            Ok(u64::try_from(deleted).unwrap_or(u64::MAX))
        })
    }
}
