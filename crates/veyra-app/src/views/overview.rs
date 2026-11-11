//! The Overview page: every number comes from the analytics engine (ADR-008).

use std::collections::BTreeMap;

use jiff::ToSpan;
use veyra_analytics::activity::{ACTIVE_TIME_DEFINITION, SESSIONS_DEFINITION};
use veyra_analytics::composition::AccuracyComposition;
use veyra_analytics::dimension::Dimension;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_analytics::sums::BaseSums;
use veyra_domain::accuracy::Accuracy;

use super::kpis::{kpi, metric_kpi, token_accuracy};
use super::rankings::{Labeller, dimension_ranking, groups_of};
use super::{accuracy_class, activity_of, evaluated, internal, local_date, local_time, measures};
use crate::DataServices;
use crate::dto::{
    AccuracyClass, CalendarDay, Composition, DailyPoint, HourCell, Labelled, Overview,
    OverviewKpis, RankRow,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Models shown before "Other".
const TOP_MODELS: usize = 8;
/// Projects shown before "Other".
const TOP_PROJECTS: usize = 30;
/// Days in the activity calendar.
const CALENDAR_DAYS: i32 = 365;

pub(crate) fn composition(sums: &BaseSums) -> Composition {
    let mix = AccuracyComposition::of(sums);
    let share = |class| mix.share(class).unwrap_or(0.0);
    Composition {
        exact: share(Accuracy::Exact),
        provider_reported: share(Accuracy::ProviderReported),
        partial: share(Accuracy::Partial),
        estimated: share(Accuracy::Estimated),
        unsupported: share(Accuracy::Unsupported),
        weakest: mix.weakest().map(accuracy_class),
        pricing_coverage: MetricId::PricingCoverage
            .compute(sums)
            .map(MetricValue::as_f64),
    }
}

/// Headline tiles with comparisons. Sessions and active time come from event times.
fn kpis(
    data: &DataServices,
    scope: &Resolved,
    current: &BaseSums,
    previous: Option<&BaseSums>,
) -> Result<OverviewKpis, ApiError> {
    let accuracy = token_accuracy(current);
    let count = |n: u64| Some(MetricValue::Count(n));
    let now = activity_of(data, scope, scope.period)?;
    let before = scope
        .previous
        .map(|period| activity_of(data, scope, period))
        .transpose()?;
    let millis = |ms: i64| count(u64::try_from(ms).unwrap_or(0));
    Ok(OverviewKpis {
        tokens: metric_kpi(MetricId::TotalTokens, current, previous, accuracy),
        cost_nano_usd: metric_kpi(
            MetricId::ApiEquivalentCost,
            current,
            previous,
            Some(AccuracyClass::Estimated),
        ),
        requests: metric_kpi(MetricId::Requests, current, previous, accuracy),
        sessions: kpi(
            count(now.sessions),
            before.and_then(|b| count(b.sessions)),
            accuracy,
            SESSIONS_DEFINITION,
        ),
        active_ms: kpi(
            millis(now.active_ms),
            before.and_then(|b| millis(b.active_ms)),
            None,
            ACTIVE_TIME_DEFINITION,
        ),
        cache_hit_ratio: metric_kpi(MetricId::CacheHitRatio, current, previous, accuracy),
    })
}

/// The daily trend by application, largest application first.
fn daily(
    data: &DataServices,
    scope: &Resolved,
) -> Result<(Vec<Labelled>, Vec<DailyPoint>), ApiError> {
    let result = evaluated(
        data,
        &scope.spec(vec![Dimension::App], TimeGrain::Day, false),
    )?;
    let mut apps = result.groups;
    apps.sort_by(|a, b| {
        measures(&b.totals)
            .tokens
            .cmp(&measures(&a.totals).tokens)
            .then_with(|| a.key.cmp(&b.key))
    });
    let by_start: Vec<BTreeMap<i64, BaseSums>> = apps
        .iter()
        .map(|group| group.series.iter().map(|p| (p.start.0, p.sums)).collect())
        .collect();
    let points = result
        .series
        .iter()
        .map(|point| {
            Ok(DailyPoint {
                date: local_date(point.start, &scope.time_zone)?,
                values: by_start
                    .iter()
                    .map(|series| series.get(&point.start.0).map(measures).unwrap_or_default())
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let labelled = apps
        .iter()
        .map(|g| {
            data.names.label_app(
                g.key
                    .first()
                    .cloned()
                    .flatten()
                    .unwrap_or_else(|| "unknown".to_owned()),
            )
        })
        .collect();
    Ok((labelled, points))
}

/// The providers with usage in the scope, by display name (the provider filter's choices).
fn providers(data: &DataServices, scope: &Resolved) -> Result<Vec<Labelled>, ApiError> {
    let result = evaluated(
        data,
        &scope.spec(vec![Dimension::Provider], TimeGrain::Total, false),
    )?;
    let mut providers: Vec<Labelled> = result
        .groups
        .into_iter()
        .filter_map(|group| group.key.into_iter().next().flatten())
        .map(|key| data.names.label_provider(key))
        .collect();
    providers.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(providers)
}

/// The top groups of one dimension by tokens, plus "Other".
fn top(
    data: &DataServices,
    scope: &Resolved,
    dimension: Dimension,
    n: usize,
    scope_tokens: u64,
) -> Result<Vec<RankRow>, ApiError> {
    let labeller = Labeller::new(data, dimension)?;
    let groups = groups_of(data, scope, &labeller)?;
    Ok(dimension_ranking(data, groups, &labeller, n, scope_tokens))
}

/// The activity calendar: the year ending on the last day of the scope.
fn calendar(data: &DataServices, scope: &Resolved) -> Result<Vec<CalendarDay>, ApiError> {
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
    evaluated(data, &spec)?
        .series
        .iter()
        .map(|point| {
            Ok(CalendarDay {
                date: local_date(point.start, &scope.time_zone)?,
                tokens: measures(&point.sums).tokens,
            })
        })
        .collect()
}

/// Tokens per local weekday and hour over the period of the scope.
fn hours(data: &DataServices, scope: &Resolved) -> Result<Vec<HourCell>, ApiError> {
    let mut grid = [[0_u64; 24]; 7];
    for point in evaluated(data, &scope.spec(vec![], TimeGrain::Hour, false))?.series {
        let local = local_time(point.start, &scope.time_zone)?;
        let weekday = usize::try_from(local.weekday().to_monday_zero_offset()).unwrap_or(0);
        let hour = usize::try_from(local.hour()).unwrap_or(0);
        grid[weekday][hour] += measures(&point.sums).tokens;
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

/// Builds the Overview for a scope.
pub(crate) fn overview(data: &DataServices, scope: &Resolved) -> Result<Overview, ApiError> {
    let totals = evaluated(data, &scope.spec(vec![], TimeGrain::Total, true))?;
    let current = &totals.totals;
    let scope_tokens = measures(current).tokens;
    let (apps, daily_points) = daily(data, scope)?;
    Ok(Overview {
        kpis: kpis(data, scope, current, totals.previous_totals.as_ref())?,
        composition: composition(current),
        apps,
        providers: providers(data, scope)?,
        daily: daily_points,
        models: top(data, scope, Dimension::Model, TOP_MODELS, scope_tokens)?,
        projects: top(data, scope, Dimension::Project, TOP_PROJECTS, scope_tokens)?,
        calendar: calendar(data, scope)?,
        hours: hours(data, scope)?,
    })
}
