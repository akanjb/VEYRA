//! Renaming and merging projects (ADR-027).
//!
//! Every event keeps the project it was recorded in (`source_project_id`); `project_id`
//! is the project its usage counts toward. Merging and unmerging rewrite `project_id` of
//! the affected events and rebuild their buckets' rollups in the same transaction, so
//! every view changes at once and nothing recorded is lost.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use veyra_domain::project::{
    ProjectChangeError, ProjectRecord, ProjectRepository, normalise_project_name,
};
use veyra_domain::source::PortError;

use crate::{Store, StoreError, rollup};

const PROJECT_COLUMNS: &str = "id, key, display_name, merged_into";

fn project_from(row: &Row<'_>) -> rusqlite::Result<ProjectRecord> {
    Ok(ProjectRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        display_name: row.get(2)?,
        merged_into: row.get(3)?,
    })
}

fn project(connection: &Connection, id: i64) -> Result<Option<ProjectRecord>, StoreError> {
    Ok(connection
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
            [id],
            project_from,
        )
        .optional()?)
}

fn existing(connection: &Connection, id: i64) -> Result<ProjectRecord, PortError> {
    project(connection, id)?.ok_or_else(|| ProjectChangeError::NotFound(id).into())
}

/// Points the events recorded in `sources` at `target` and rebuilds their buckets.
fn reassign(
    transaction: &Transaction<'_>,
    sources: &[i64],
    target: impl Fn(i64) -> i64,
) -> Result<(), StoreError> {
    let mut buckets = BTreeSet::new();
    for source in sources {
        let mut statement = transaction.prepare_cached(
            "SELECT DISTINCT bucket_start FROM usage_events WHERE source_project_id = ?1",
        )?;
        for bucket in statement.query_map([source], |row| row.get::<_, i64>(0))? {
            buckets.insert(bucket?);
        }
        transaction
            .prepare_cached("UPDATE usage_events SET project_id = ?2 WHERE source_project_id = ?1")?
            .execute(params![source, target(*source)])?;
    }
    rollup::rebuild(transaction, &buckets)?;
    Ok(())
}

impl ProjectRepository for Store {
    fn list_projects(&self) -> Result<Vec<ProjectRecord>, PortError> {
        self.read_all(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects ORDER BY key"),
            project_from,
        )
    }

    fn rename_project(&self, id: i64, name: Option<&str>) -> Result<ProjectRecord, PortError> {
        self.change(|transaction| {
            existing(transaction, id)?;
            transaction
                .execute(
                    "UPDATE projects SET display_name = ?2 WHERE id = ?1",
                    params![id, normalise_project_name(name)],
                )
                .map_err(StoreError::from)?;
            existing(transaction, id)
        })
    }

    fn merge_project(&self, from: i64, into: i64) -> Result<(), PortError> {
        self.change(|transaction| {
            existing(transaction, from)?;
            // Merging into a merged project means merging into its target.
            let target = existing(transaction, into)?;
            let into = target.merged_into.unwrap_or(target.id);
            if into == from {
                return Err(ProjectChangeError::SelfMerge.into());
            }
            // `from` and everything already merged into it, so merges stay one level deep.
            let mut statement = transaction
                .prepare("SELECT id FROM projects WHERE id = ?1 OR merged_into = ?1")
                .map_err(StoreError::from)?;
            let moved: Vec<i64> = statement
                .query_map([from], |row| row.get(0))
                .map_err(StoreError::from)?
                .collect::<Result<_, _>>()
                .map_err(StoreError::from)?;
            drop(statement);
            for id in &moved {
                transaction
                    .execute(
                        "UPDATE projects SET merged_into = ?2 WHERE id = ?1",
                        params![id, into],
                    )
                    .map_err(StoreError::from)?;
            }
            reassign(transaction, &moved, |_| into)?;
            Ok(())
        })
    }

    fn unmerge_project(&self, id: i64) -> Result<(), PortError> {
        self.change(|transaction| {
            if existing(transaction, id)?.merged_into.is_none() {
                return Err(ProjectChangeError::NotMerged(id).into());
            }
            transaction
                .execute("UPDATE projects SET merged_into = NULL WHERE id = ?1", [id])
                .map_err(StoreError::from)?;
            reassign(transaction, &[id], |source| source)?;
            Ok(())
        })
    }
}
