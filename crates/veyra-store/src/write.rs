//! Idempotent batch writes (PRD FR-ING-05/06).
//!
//! Each event is matched on its dedup key: new events are inserted, changed events are
//! updated, and events whose content fingerprint is unchanged are skipped. Every bucket
//! touched (including the old bucket of a moved event) is rebuilt in the same
//! transaction, so rollups always equal the raw events.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

use rusqlite::types::Value;
use rusqlite::{
    CachedStatement, OptionalExtension, Statement, Transaction, params, params_from_iter,
};
use veyra_domain::event::{EventContext, UsageEvent};
use veyra_domain::identity::{GatewayId, ModelId, ProviderId};
use veyra_domain::pricing::EventCost;
use veyra_domain::time::UtcMillis;
use veyra_domain::tooling::{ToolCall, ToolOutcome, mcp_parts};

use crate::fingerprint::fingerprint;
use crate::sql::{EVENT_COLUMNS, opt_count, opt_int, opt_text, text};
use crate::{StoreError, rollup};

/// An event together with its API-equivalent cost, as written to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricedEvent {
    pub event: UsageEvent,
    pub cost: EventCost,
    /// How complete this observation is (see `RawUsage::precedence`). A lower value never
    /// replaces a higher one; on a tie the earlier `occurred_at` is kept.
    pub precedence: i64,
    /// Tool invocations requested in this event (metadata only).
    pub tool_calls: Vec<ToolCall>,
}

impl PricedEvent {
    /// An event without tool calls, at default precedence.
    #[must_use]
    pub fn new(event: UsageEvent, cost: EventCost) -> Self {
        Self {
            event,
            cost,
            precedence: 0,
            tool_calls: Vec::new(),
        }
    }
}

/// A file's read position, committed atomically with the events read from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCheckpoint {
    pub source_id: i64,
    /// Path relative to the source root.
    pub path: String,
    pub size: u64,
    pub modified_at: Option<UtcMillis>,
    /// Byte offset after the last complete line read.
    pub offset: u64,
    /// Fingerprint of the file's first `head_len` bytes.
    pub head_hash: i64,
    pub head_len: u32,
    pub parser_version: u32,
}

/// Everything one ingest step writes, committed in a single transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IngestBatch {
    pub events: Vec<PricedEvent>,
    pub tool_outcomes: Vec<ToolOutcome>,
    pub checkpoint: Option<FileCheckpoint>,
}

/// What a batch write changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteSummary {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// Observations ignored because a more complete one is already stored.
    pub superseded: usize,
    pub buckets_rebuilt: usize,
}

/// The stored state of an event, for deciding how a new observation applies.
struct Existing {
    id: i64,
    fingerprint: i64,
    bucket: i64,
    precedence: i64,
    occurred_at: i64,
}

/// Writes events, tool outcomes and an optional checkpoint in one transaction.
/// Invalid events reject the whole batch.
pub fn commit(
    connection: &mut rusqlite::Connection,
    events: &[PricedEvent],
    tool_outcomes: &[ToolOutcome],
    checkpoint: Option<&FileCheckpoint>,
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

    let mut statements = EventStatements::prepare(&transaction)?;
    for priced in events {
        let fp = fingerprint(&priced.event, &priced.cost);
        let new_bucket = priced.event.occurred_at.bucket_start().0;
        let (event_id, bucket) = match existing(&mut statements.existing, priced)? {
            Some(e) if e.fingerprint == fp => {
                summary.unchanged += 1;
                (e.id, e.bucket)
            }
            Some(e) if is_superseded(priced, &e) => {
                summary.superseded += 1;
                (e.id, e.bucket)
            }
            Some(e) => {
                let values = event_values(&transaction, priced, fp, &mut keys, now)?;
                update_event(&mut statements.update, e.id, values)?;
                bind_cost(&mut statements.cost, e.id, &priced.cost)?;
                buckets.insert(e.bucket);
                buckets.insert(new_bucket);
                summary.updated += 1;
                (e.id, new_bucket)
            }
            None => {
                let values = event_values(&transaction, priced, fp, &mut keys, now)?;
                let key = priced.event.dedup_key.as_str();
                let id = insert_event(&mut statements.insert, key, values)?;
                bind_cost(&mut statements.cost, id, &priced.cost)?;
                buckets.insert(new_bucket);
                summary.inserted += 1;
                (id, new_bucket)
            }
        };
        // Tool calls accumulate across observations of the same request; a new call
        // changes its bucket's tool rollup even when the event itself is unchanged.
        if insert_tool_calls(&mut statements.tool_call, event_id, &priced.tool_calls)? {
            buckets.insert(bucket);
        }
    }
    drop(statements);
    buckets.extend(apply_tool_outcomes(&transaction, tool_outcomes)?);
    if let Some(checkpoint) = checkpoint {
        upsert_checkpoint(&transaction, checkpoint, now)?;
    }

    summary.buckets_rebuilt = rollup::rebuild(&transaction, &buckets)?;
    transaction.commit()?;
    Ok(summary)
}

