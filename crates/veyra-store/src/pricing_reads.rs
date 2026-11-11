//! Pricing reads for the cost assumptions panel: how a period's events were priced,
//! grouped by provider, model, rule, outcome and assumptions.

use rusqlite::{Connection, Row, params_from_iter};
use veyra_analytics::port::{PricingReader, PricingUse, RangeQuery, ReadError, Tally};
use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{Assumptions, CostStatus};

use crate::session_reads::range_clauses;
use crate::sql::{EVENT_TOTAL_TOKENS, count_from};
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

fn pricing_uses(
    connection: &Connection,
    query: &RangeQuery,
) -> Result<Vec<PricingUse>, StoreError> {
    let mut values = Vec::new();
    let clauses = range_clauses(query.from, query.until, &query.filters, &mut values);
    let mut statement = connection.prepare(&format!(
        "SELECT e.provider_id, COALESCE(e.model_id, e.raw_model), c.rule_id, c.status, \
         c.assumptions, COUNT(*), COALESCE(SUM({EVENT_TOTAL_TOKENS}), 0), \
         COALESCE(SUM(c.total_nusd), 0) \
         FROM usage_events e JOIN event_costs c ON c.event_id = e.id \
         WHERE {} GROUP BY 1, 2, 3, 4, 5",
        clauses.join(" AND ")
    ))?;
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
