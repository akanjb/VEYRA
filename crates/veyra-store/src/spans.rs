//! Session spans in the store (ADR-030). An SQL aggregate turns a session's request times
//! within a bucket into its active intervals, using the analytics engine's own definition
//! ([`span_of`]). The intervals are stored as text in `rollup_15m_sessions.active`.

use rusqlite::Connection;
use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use veyra_analytics::activity::{DEFAULT_IDLE_THRESHOLD_MS, span_of};

use crate::StoreError;

/// SQL name of the aggregate: `veyra_active_intervals(occurred_at)`.
pub const ACTIVE_INTERVALS_FUNCTION: &str = "veyra_active_intervals";

/// Collects request times, then encodes their active intervals.
struct ActiveIntervals;

impl Aggregate<Vec<i64>, String> for ActiveIntervals {
    fn init(&self, _: &mut Context<'_>) -> rusqlite::Result<Vec<i64>> {
        Ok(Vec::new())
    }

    fn step(&self, context: &mut Context<'_>, times: &mut Vec<i64>) -> rusqlite::Result<()> {
        times.push(context.get(0)?);
        Ok(())
    }

    fn finalize(&self, _: &mut Context<'_>, times: Option<Vec<i64>>) -> rusqlite::Result<String> {
        let mut times = times.unwrap_or_default();
        times.sort_unstable();
        let active = span_of(0, &times, DEFAULT_IDLE_THRESHOLD_MS)
            .map(|span| span.active)
            .unwrap_or_default();
        Ok(encode(&active))
    }
}

/// Registers the aggregate on a connection that rebuilds rollups.
pub(crate) fn register(connection: &Connection) -> rusqlite::Result<()> {
    connection.create_aggregate_function(
        ACTIVE_INTERVALS_FUNCTION,
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        ActiveIntervals,
    )
}

/// `start:end` pairs separated by commas; empty when there are none.
pub(crate) fn encode(intervals: &[(i64, i64)]) -> String {
    intervals
        .iter()
        .map(|(start, end)| format!("{start}:{end}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The intervals of [`encode`]'s text.
pub(crate) fn decode(text: &str) -> Result<Vec<(i64, i64)>, StoreError> {
    let corrupt = || StoreError::Corrupt(format!("active intervals `{text}`"));
    text.split(',')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (start, end) = pair.split_once(':').ok_or_else(corrupt)?;
            Ok((
                start.parse().map_err(|_| corrupt())?,
                end.parse().map_err(|_| corrupt())?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{decode, encode, register};

    #[test]
    fn intervals_round_trip_through_text() {
        let intervals = vec![(-5, 0), (10, 20)];
        assert_eq!(encode(&intervals), "-5:0,10:20");
        assert_eq!(decode(&encode(&intervals)).unwrap(), intervals);
        assert_eq!(decode("").unwrap(), vec![]);
        assert!(decode("1-2").is_err());
    }

    #[test]
    fn the_aggregate_uses_the_engine_definition_in_any_row_order() {
        let connection = Connection::open_in_memory().unwrap();
        register(&connection).unwrap();
        // 0, 60 s and 1 000 s: 0 → 60 s, then 60 s → 360 s (a gap capped at five minutes).
        let text: String = connection
            .query_row(
                "SELECT veyra_active_intervals(t) FROM \
                 (SELECT 1000000 AS t UNION ALL SELECT 0 UNION ALL SELECT 60000)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(text, "0:360000");
    }
}