const EXISTING_SQL: &str = "SELECT id, fingerprint, bucket_start, precedence, occurred_at \
     FROM usage_events WHERE dedup_key = ?1";

const INSERT_TOOL_CALL_SQL: &str = "INSERT INTO tool_calls (event_id, tool_use_id, tool_name, mcp_server, mcp_tool) \
     VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (tool_use_id) DO NOTHING";

/// Statements run for every event, prepared once per batch. Looking a statement up in
/// the cache hashes its whole SQL text, which for these long statements cost more per
/// event than running them.
struct EventStatements<'t> {
    existing: CachedStatement<'t>,
    insert: CachedStatement<'t>,
    update: CachedStatement<'t>,
    cost: CachedStatement<'t>,
    tool_call: CachedStatement<'t>,
}

impl<'t> EventStatements<'t> {
    fn prepare(transaction: &'t Transaction<'_>) -> Result<Self, StoreError> {
        Ok(Self {
            existing: transaction.prepare_cached(EXISTING_SQL)?,
            insert: transaction.prepare_cached(&INSERT_EVENT)?,
            update: transaction.prepare_cached(&UPDATE_EVENT)?,
            cost: transaction.prepare_cached(UPSERT_COST_SQL)?,
            tool_call: transaction.prepare_cached(INSERT_TOOL_CALL_SQL)?,
        })
    }
}

fn existing(
    statement: &mut Statement<'_>,
    priced: &PricedEvent,
) -> Result<Option<Existing>, StoreError> {
    Ok(statement
        .query_row([priced.event.dedup_key.as_str()], |row| {
            Ok(Existing {
                id: row.get(0)?,
                fingerprint: row.get(1)?,
                bucket: row.get(2)?,
                precedence: row.get(3)?,
                occurred_at: row.get(4)?,
            })
        })
        .optional()?)
}

/// Whether the stored observation is more complete than the new one, making the result
/// independent of the order in which observations are read.
fn is_superseded(new: &PricedEvent, stored: &Existing) -> bool {
    new.precedence < stored.precedence
        || (new.precedence == stored.precedence && new.event.occurred_at.0 > stored.occurred_at)
}

/// Stores the calls not stored yet. Returns whether any was new.
fn insert_tool_calls(
    insert: &mut Statement<'_>,
    event_id: i64,
    calls: &[ToolCall],
) -> Result<bool, StoreError> {
    let mut inserted = false;
    for call in calls {
        let (server, tool) = mcp_parts(&call.name).unzip();
        inserted |= insert.execute(params![event_id, call.id, call.name, server, tool])? > 0;
    }
    Ok(inserted)
}

/// Records the outcomes of stored calls. Returns the buckets of the events whose calls
/// changed, whose tool rollup must be rebuilt.
fn apply_tool_outcomes(
    transaction: &Transaction<'_>,
    outcomes: &[ToolOutcome],
) -> Result<BTreeSet<i64>, StoreError> {
    let mut update = transaction.prepare_cached(
        "UPDATE tool_calls SET is_error = ?2 WHERE tool_use_id = ?1 AND is_error IS NOT ?2 \
         RETURNING (SELECT bucket_start FROM usage_events WHERE id = event_id)",
    )?;
    let mut buckets = BTreeSet::new();
    for outcome in outcomes {
        let changed = update
            .query_row(
                params![outcome.tool_call_id, i64::from(outcome.is_error)],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        buckets.extend(changed);
    }
    Ok(buckets)
}

fn upsert_checkpoint(
    transaction: &Transaction<'_>,
    checkpoint: &FileCheckpoint,
    now: UtcMillis,
) -> Result<(), StoreError> {
    let size = i64::try_from(checkpoint.size).map_err(|_| StoreError::OutOfRange("file size"))?;
    let offset =
        i64::try_from(checkpoint.offset).map_err(|_| StoreError::OutOfRange("file offset"))?;
    transaction
        .prepare_cached(
            "INSERT INTO source_files (source_id, path, size, modified_at, byte_offset, head_hash, \
             head_len, parser_version, state, last_ingested_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', ?9) \
             ON CONFLICT (source_id, path) DO UPDATE SET size = excluded.size, \
             modified_at = excluded.modified_at, byte_offset = excluded.byte_offset, \
             head_hash = excluded.head_hash, head_len = excluded.head_len, \
             parser_version = excluded.parser_version, state = 'active', \
             last_ingested_at = excluded.last_ingested_at",
        )?
        .execute(params![
            checkpoint.source_id,
            checkpoint.path,
            size,
            checkpoint.modified_at.map(|m| m.0),
            offset,
            checkpoint.head_hash,
            checkpoint.head_len,
            checkpoint.parser_version,
            now.0,
        ])?;
    Ok(())
}

/// Inserts one event: the dedup key, then [`EVENT_COLUMNS`] (built once).
static INSERT_EVENT: LazyLock<String> = LazyLock::new(|| {
    let placeholders = (1..=EVENT_COLUMNS.len() + 1)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO usage_events (dedup_key, {}) VALUES ({placeholders})",
        EVENT_COLUMNS.join(", ")
    )
});

