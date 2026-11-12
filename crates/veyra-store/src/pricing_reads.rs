//! Pricing reads for the cost assumptions panel: how a period's events were priced,
//! grouped by provider, model, rule, outcome and assumptions. Read from
//! `rollup_15m_pricing` when it answers exactly (ADR-030), otherwise from the events.

use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};
use veyra_analytics::dimension::Dimension;
use veyra_analytics::port::{PricingReader, PricingUse, RangeQuery, ReadError, Tally};
use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{Assumptions, CostStatus};

use crate::reader::filter_clause;
use crate::session_reads::range_clauses;
use crate::sql::{EVENT_TOTAL_TOKENS, count_from, dimension_info};
use crate::{Store, StoreError};

/// A stored, never-negative count.
fn count(value: i64, what: &str) -> Result<u64, StoreError> {
    Ok(count_from(Some(value), what)?.unwrap_or(0))
}

fn pricing_use(row: &Row<'_>) -> Result<PricingUse, StoreError> {
    let status: String = row.get(3)?;
    Ok(PricingUse {
        provider: row.get(0)?,
        model: row.get(1)?,
        rule_id: row.get(2)?,
        status: CostStatus::parse(&status)
            .ok_or_else(|| StoreError::Corrupt(format!("unknown cost status `{status}`")))?,
        assumptions: Assumptions::from_bits(row.get(4)?),
        tally: Tally {
            events: count(row.get(5)?, "event count")?,
            tokens: count(row.get(6)?, "token count")?,
            cost: NanoUsd(row.get(7)?),
        },
    })
}

/// The grouping both sources share, in [`pricing_use`]'s column order. `alias` is the
/// table holding the provider and model columns; `sums` are the three totals.
fn uses_sql(from: &str, alias: &str, sums: &str, filter: &str) -> String {
    let model = format!("COALESCE({alias}.model_id, {alias}.raw_model)");
    format!(
        "SELECT {alias}.provider_id, {model}, rule_id, status, assumptions, {sums} \
         FROM {from} WHERE {filter} \
         GROUP BY {alias}.provider_id, {model}, rule_id, status, assumptions"
    )
}

/// Whether `rollup_15m_pricing` answers the query exactly: its bounds fall on buckets and
/// it filters on provider and model only.
fn from_rollup(query: &RangeQuery) -> bool {
    query.from.is_bucket_aligned()
        && query.until.is_bucket_aligned()
        && query
            .filters
            .iter()
            .all(|(dimension, _)| matches!(dimension, Dimension::Provider | Dimension::Model))
}

fn rollup_sql(query: &RangeQuery, values: &mut Vec<Value>) -> String {
    values.extend([Value::Integer(query.from.0), Value::Integer(query.until.0)]);
    let mut clauses = vec![
        "p.bucket_start >= ?1".to_owned(),
        "p.bucket_start < ?2".to_owned(),
    ];
    for (dimension, allowed) in query.filters.iter() {
        let (_, column, integer) = dimension_info(dimension);
        if let Some(clause) = filter_clause(&format!("p.{column}"), integer, allowed, values) {
            clauses.push(clause);
        }
    }
    uses_sql(
        "rollup_15m_pricing p",
        "p",
        "SUM(p.events), SUM(p.tokens), SUM(p.cost_total_nusd)",
        &clauses.join(" AND "),
    )
}

fn events_sql(query: &RangeQuery, values: &mut Vec<Value>) -> String {
    let clauses = range_clauses(query.from, query.until, &query.filters, values);
    uses_sql(
        "usage_events e JOIN event_costs c ON c.event_id = e.id",
        "e",
        &format!(
            "COUNT(*), COALESCE(SUM({EVENT_TOTAL_TOKENS}), 0), COALESCE(SUM(c.total_nusd), 0)"
        ),
        &clauses.join(" AND "),
    )
}

fn pricing_uses(
    connection: &Connection,
    query: &RangeQuery,
) -> Result<Vec<PricingUse>, StoreError> {
    let mut values = Vec::new();
    let sql = if from_rollup(query) {
        rollup_sql(query, &mut values)
    } else {
        events_sql(query, &mut values)
    };
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(values))?;
    let mut uses = Vec::new();
    while let Some(row) = rows.next()? {
        uses.push(pricing_use(row)?);
    }
    Ok(uses)
}

impl PricingReader for Store {
    fn pricing_uses(&self, query: &RangeQuery) -> Result<Vec<PricingUse>, ReadError> {
        self.with_reader(|connection| pricing_uses(connection, query))
            .map_err(|e| ReadError(e.to_string()))
    }
}
