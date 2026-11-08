//! Engine tests against an in-memory reader.

use jiff::civil::date;
use jiff::tz::TimeZone;
use veyra_domain::time::UtcMillis;

use super::evaluate;
use crate::dimension::{Dimension, Filters};
use crate::port::{BaseSumsQuery, BaseSumsReader, BaseSumsRow, ReadError};
use crate::query::{Period, QuerySpec, TimeGrain};
use crate::sums::BaseSums;

const HOUR: i64 = 3_600_000;
/// 2026-11-08T00:00:00Z
const NOV_8: i64 = 1_794_096_000_000;

/// Serves fixed bucket rows, honouring the period and the provider dimension only.
struct FakeReader {
    rows: Vec<(i64, &'static str, u64)>, // (bucket UTC ms, provider, output tokens)
}

impl BaseSumsReader for FakeReader {
    fn read_base_sums(&self, q: &BaseSumsQuery) -> Result<Vec<BaseSumsRow>, ReadError> {
        Ok(self
            .rows
            .iter()
            .filter(|(at, _, _)| *at >= q.from.0 && *at < q.until.0)
            .map(|(at, provider, tokens)| {
                let mut sums = BaseSums {
                    events: 1,
                    ..BaseSums::default()
                };
                sums.tokens.output_total = Some(*tokens);
                BaseSumsRow {
                    bucket_start: q.by_bucket.then_some(UtcMillis(*at)),
                    key: q
                        .group_by
                        .iter()
                        .map(|_| Some((*provider).to_owned()))
                        .collect(),
                    sums,
                }
            })
            .collect())
    }
}

fn kolkata() -> TimeZone {
    TimeZone::get("Asia/Kolkata").unwrap()
}

fn spec(period: Period, grain: TimeGrain, group_by: Vec<Dimension>) -> QuerySpec {
    QuerySpec {
        period,
        comparison: None,
        filters: Filters::new(),
        group_by,
        grain,
        time_zone: kolkata(),
    }
}

#[test]
fn buckets_map_to_local_days_and_gaps_are_filled() {
    // Local 2026-11-08 (IST) = [2026-11-07T18:30Z, 2026-11-08T18:30Z).
    let reader = FakeReader {
        rows: vec![
            (NOV_8 - 5 * HOUR - HOUR / 2, "anthropic", 10), // 2026-11-08 00:00 IST
            (NOV_8 + 18 * HOUR, "openai", 20),              // 2026-11-08 23:30 IST
            (NOV_8 + 18 * HOUR + HOUR / 2, "openai", 40),   // 2026-11-09 00:00 IST
        ],
    };
    let period = Period::local_days(date(2026, 11, 8), 3, &kolkata()).unwrap();
    let result = evaluate(&reader, &spec(period, TimeGrain::Day, vec![])).unwrap();

    let daily: Vec<Option<u64>> = result
        .series
        .iter()
        .map(|p| p.sums.tokens.output_total)
        .collect();
    assert_eq!(
        daily,
        [Some(30), Some(40), None],
        "third day is an empty, filled gap"
    );
    assert_eq!(result.totals.tokens.output_total, Some(70));
    assert_eq!(result.series[0].start, period.from());
}

#[test]
fn groups_carry_their_own_series_and_comparison_totals() {
    let reader = FakeReader {
        rows: vec![
            (NOV_8, "anthropic", 10),
            (NOV_8 + HOUR, "openai", 5),
            (NOV_8 - 24 * HOUR, "anthropic", 7), // previous day
        ],
    };
    let period = Period::new(UtcMillis(NOV_8), UtcMillis(NOV_8 + 24 * HOUR)).unwrap();
    let mut query = spec(period, TimeGrain::Hour, vec![Dimension::Provider]);
    query.time_zone = TimeZone::UTC;
    query.comparison = Some(period.previous());
    let result = evaluate(&reader, &query).unwrap();

    assert_eq!(result.groups.len(), 2);
    let anthropic = &result.groups[0];
    assert_eq!(anthropic.key, vec![Some("anthropic".to_owned())]);
    assert_eq!(anthropic.totals.tokens.output_total, Some(10));
    assert_eq!(
        anthropic.previous.and_then(|p| p.tokens.output_total),
        Some(7)
    );
    assert_eq!(anthropic.series.len(), 24);
    let openai = &result.groups[1];
    assert_eq!(
        openai.previous.map(|p| p.events),
        Some(0),
        "absent before → zero"
    );
    assert_eq!(result.previous_totals.map(|p| p.events), Some(1));
}

#[test]
fn weeks_start_on_monday_and_months_on_the_first() {
    let reader = FakeReader { rows: vec![] };
    let tz = kolkata();
    // 2026-11-08 is a Sunday; its ISO week starts Monday 2026-11-02.
    let period = Period::local_days(date(2026, 11, 8), 2, &tz).unwrap();
    let weekly = evaluate(&reader, &spec(period, TimeGrain::Week, vec![])).unwrap();
    let monday = Period::local_days(date(2026, 11, 2), 1, &tz)
        .unwrap()
        .from();
    let next_monday = Period::local_days(date(2026, 11, 9), 1, &tz)
        .unwrap()
        .from();
    let starts: Vec<_> = weekly.series.iter().map(|p| p.start).collect();
    assert_eq!(starts, [monday, next_monday]);

    let monthly = evaluate(&reader, &spec(period, TimeGrain::Month, vec![])).unwrap();
    let first = Period::local_days(date(2026, 11, 1), 1, &tz)
        .unwrap()
        .from();
    assert_eq!(
        monthly.series.iter().map(|p| p.start).collect::<Vec<_>>(),
        [first]
    );
}

#[test]
fn total_grain_has_no_series() {
    let reader = FakeReader {
        rows: vec![(NOV_8, "anthropic", 3)],
    };
    let period = Period::new(UtcMillis(NOV_8), UtcMillis(NOV_8 + HOUR)).unwrap();
    let result = evaluate(&reader, &spec(period, TimeGrain::Total, vec![])).unwrap();
    assert_eq!(result.series, Vec::new());
    assert_eq!(result.totals.tokens.output_total, Some(3));
}
