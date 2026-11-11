//! The Activity page: when do I work with AI? Every number comes from the analytics
//! engine (ADR-008).

use std::collections::HashMap;
use std::thread;

use veyra_analytics::activity::{
    self as engine, ACTIVE_TIME_DEFINITION, DEFAULT_IDLE_THRESHOLD_MS, DURATION_BOUNDS_MS,
    SESSIONS_DEFINITION, SessionLength, WALL_TIME_DEFINITION,
};
use veyra_analytics::engine::period_starts;
use veyra_analytics::metrics::MetricValue;
use veyra_analytics::query::{Period, TimeGrain};
use veyra_analytics::sessions::{SessionQuery, SessionSort};
use veyra_domain::time::UtcMillis;

use super::kpis::kpi;
use super::time_grids::{calendar, hours};
use super::{internal, joined, local_date};
use crate::DataServices;
use crate::dto::{Activity, ActivityDay, ActivityKpis, LengthBin, LongSession};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Long-running sessions listed.
const LONGEST_SHOWN: u32 = 10;

/// What "active days" means.
const ACTIVE_DAYS_DEFINITION: &str = "Local days with at least one request.";

/// Event times of a period, and its activity per local day.
struct PeriodTimes {
    times: Vec<(i64, UtcMillis)>,
    days: Vec<engine::Activity>,
}

fn period_times(
    data: &DataServices,
    scope: &Resolved,
    period: Period,
) -> Result<(PeriodTimes, Vec<UtcMillis>), ApiError> {
    let times = data
        .analytics
        .event_times(&scope.range(period))
        .map_err(|e| internal(&e))?;
    let starts =
        period_starts(&period, TimeGrain::Day, &scope.time_zone).map_err(|e| internal(&e))?;
    let days =
        engine::activity_by_period(&times, DEFAULT_IDLE_THRESHOLD_MS, &starts, period.until());
    Ok((PeriodTimes { times, days }, starts))
}

const fn count(n: u64) -> MetricValue {
    MetricValue::Count(n)
}

fn millis(ms: i64) -> MetricValue {
    count(u64::try_from(ms).unwrap_or(0))
}

fn active_days(days: &[engine::Activity]) -> u64 {
    u64::try_from(days.iter().filter(|d| d.sessions > 0).count()).unwrap_or(u64::MAX)
}

fn kpis(now: &PeriodTimes, before: Option<&PeriodTimes>) -> ActivityKpis {
    let whole = engine::activity(&now.times, DEFAULT_IDLE_THRESHOLD_MS);
    let earlier = before.map(|b| (engine::activity(&b.times, DEFAULT_IDLE_THRESHOLD_MS), b));
    // Durations are derived from request times; they carry no token accuracy.
    ActivityKpis {
        active_ms: kpi(
            Some(millis(whole.active_ms)),
            earlier.map(|(a, _)| millis(a.active_ms)),
            None,
            ACTIVE_TIME_DEFINITION,
        ),
        wall_ms: kpi(
            Some(millis(whole.wall_ms)),
            earlier.map(|(a, _)| millis(a.wall_ms)),
            None,
            WALL_TIME_DEFINITION,
        ),
        sessions: kpi(
            Some(count(whole.sessions)),
            earlier.map(|(a, _)| count(a.sessions)),
            None,
            SESSIONS_DEFINITION,
        ),
        active_days: kpi(
            Some(count(active_days(&now.days))),
            earlier.map(|(_, b)| count(active_days(&b.days))),
            None,
            ACTIVE_DAYS_DEFINITION,
        ),
    }
}

/// Sessions per length range, by active and by wall time.
fn length_bins(lengths: &[SessionLength]) -> Vec<LengthBin> {
    let mut by_active = [0_u64; DURATION_BOUNDS_MS.len()];
    let mut by_wall = [0_u64; DURATION_BOUNDS_MS.len()];
    for length in lengths {
        by_active[engine::duration_bin(length.active_ms)] += 1;
        by_wall[engine::duration_bin(length.wall_ms)] += 1;
    }
    DURATION_BOUNDS_MS
        .iter()
        .enumerate()
        .map(|(i, lower)| LengthBin {
            lower_ms: *lower,
            upper_ms: DURATION_BOUNDS_MS.get(i + 1).copied(),
            by_active: by_active[i],
            by_wall: by_wall[i],
        })
        .collect()
}

/// The sessions with the longest wall time in the period.
fn longest(
    data: &DataServices,
    scope: &Resolved,
    lengths: &[SessionLength],
) -> Result<Vec<LongSession>, ApiError> {
    let by_id: HashMap<i64, &SessionLength> = lengths.iter().map(|l| (l.session, l)).collect();
    let page = data
        .analytics
        .sessions(&SessionQuery {
            from: scope.period.from(),
            until: scope.period.until(),
            filters: scope.filters.clone(),
            sort: SessionSort::Duration,
            limit: LONGEST_SHOWN,
            offset: 0,
        })
        .map_err(|e| internal(&e))?;
    Ok(page
        .sessions
        .iter()
        .map(|s| LongSession {
            id: s.id,
            app: data.names.label_app(s.app.clone()).label,
            project: s.project.clone(),
            models: data.names.label_models(&s.models),
            started_at: s.first_at.0,
            ended_at: s.last_at.0,
            active_ms: by_id.get(&s.id).map_or(0, |l| l.active_ms),
            wall_ms: s.last_at.0 - s.first_at.0,
        })
        .collect())
}

/// Builds the Activity page for a scope.
pub(crate) fn activity(data: &DataServices, scope: &Resolved) -> Result<Activity, ApiError> {
    thread::scope(|s| {
        let before = s.spawn(|| {
            scope
                .previous
                .map(|period| period_times(data, scope, period).map(|(times, _)| times))
                .transpose()
        });
        let calendar = s.spawn(|| calendar(data, scope));
        let hours = s.spawn(|| hours(data, scope));
        let (now, starts) = period_times(data, scope, scope.period)?;
        let daily = starts
            .iter()
            .zip(&now.days)
            .map(|(start, day)| {
                Ok(ActivityDay {
                    date: local_date(*start, &scope.time_zone)?,
                    sessions: day.sessions,
                    active_ms: day.active_ms,
                    wall_ms: day.wall_ms,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        let lengths = engine::session_lengths(&now.times, DEFAULT_IDLE_THRESHOLD_MS);
        let longest = longest(data, scope, &lengths)?;
        Ok(Activity {
            kpis: kpis(&now, joined(before)?.as_ref()),
            calendar: joined(calendar)?,
            hours: joined(hours)?,
            daily,
            session_lengths: length_bins(&lengths),
            longest,
            idle_threshold_ms: DEFAULT_IDLE_THRESHOLD_MS,
            active_definition: ACTIVE_TIME_DEFINITION.to_owned(),
            wall_definition: WALL_TIME_DEFINITION.to_owned(),
        })
    })
}
