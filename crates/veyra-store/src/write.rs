//! Idempotent batch writes (PRD FR-ING-05/06).
//!
//! Each event is matched on its dedup key: new events are inserted, changed events are
//! updated, and events whose content fingerprint is unchanged are skipped. Every bucket
//! touched (including the old bucket of a moved event) is rebuilt in the same
//! transaction, so rollups always equal the raw events.

use std::collections::{BTreeSet, HashMap};

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, params, params_from_iter};
use veyra_domain::event::{EventContext, UsageEvent};
use veyra_domain::identity::{GatewayId, ModelId, ProviderId};
use veyra_domain::pricing::EventCost;
use veyra_domain::time::UtcMillis;

use crate::fingerprint::fingerprint;
use crate::sql::{EVENT_COLUMNS, opt_count, opt_int, opt_text, text};
use crate::{StoreError, rollup};

/// An event together with its API-equivalent cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricedEvent {
    pub event: UsageEvent,
    pub cost: EventCost,
}

/// What a batch write changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteSummary {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub buckets_rebuilt: usize,
}

/// Writes a batch in one transaction; invalid events reject the whole batch.
pub fn write_events(
    connection: &mut rusqlite::Connection,
    events: &[PricedEvent],
    now: UtcMillis,
) -> Result<WriteSummary, StoreError> {
    for priced in events {
        priced
            .event
            .validate()
            .map_err(|source| StoreError::InvalidEvent {
                key: priced.event.dedup_key.as_str().to_owned(),
                source,
            })?;
    }

    let transaction = connection.transaction()?;
    let mut keys = KeyCache::default();
    let mut buckets = BTreeSet::new();
    let mut summary = WriteSummary::default();

    for priced in events {
        let fp = fingerprint(&priced.event, &priced.cost);
        let bucket = priced.event.occurred_at.bucket_start().0;
        let existing: Option<(i64, i64, i64)> = transaction
            .prepare_cached(
                "SELECT id, fingerprint, bucket_start FROM usage_events WHERE dedup_key = ?1",
            )?
            .query_row([priced.event.dedup_key.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?;

        let event_id = match existing {
            Some((_, stored_fp, _)) if stored_fp == fp => {
                summary.unchanged += 1;
                continue;
            }
            Some((id, _, old_bucket)) => {
                let values = event_values(&transaction, priced, fp, &mut keys, now)?;
                update_event(&transaction, id, values)?;
                buckets.insert(old_bucket);
                summary.updated += 1;
                id
            }
            None => {
                let values = event_values(&transaction, priced, fp, &mut keys, now)?;
                summary.inserted += 1;
                insert_event(&transaction, priced.event.dedup_key.as_str(), values)?
            }
        };
        upsert_cost(&transaction, event_id, &priced.cost)?;
        buckets.insert(bucket);
    }

    summary.buckets_rebuilt = rollup::rebuild(&transaction, &buckets)?;
    transaction.commit()?;
    Ok(summary)
}

fn insert_event(
    transaction: &Transaction<'_>,
    dedup_key: &str,
    values: Vec<Value>,
) -> Result<i64, StoreError> {
    let placeholders = (1..=EVENT_COLUMNS.len() + 1)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO usage_events (dedup_key, {}) VALUES ({placeholders})",
        EVENT_COLUMNS.join(", ")
    );
    let all = std::iter::once(text(dedup_key)).chain(values);
    transaction
        .prepare_cached(&sql)?
        .execute(params_from_iter(all))?;
    Ok(transaction.last_insert_rowid())
}

fn update_event(
    transaction: &Transaction<'_>,
    id: i64,
    values: Vec<Value>,
) -> Result<(), StoreError> {
    let assignments = EVENT_COLUMNS
        .iter()
        .enumerate()
        .map(|(i, column)| format!("{column} = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE usage_events SET {assignments} WHERE id = ?{}",
        EVENT_COLUMNS.len() + 1
    );
    let all = values
        .into_iter()
        .chain(std::iter::once(Value::Integer(id)));
    transaction
        .prepare_cached(&sql)?
        .execute(params_from_iter(all))?;
    Ok(())
}

pub(crate) fn upsert_cost(
    transaction: &Transaction<'_>,
    event_id: i64,
    cost: &EventCost,
) -> Result<(), StoreError> {
    let b = &cost.breakdown;
    transaction
        .prepare_cached(
            "INSERT INTO event_costs (event_id, catalog_version, rule_id, status, assumptions, \
             input_nusd, cache_read_nusd, cache_write_nusd, output_nusd, total_nusd, \
             cache_read_saving_nusd, cache_write_overhead_nusd, rate_verified_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
             ON CONFLICT (event_id) DO UPDATE SET catalog_version = excluded.catalog_version, \
             rule_id = excluded.rule_id, status = excluded.status, assumptions = excluded.assumptions, \
             input_nusd = excluded.input_nusd, cache_read_nusd = excluded.cache_read_nusd, \
             cache_write_nusd = excluded.cache_write_nusd, output_nusd = excluded.output_nusd, \
             total_nusd = excluded.total_nusd, cache_read_saving_nusd = excluded.cache_read_saving_nusd, \
             cache_write_overhead_nusd = excluded.cache_write_overhead_nusd, \
             rate_verified_at = excluded.rate_verified_at",
        )?
        .execute(params![
            event_id,
            cost.catalog_version,
            cost.rule_id,
            cost.status.as_str(),
            i64::from(cost.assumptions.bits()),
            b.input.0,
            b.cache_read.0,
            b.cache_write.0,
            b.output.0,
            b.total().0,
            b.cache_read_saving.map(|m| m.0),
            b.cache_write_overhead.map(|m| m.0),
            cost.rate_verified_at.map(|d| d.to_string()),
        ])?;
    Ok(())
}

/// Store ids for an event's session, project and device.
#[derive(Debug, Clone, Copy, Default)]
struct ContextIds {
    session: Option<i64>,
    project: Option<i64>,
    device: Option<i64>,
}

/// Natural key → id, per table, for the duration of one batch.
#[derive(Debug, Default)]
struct KeyCache(HashMap<(&'static str, String), i64>);

impl KeyCache {
    fn resolve(
        &mut self,
        transaction: &Transaction<'_>,
        table: &'static str,
        key: &str,
    ) -> Result<i64, StoreError> {
        if let Some(id) = self.0.get(&(table, key.to_owned())) {
            return Ok(*id);
        }
        transaction
            .prepare_cached(&format!(
                "INSERT INTO {table} (key) VALUES (?1) ON CONFLICT (key) DO NOTHING"
            ))?
            .execute([key])?;
        let id: i64 = transaction
            .prepare_cached(&format!("SELECT id FROM {table} WHERE key = ?1"))?
            .query_row([key], |row| row.get(0))?;
        self.0.insert((table, key.to_owned()), id);
        Ok(id)
    }
}

fn resolve_context(
    transaction: &Transaction<'_>,
    context: &EventContext,
    keys: &mut KeyCache,
) -> Result<ContextIds, StoreError> {
    let session = context
        .session_key
        .as_deref()
        .map(|key| keys.resolve(transaction, "sessions", key))
        .transpose()?;
    if let Some(session) = session {
        let parent = context
            .parent_session_key
            .as_deref()
            .map(|key| keys.resolve(transaction, "sessions", key))
            .transpose()?;
        if parent.is_some() || context.is_subagent {
            transaction
                .prepare_cached(
                    "UPDATE sessions SET parent_session_id = COALESCE(parent_session_id, ?2), \
                     is_subagent = MAX(is_subagent, ?3) WHERE id = ?1",
                )?
                .execute(params![session, parent, i64::from(context.is_subagent)])?;
        }
    }
    Ok(ContextIds {
        session,
        project: context
            .project_key
            .as_deref()
            .map(|key| keys.resolve(transaction, "projects", key))
            .transpose()?,
        device: context
            .device_key
            .as_deref()
            .map(|key| keys.resolve(transaction, "devices", key))
            .transpose()?,
    })
}

/// Values for [`EVENT_COLUMNS`], in order.
fn event_values(
    transaction: &Transaction<'_>,
    priced: &PricedEvent,
    fp: i64,
    keys: &mut KeyCache,
    now: UtcMillis,
) -> Result<Vec<Value>, StoreError> {
    let e = &priced.event;
    let ids = resolve_context(transaction, &e.context, keys)?;
    let a = &e.attribution;
    let t = &e.tokens;
    let acc = &e.accuracy;
    Ok(vec![
        Value::Integer(fp),
        Value::Integer(e.occurred_at.0),
        Value::Integer(e.occurred_at.bucket_start().0),
        text(e.source.connector.as_str()),
        Value::Integer(i64::from(e.source.parser_version)),
        opt_text(e.source.tool_version.as_deref()),
        opt_int(ids.session),
        opt_int(ids.project),
        opt_int(ids.device),
        Value::Integer(i64::from(e.context.is_subagent)),
        opt_text(a.provider.as_ref().map(ProviderId::as_str)),
        opt_text(a.model.as_ref().map(ModelId::as_str)),
        opt_text(a.raw_model.as_deref()),
        text(a.app.as_str()),
        text(a.interface.as_str()),
        opt_text(a.gateway.as_ref().map(GatewayId::as_str)),
        text(e.granularity.as_str()),
        text(e.service_tier.as_str()),
        Value::Integer(i64::from(e.inference_geo_us)),
        opt_count(e.requests.map(u64::from), "request count")?,
        opt_count(t.input_uncached, "input tokens")?,
        opt_count(t.cache_read, "cache read tokens")?,
        opt_count(t.cache_write_short, "cache write tokens")?,
        opt_count(t.cache_write_long, "cache write tokens")?,
        opt_count(t.cache_write_unspecified, "cache write tokens")?,
        opt_count(t.output_total, "output tokens")?,
        opt_count(t.reasoning, "reasoning tokens")?,
        opt_count(t.tool_tokens, "tool tokens")?,
        opt_int(e.reported_cost.map(|c| c.0)),
        text(acc.tokens.as_str()),
        text(acc.cache.as_str()),
        text(acc.reasoning.as_str()),
        text(acc.requests.as_str()),
        text(acc.cost.as_str()),
        Value::Integer(now.0),
    ])
}
