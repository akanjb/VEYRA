//! Evaluates a [`QuerySpec`] against a [`BaseSumsReader`]: groups rows, maps 15-minute
//! UTC buckets to local periods, fills gaps and attaches comparison totals.

use std::collections::BTreeMap;

use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use veyra_domain::time::UtcMillis;

use crate::dimension::DimensionValue;
use crate::port::{BaseSumsQuery, BaseSumsReader, ReadError};
use crate::query::{Period, QueryError, QuerySpec, TimeGrain, millis};
use crate::sums::BaseSums;

/// Error raised while evaluating a query.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error("time conversion failed: {0}")]
    Time(#[from] jiff::Error),
}

/// Sums for one local period, identified by the UTC instant it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesPoint {
    pub start: UtcMillis,
    pub sums: BaseSums,
}

/// Results for one dimension combination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupResult {
    pub key: Vec<DimensionValue>,
    pub totals: BaseSums,
    /// Totals in the comparison period, when one was requested.
    pub previous: Option<BaseSums>,
    /// One point per local period (empty for [`TimeGrain::Total`]), gaps filled.
    pub series: Vec<SeriesPoint>,
}

/// The result of a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub totals: BaseSums,
    pub previous_totals: Option<BaseSums>,
    pub series: Vec<SeriesPoint>,
    /// Sorted by key; empty when nothing is grouped.
    pub groups: Vec<GroupResult>,
}

/// Evaluates a query.
///
/// # Errors
/// Returns [`EngineError`] for invalid specifications, read failures or time errors.
pub fn evaluate(reader: &dyn BaseSumsReader, spec: &QuerySpec) -> Result<QueryResult, EngineError> {
    spec.validate()?;
    let rows = reader.read_base_sums(&BaseSumsQuery {
        from: spec.period.from(),
        until: spec.period.until(),
        filters: spec.filters.clone(),
        group_by: spec.group_by.clone(),
        by_bucket: spec.grain != TimeGrain::Total,
    })?;

    let starts = period_starts(&spec.period, spec.grain, &spec.time_zone)?;
    let mut overall_series: BTreeMap<i64, BaseSums> = BTreeMap::new();
    let mut groups: BTreeMap<Vec<DimensionValue>, (BaseSums, BTreeMap<i64, BaseSums>)> =
        BTreeMap::new();
    let mut totals = BaseSums::default();

    for row in &rows {
        totals.merge(&row.sums);
        let group = groups.entry(row.key.clone()).or_default();
        group.0.merge(&row.sums);
        if let Some(bucket) = row.bucket_start {
            let start = containing_period(&starts, bucket);
            group.1.entry(start).or_default().merge(&row.sums);
            overall_series.entry(start).or_default().merge(&row.sums);
        }
    }

    let previous = spec
        .comparison
        .map(|period| previous_totals(reader, spec, &period))
        .transpose()?;

    Ok(QueryResult {
        totals,
        previous_totals: previous.as_ref().map(|p| p.0),
        series: fill(&starts, &overall_series),
        groups: groups
            .into_iter()
            .map(|(key, (totals, series))| GroupResult {
                previous: previous
                    .as_ref()
                    .map(|p| p.1.get(&key).copied().unwrap_or_default()),
                series: fill(&starts, &series),
                key,
                totals,
            })
            .collect(),
    })
}

type PreviousTotals = (BaseSums, BTreeMap<Vec<DimensionValue>, BaseSums>);

fn previous_totals(
    reader: &dyn BaseSumsReader,
    spec: &QuerySpec,
    period: &Period,
) -> Result<PreviousTotals, EngineError> {
    let rows = reader.read_base_sums(&BaseSumsQuery {
        from: period.from(),
        until: period.until(),
        filters: spec.filters.clone(),
        group_by: spec.group_by.clone(),
        by_bucket: false,
    })?;
    let mut by_key: BTreeMap<Vec<DimensionValue>, BaseSums> = BTreeMap::new();
    for row in &rows {
        by_key.entry(row.key.clone()).or_default().merge(&row.sums);
    }
    Ok((BaseSums::total(rows.iter().map(|r| &r.sums)), by_key))
}

/// The start of the local period containing `bucket`, found by binary search over the
/// period boundaries. Exact because every boundary is a 15-minute bucket boundary.
fn containing_period(starts: &[UtcMillis], bucket: UtcMillis) -> i64 {
    period_index(starts, bucket).map_or(bucket.0, |index| starts[index].0)
}

/// The index of the local period (in ascending `starts`) that contains `at`; `None`
/// before the first period.
#[must_use]
pub fn period_index(starts: &[UtcMillis], at: UtcMillis) -> Option<usize> {
    starts.partition_point(|start| *start <= at).checked_sub(1)
}

fn fill(starts: &[UtcMillis], values: &BTreeMap<i64, BaseSums>) -> Vec<SeriesPoint> {
    starts
        .iter()
        .map(|start| SeriesPoint {
            start: *start,
            sums: values.get(&start.0).copied().unwrap_or_default(),
        })
        .collect()
}

fn zoned(at: UtcMillis, tz: &TimeZone) -> Result<Zoned, jiff::Error> {
    Ok(Timestamp::from_millisecond(at.0)?.to_zoned(tz.clone()))
}

/// The start of the local period containing `at`.
fn period_start(at: UtcMillis, grain: TimeGrain, tz: &TimeZone) -> Result<UtcMillis, jiff::Error> {
    let local = zoned(at, tz)?;
    let start = match grain {
        TimeGrain::Total => return Ok(at),
        TimeGrain::Hour => local
            .with()
            .minute(0)
            .second(0)
            .subsec_nanosecond(0)
            .build()?,
        TimeGrain::Day => local.start_of_day()?,
        TimeGrain::Week => {
            let offset = i64::from(local.weekday().to_monday_zero_offset());
            local
                .date()
                .checked_sub(offset.days())?
                .to_zoned(tz.clone())?
        }
        TimeGrain::Month => local.date().first_of_month().to_zoned(tz.clone())?,
    };
    Ok(millis(start.timestamp()))
}

/// Every local period start overlapping the query period, in order (empty for
/// [`TimeGrain::Total`]).
///
/// # Errors
/// Returns [`jiff::Error`] for instants or dates outside the supported range.
pub fn period_starts(
    period: &Period,
    grain: TimeGrain,
    tz: &TimeZone,
) -> Result<Vec<UtcMillis>, jiff::Error> {
    if grain == TimeGrain::Total {
        return Ok(Vec::new());
    }
    let mut starts = Vec::new();
    let mut current = period_start(period.from(), grain, tz)?;
    while current < period.until() {
        starts.push(current);
        let local = zoned(current, tz)?;
        let next = match grain {
            TimeGrain::Hour => local.checked_add(1.hour())?,
            TimeGrain::Day => local.date().tomorrow()?.to_zoned(tz.clone())?,
            TimeGrain::Week => local.date().checked_add(1.week())?.to_zoned(tz.clone())?,
            TimeGrain::Month | TimeGrain::Total => {
                local.date().checked_add(1.month())?.to_zoned(tz.clone())?
            }
        };
        current = millis(next.timestamp());
    }
    Ok(starts)
}

#[cfg(test)]
mod tests;
