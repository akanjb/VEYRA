//! The Overview page: every number comes from the analytics engine (ADR-008).

use std::collections::BTreeMap;

use jiff::ToSpan;
use veyra_analytics::activity::{Activity, DEFAULT_IDLE_THRESHOLD_MS, activity};
use veyra_analytics::composition::AccuracyComposition;
use veyra_analytics::dimension::Dimension;
use veyra_analytics::engine::{GroupResult, evaluate};
use veyra_analytics::metrics::{MetricId, MetricValue, delta};
use veyra_analytics::port::EventTimesQuery;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_analytics::sums::BaseSums;
use veyra_analytics::top_n::top_n;
use veyra_domain::accuracy::Accuracy;

use super::{accuracy_class, internal, local_time, measures};
use crate::DataServices;
use crate::dto::{
    AccuracyClass, CalendarDay, Composition, DailyPoint, HourCell, Kpi, Overview, OverviewKpis,
    RankRow,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Models shown before "Other".
const TOP_MODELS: usize = 8;
/// Projects shown before "Other".
const TOP_PROJECTS: usize = 30;
/// Days in the activity calendar.
const CALENDAR_DAYS: i32 = 365;

fn kpi(
    current: Option<MetricValue>,
    previous: Option<MetricValue>,
    accuracy: Option<AccuracyClass>,
) -> Kpi {
    Kpi {
        value: current.map(MetricValue::as_f64),
        previous: previous.map(MetricValue::as_f64),
        change: delta(current, previous).and_then(|d| d.relative),
        accuracy,
    }
}

fn metric_kpi(
    metric: MetricId,
    current: &BaseSums,
    previous: Option<&BaseSums>,
    accuracy: Option<AccuracyClass>,
) -> Kpi {
    kpi(
        metric.compute(current),
        previous.and_then(|p| metric.compute(p)),
        accuracy,
    )
}

fn activity_of(
    data: &DataServices,
    scope: &Resolved,
    period: Period,
) -> Result<Activity, ApiError> {
    let times = data
        .analytics
        .event_times(&EventTimesQuery {
            from: period.from(),
            until: period.until(),
            filters: scope.filters.clone(),
        })
        .map_err(|e| internal(&e))?;
    Ok(activity(&times, DEFAULT_IDLE_THRESHOLD_MS))
}

fn composition(sums: &BaseSums) -> Composition {
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

#[expect(clippy::cast_precision_loss, reason = "a display share")]
fn share(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// A ranking by tokens: the top `n` groups and an exact "Other".
fn ranking(
    groups: Vec<GroupResult>,
    n: usize,
    scope_tokens: u64,
    row: impl Fn(&GroupResult) -> (Option<String>, String, Option<String>),
) -> Vec<RankRow> {
    let ranked = top_n(groups, MetricId::TotalTokens, n);
    let mut rows: Vec<RankRow> = ranked
        .top
        .iter()
        .map(|group| {
            let (key, label, provider) = row(group);
            let measures = measures(&group.totals);
            RankRow {
                key,
                label,
                provider,
                measures,
                share: share(measures.tokens, scope_tokens),
                group_count: 1,
                is_other: false,
            }
        })
        .collect();
    if let Some(other) = ranked.other {
        let measures = measures(&other.totals);
        rows.push(RankRow {
            key: None,
            label: "Other".to_owned(),
            provider: None,
            measures,
            share: share(measures.tokens, scope_tokens),
            group_count: u64::try_from(other.group_count).unwrap_or(u64::MAX),
            is_other: true,
        });
    }
    rows
}

fn evaluated(
    data: &DataServices,
    spec: &QuerySpec,
) -> Result<veyra_analytics::engine::QueryResult, ApiError> {
    evaluate(data.analytics.as_ref(), spec).map_err(|e| internal(&e))
}

/// Headline tiles with comparisons. Sessions and active time come from event times.
fn kpis(
    data: &DataServices,
    scope: &Resolved,
    current: &BaseSums,
    previous: Option<&BaseSums>,
) -> Result<OverviewKpis, ApiError> {
    let token_accuracy = AccuracyComposition::of(current)
        .weakest()
        .map(accuracy_class);
    let now = activity_of(data, scope, scope.period)?;
    let before = scope
        .previous
        .map(|period| activity_of(data, scope, period))
        .transpose()?;
    let count = |n: u64| Some(MetricValue::Count(n));
    let millis = |ms: i64| Some(MetricValue::Count(u64::try_from(ms).unwrap_or(0)));
    Ok(OverviewKpis {
        tokens: metric_kpi(MetricId::TotalTokens, current, previous, token_accuracy),
        cost_nano_usd: metric_kpi(
            MetricId::ApiEquivalentCost,
            current,
            previous,
            Some(AccuracyClass::Estimated),
        ),
        requests: metric_kpi(MetricId::Requests, current, previous, token_accuracy),
        sessions: kpi(
            count(now.sessions),
            before.and_then(|b| count(b.sessions)),
            token_accuracy,
        ),
        active_ms: kpi(
            millis(now.active_ms),
            before.and_then(|b| millis(b.active_ms)),
            None,
        ),
        cache_hit_ratio: metric_kpi(MetricId::CacheHitRatio, current, previous, token_accuracy),
    })
}

/// The daily trend by application, largest application first.
fn daily(
    data: &DataServices,
    scope: &Resolved,
) -> Result<(Vec<String>, Vec<DailyPoint>), ApiError> {
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
                date: local_time(point.start, &scope.time_zone)?
                    .date()
                    .to_string(),
                values: by_start
                    .iter()
                    .map(|series| series.get(&point.start.0).map(measures).unwrap_or_default())
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let names = apps
        .iter()
        .map(|g| {
            g.key
                .first()
                .cloned()
                .flatten()
                .unwrap_or_else(|| "unknown".to_owned())
        })
        .collect();
    Ok((names, points))
}

/// The model mix, keyed by (provider, model).
fn models(
    data: &DataServices,
    scope: &Resolved,
    scope_tokens: u64,
) -> Result<Vec<RankRow>, ApiError> {
    let result = evaluated(
        data,
        &scope.spec(
            vec![Dimension::Provider, Dimension::Model],
            TimeGrain::Total,
            false,
        ),
    )?;
    Ok(ranking(result.groups, TOP_MODELS, scope_tokens, |group| {
        let provider = group.key.first().cloned().flatten();
        let model = group.key.get(1).cloned().flatten();
        let label = model.as_ref().map_or_else(
            || "Unknown model".to_owned(),
            |id| {
                data.model_names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| id.clone())
            },
        );
        (model, label, provider)
    }))
}

/// The project distribution, labelled with project paths.
fn projects(
    data: &DataServices,
    scope: &Resolved,
    scope_tokens: u64,
) -> Result<Vec<RankRow>, ApiError> {
    let result = evaluated(
        data,
        &scope.spec(vec![Dimension::Project], TimeGrain::Total, false),
    )?;
    let paths: BTreeMap<String, String> = data
        .analytics
        .projects()
        .map_err(|e| internal(&e))?
        .into_iter()
        .map(|p| (p.id.to_string(), p.path))
        .collect();
    Ok(ranking(
        result.groups,
        TOP_PROJECTS,
        scope_tokens,
        |group| {
            let id = group.key.first().cloned().flatten();
            let label = id
                .as_ref()
                .and_then(|id| paths.get(id).cloned())
                .unwrap_or_else(|| "No project".to_owned());
            (id, label, None)
        },
    ))
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
                date: local_time(point.start, &scope.time_zone)?
                    .date()
                    .to_string(),
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
        daily: daily_points,
        models: models(data, scope, scope_tokens)?,
        projects: projects(data, scope, scope_tokens)?,
        calendar: calendar(data, scope)?,
        hours: hours(data, scope)?,
    })
}