/// Updates one event's [`EVENT_COLUMNS`]; the id is the last parameter (built once).
static UPDATE_EVENT: LazyLock<String> = LazyLock::new(|| {
    let assignments = EVENT_COLUMNS
        .iter()
        .enumerate()
        .map(|(i, column)| format!("{column} = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "UPDATE usage_events SET {assignments} WHERE id = ?{}",
        EVENT_COLUMNS.len() + 1
    )
});

fn insert_event(
    statement: &mut Statement<'_>,
    dedup_key: &str,
    values: Vec<Value>,
) -> Result<i64, StoreError> {
    let all = std::iter::once(text(dedup_key)).chain(values);
    Ok(statement.insert(params_from_iter(all))?)
}

fn update_event(
    statement: &mut Statement<'_>,
    id: i64,
    values: Vec<Value>,
) -> Result<(), StoreError> {
    let all = values
        .into_iter()
        .chain(std::iter::once(Value::Integer(id)));
    statement.execute(params_from_iter(all))?;
    Ok(())
}

const UPSERT_COST_SQL: &str = "INSERT INTO event_costs (event_id, catalog_version, rule_id, \
     status, assumptions, input_nusd, cache_read_nusd, cache_write_nusd, output_nusd, \
     total_nusd, cache_read_saving_nusd, cache_write_overhead_nusd, rate_verified_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
     ON CONFLICT (event_id) DO UPDATE SET catalog_version = excluded.catalog_version, \
     rule_id = excluded.rule_id, status = excluded.status, assumptions = excluded.assumptions, \
     input_nusd = excluded.input_nusd, cache_read_nusd = excluded.cache_read_nusd, \
     cache_write_nusd = excluded.cache_write_nusd, output_nusd = excluded.output_nusd, \
     total_nusd = excluded.total_nusd, cache_read_saving_nusd = excluded.cache_read_saving_nusd, \
     cache_write_overhead_nusd = excluded.cache_write_overhead_nusd, \
     rate_verified_at = excluded.rate_verified_at";

/// Writes an event's cost (re-pricing uses this outside the per-event loop).
pub(crate) fn upsert_cost(
    transaction: &Transaction<'_>,
    event_id: i64,
    cost: &EventCost,
) -> Result<(), StoreError> {
    let mut statement = transaction.prepare_cached(UPSERT_COST_SQL)?;
    bind_cost(&mut statement, event_id, cost)
}

fn bind_cost(
    statement: &mut Statement<'_>,
    event_id: i64,
    cost: &EventCost,
) -> Result<(), StoreError> {
    let b = &cost.breakdown;
    statement.execute(params![
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
    /// The project usage counts toward (the one it is merged into, if any).
    project: Option<i64>,
    /// The project the event was recorded in.
    source_project: Option<i64>,
    device: Option<i64>,
}

/// Natural key → id, per table, for the duration of one batch.
#[derive(Debug, Default)]
struct KeyCache {
    ids: HashMap<&'static str, HashMap<String, i64>>,
    /// Session links already applied in this batch: (session, parent, is subagent).
    linked: HashSet<(i64, Option<i64>, bool)>,
    /// Project id → the project its usage counts toward.
    effective_projects: HashMap<i64, i64>,
}

impl KeyCache {
    fn resolve(
        &mut self,
        transaction: &Transaction<'_>,
        table: &'static str,
        key: &str,
    ) -> Result<i64, StoreError> {
        if let Some(id) = self.ids.get(table).and_then(|ids| ids.get(key)) {
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
        self.ids
            .entry(table)
            .or_default()
            .insert(key.to_owned(), id);
        Ok(id)
    }

    /// The project `id`'s usage counts toward: the one it is merged into, or itself.
    fn effective_project(
        &mut self,
        transaction: &Transaction<'_>,
        id: i64,
    ) -> Result<i64, StoreError> {
        if let Some(effective) = self.effective_projects.get(&id) {
            return Ok(*effective);
        }
        let effective: i64 = transaction
            .prepare_cached("SELECT COALESCE(merged_into, id) FROM projects WHERE id = ?1")?
            .query_row([id], |row| row.get(0))?;
        self.effective_projects.insert(id, effective);
        Ok(effective)
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
        // Once per session per batch: repeating the same update would change nothing.
        if (parent.is_some() || context.is_subagent)
            && keys.linked.insert((session, parent, context.is_subagent))
        {
            transaction
                .prepare_cached(
                    "UPDATE sessions SET parent_session_id = COALESCE(parent_session_id, ?2), \
                     is_subagent = MAX(is_subagent, ?3) WHERE id = ?1",
                )?
                .execute(params![session, parent, i64::from(context.is_subagent)])?;
        }
    }
    let source_project = context
        .project_key
        .as_deref()
        .map(|key| keys.resolve(transaction, "projects", key))
        .transpose()?;
    Ok(ContextIds {
        session,
        project: source_project
            .map(|id| keys.effective_project(transaction, id))
            .transpose()?,
        source_project,
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
        Value::Integer(priced.precedence),
        opt_int(ids.source_project),
    ])
}
