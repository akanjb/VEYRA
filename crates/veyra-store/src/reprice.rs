//! Re-pricing after a catalogue change (PRD FR-PRICE-03).
//!
//! The store finds events priced with another catalogue version and persists new costs;
//! the caller prices them with the domain engine, keeping pricing out of data access.

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use veyra_domain::event::UsageEvent;
use veyra_domain::pricing::EventCost;

use crate::fingerprint::fingerprint;
use crate::mapping::{EVENT_SELECT, row_to_event};
use crate::write::upsert_cost;
use crate::{Store, StoreError, rollup};

/// A persisted event with its store id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: i64,
    pub event: UsageEvent,
}

impl Store {
    /// Up to `limit` events whose cost was computed with a catalogue version other than
    /// `catalog_version`, oldest first.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure or unreadable stored data.
    pub fn events_priced_with_other_catalog(
        &self,
        catalog_version: &str,
        limit: usize,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        let limit = i64::try_from(limit).map_err(|_| StoreError::OutOfRange("limit"))?;
        self.with_reader(|connection| {
            let sql = format!(
                "{EVENT_SELECT} WHERE e.id IN \
                 (SELECT event_id FROM event_costs WHERE catalog_version <> ?1) \
                 ORDER BY e.id LIMIT ?2"
            );
            let mut statement = connection.prepare_cached(&sql)?;
            let rows = statement.query_map(rusqlite::params![catalog_version, limit], |row| {
                Ok(row_to_event(row))
            })?;
            rows.map(|row| {
                let (id, event) = row??;
                Ok(StoredEvent { id, event })
            })
            .collect()
        })
    }

    /// Persists new costs for stored events and rebuilds the affected rollups.
    ///
    /// # Errors
    /// Returns [`StoreError`] on database failure; nothing is written in that case.
    pub fn apply_costs(&self, updates: &[(StoredEvent, EventCost)]) -> Result<usize, StoreError> {
        let mut writer = self.lock_writer()?;
        let transaction = writer.transaction()?;
        let mut buckets = BTreeSet::new();
        for (stored, cost) in updates {
            upsert_cost(&transaction, stored.id, cost)?;
            transaction
                .prepare_cached("UPDATE usage_events SET fingerprint = ?2 WHERE id = ?1")?
                .execute(rusqlite::params![
                    stored.id,
                    fingerprint(&stored.event, cost)
                ])?;
            buckets.insert(stored.event.occurred_at.bucket_start().0);
        }
        rollup::rebuild(&transaction, &buckets)?;
        transaction.commit()?;
        if !updates.is_empty() {
            self.data_version.fetch_add(1, Ordering::AcqRel);
        }
        Ok(updates.len())
    }
}
