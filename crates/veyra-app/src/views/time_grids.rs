//! Calendar and hour-of-week grids, shared by the Overview and Activity pages.

use jiff::ToSpan;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};

use super::{internal, local_date, local_time, token_points};
use crate::DataServices;
use crate::dto::{CalendarDay, HourCell};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Days in the activity calendar.
const CALENDAR_DAYS: i32 = 365;

/// The activity calendar: the year ending on the last day of the scope.
pub(crate) fn calendar(
    data: &DataServices,
    scope: &Resolved,
) -> Result<Vec<CalendarDay>, ApiError> {
    let first_day = scope
        .end
        .checked_sub((CALENDAR_DAYS - 1).days())
        .map_err(|e| internal(&e))?;
    let year =
        Period::local_days(first_day, CALENDAR_DAYS, &scope.time_zone).map_err(|e| internal(&e))?;
    let spec = QuerySpec {
        period: year,
        comparison: None,
        ..scope.spec(vec![], TimeGrain::Day, false)
    };
    token_points(data, &spec)?
        .iter()
        .map(|point| {
            Ok(CalendarDay {
                date: local_date(point.start, &scope.time_zone)?,
                tokens: point.tokens,
            })
        })
        .collect()
}

/// Tokens per local weekday and hour over the period of the scope.
pub(crate) fn hours(data: &DataServices, scope: &Resolved) -> Result<Vec<HourCell>, ApiError> {
    let mut grid = [[0_u64; 24]; 7];
    for point in token_points(data, &scope.spec(vec![], TimeGrain::Hour, false))? {
        let local = local_time(point.start, &scope.time_zone)?;
        let weekday = usize::try_from(local.weekday().to_monday_zero_offset()).unwrap_or(0);
        let hour = usize::try_from(local.hour()).unwrap_or(0);
        grid[weekday][hour] += point.tokens;
    }
    Ok(grid
        .iter()
        .enumerate()
        .flat_map(|(weekday, row)| {
            row.iter().enumerate().map(move |(hour, tokens)| HourCell {
                weekday: u8::try_from(weekday).unwrap_or(0),
                hour: u8::try_from(hour).unwrap_or(0),
                tokens: *tokens,
            })
        })
        .collect())
}
