//! Reads stored events back into domain [`UsageEvent`]s (used for re-pricing).

use rusqlite::Row;
use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{
    Attribution, DedupKey, EventContext, Granularity, ServiceTier, SourceRef, UsageEvent,
};
use veyra_domain::identity::{AppId, ConnectorId, GatewayId, Interface, ModelId, ProviderId};
use veyra_domain::money::NanoUsd;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;

use crate::StoreError;
use crate::sql::count_from;

/// Selects every column [`row_to_event`] needs; `e.id` is column 0.
pub const EVENT_SELECT: &str = "SELECT e.id, e.dedup_key, e.occurred_at, e.connector_id, \
     e.parser_version, e.tool_version, s.key, ps.key, e.is_subagent, p.key, d.key, \
     e.provider_id, e.model_id, e.raw_model, e.app_id, e.interface, e.gateway_id, \
     e.granularity, e.service_tier, e.inference_geo_us, e.request_count, e.input_uncached, \
     e.cache_read, e.cache_write_short, e.cache_write_long, e.cache_write_unspecified, \
     e.output_total, e.reasoning, e.tool_tokens, e.reported_cost_nusd, e.acc_tokens, \
     e.acc_cache, e.acc_reasoning, e.acc_requests, e.acc_cost \
     FROM usage_events e \
     LEFT JOIN sessions s ON s.id = e.session_id \
     LEFT JOIN sessions ps ON ps.id = s.parent_session_id \
     LEFT JOIN projects p ON p.id = e.project_id \
     LEFT JOIN devices d ON d.id = e.device_id";

fn corrupt(what: impl std::fmt::Display) -> StoreError {
    StoreError::Corrupt(what.to_string())
}

fn accuracy(row: &Row<'_>, index: usize) -> Result<Accuracy, StoreError> {
    let value: String = row.get(index)?;
    Accuracy::parse(&value).ok_or_else(|| corrupt(format!("accuracy `{value}`")))
}

fn opt_id<T>(
    row: &Row<'_>,
    index: usize,
    make: impl Fn(String) -> Result<T, veyra_domain::identity::IdError>,
) -> Result<Option<T>, StoreError> {
    row.get::<_, Option<String>>(index)?
        .map(|v| make(v).map_err(corrupt))
        .transpose()
}

/// Maps a row selected with [`EVENT_SELECT`] to `(id, event)`.
pub fn row_to_event(row: &Row<'_>) -> Result<(i64, UsageEvent), StoreError> {
    let count = |index: usize| -> Result<Option<u64>, StoreError> {
        count_from(row.get::<_, Option<i64>>(index)?, "token count")
    };
    let interface: String = row.get(15)?;
    let granularity: String = row.get(17)?;
    let tier: String = row.get(18)?;
    let requests = count(20)?
        .map(|n| u32::try_from(n).map_err(|_| corrupt("request count")))
        .transpose()?;

    let event = UsageEvent {
        dedup_key: DedupKey::new(row.get::<_, String>(1)?).map_err(corrupt)?,
        occurred_at: UtcMillis(row.get(2)?),
        source: SourceRef {
            connector: ConnectorId::new(row.get::<_, String>(3)?).map_err(corrupt)?,
            parser_version: row.get(4)?,
            tool_version: row.get(5)?,
        },
        context: EventContext {
            session_key: row.get(6)?,
            parent_session_key: row.get(7)?,
            is_subagent: row.get::<_, i64>(8)? != 0,
            project_key: row.get(9)?,
            device_key: row.get(10)?,
        },
        attribution: Attribution {
            provider: opt_id(row, 11, ProviderId::new)?,
            model: opt_id(row, 12, ModelId::new)?,
            raw_model: row.get(13)?,
            app: AppId::new(row.get::<_, String>(14)?).map_err(corrupt)?,
            interface: Interface::parse(&interface)
                .ok_or_else(|| corrupt(format!("interface `{interface}`")))?,
            gateway: opt_id(row, 16, GatewayId::new)?,
        },
        granularity: Granularity::parse(&granularity)
            .ok_or_else(|| corrupt(format!("granularity `{granularity}`")))?,
        service_tier: ServiceTier::parse(&tier)
            .ok_or_else(|| corrupt(format!("service tier `{tier}`")))?,
        inference_geo_us: row.get::<_, i64>(19)? != 0,
        requests,
        tokens: TokenCounts {
            input_uncached: count(21)?,
            cache_read: count(22)?,
            cache_write_short: count(23)?,
            cache_write_long: count(24)?,
            cache_write_unspecified: count(25)?,
            output_total: count(26)?,
            reasoning: count(27)?,
            tool_tokens: count(28)?,
        },
        reported_cost: row.get::<_, Option<i64>>(29)?.map(NanoUsd),
        accuracy: AccuracyMap {
            tokens: accuracy(row, 30)?,
            cache: accuracy(row, 31)?,
            reasoning: accuracy(row, 32)?,
            requests: accuracy(row, 33)?,
            cost: accuracy(row, 34)?,
        },
    };
    Ok((row.get(0)?, event))
}
