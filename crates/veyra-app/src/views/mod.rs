//! View builders: turn analytics results into DTOs. They contain no formulas of their
//! own; every metric comes from the analytics crate.

pub(crate) mod activity;
pub(crate) mod billing;
pub(crate) mod cache;
pub(crate) mod cost;
pub(crate) mod export;
mod highlights;
mod kpis;
pub(crate) mod models;
pub(crate) mod overview;
pub(crate) mod pricing;
pub(crate) mod projects;
mod rankings;
pub(crate) mod sessions;
pub(crate) mod sources;
pub(crate) mod storage;
mod time_grids;
pub(crate) mod tools;
pub(crate) mod usage;

use std::thread::ScopedJoinHandle;

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use veyra_analytics::activity::{Activity, DEFAULT_IDLE_THRESHOLD_MS, activity};
use veyra_analytics::dimension::Dimension;
use veyra_analytics::engine::{QueryResult, TokenPoint, evaluate, token_series};
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::port::{BaseSumsQuery, CostRow, TokenRow};
use veyra_analytics::query::{Period, QuerySpec};
use veyra_analytics::sums::BaseSums;
use veyra_domain::accuracy::Accuracy;
use veyra_domain::time::UtcMillis;

use crate::DataServices;
use crate::dto::{AccuracyClass, Measures};
use crate::error::ApiError;
use crate::scope::Resolved;

/// The result of a part of a view run on its own thread inside `std::thread::scope`.
/// Independent reads then proceed at the same time, each on its own reader connection, so
/// a page costs about as much as its slowest part rather than the sum of its parts. A
/// panic in the part is passed on.
pub(crate) fn joined<T>(part: ScopedJoinHandle<'_, T>) -> T {
    part.join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// Totals per group of a scope's events in `period`.
fn totals_query(scope: &Resolved, period: Period, group_by: Vec<Dimension>) -> BaseSumsQuery {
    BaseSumsQuery {
        from: period.from(),
        until: period.until(),
        filters: scope.filters.clone(),
        group_by,
        by_bucket: false,
    }
}

/// Total tokens per group over the scope's period, for cards that show tokens only.
pub(crate) fn tokens_by(
    data: &DataServices,
    scope: &Resolved,
    group_by: Vec<Dimension>,
) -> Result<Vec<TokenRow>, ApiError> {
    data.analytics
        .read_tokens(&totals_query(scope, scope.period, group_by))
        .map_err(|e| internal(&e))
}

/// API-equivalent cost per group over `period`, for cards that show cost only.
pub(crate) fn costs_by(
    data: &DataServices,
    scope: &Resolved,
    period: Period,
    group_by: Vec<Dimension>,
) -> Result<Vec<CostRow>, ApiError> {
    data.analytics
        .read_costs(&totals_query(scope, period, group_by))
        .map_err(|e| internal(&e))
}

/// Evaluates an analytics query.
pub(crate) fn evaluated(data: &DataServices, spec: &QuerySpec) -> Result<QueryResult, ApiError> {
    evaluate(data.analytics.as_ref(), spec).map_err(|e| internal(&e))
}

/// Total tokens per local period of a query, for charts that show tokens only.
pub(crate) fn token_points(
    data: &DataServices,
    spec: &QuerySpec,
) -> Result<Vec<TokenPoint>, ApiError> {
    token_series(data.analytics.as_ref(), spec).map_err(|e| internal(&e))
}

/// Sessions and active time of a scope's events in `period`.
pub(crate) fn activity_of(
    data: &DataServices,
    scope: &Resolved,
    period: Period,
) -> Result<Activity, ApiError> {
    let spans = data
        .analytics
        .spans(&scope.range(period))
        .map_err(|e| internal(&e))?;
    Ok(activity(&spans, DEFAULT_IDLE_THRESHOLD_MS))
}

/// The local calendar date (`YYYY-MM-DD`) of an instant.
pub(crate) fn local_date(at: UtcMillis, time_zone: &TimeZone) -> Result<String, ApiError> {
    Ok(local_time(at, time_zone)?.date().to_string())
}

pub(crate) fn internal(error: &dyn std::fmt::Display) -> ApiError {
    ApiError::internal(error)
}

pub(crate) const fn accuracy_class(accuracy: Accuracy) -> AccuracyClass {
    match accuracy {
        Accuracy::Exact => AccuracyClass::Exact,
        Accuracy::ProviderReported => AccuracyClass::ProviderReported,
        Accuracy::Partial => AccuracyClass::Partial,
        Accuracy::Estimated => AccuracyClass::Estimated,
        Accuracy::Unsupported => AccuracyClass::Unsupported,
    }
}

/// Chart measures of a group: total tokens, API-equivalent cost and requests.
pub(crate) fn measures(sums: &BaseSums) -> Measures {
    let tokens = match MetricId::TotalTokens.compute(sums) {
        Some(MetricValue::Count(n)) => n,
        _ => 0,
    };
    Measures {
        tokens,
        cost_nano_usd: sums.cost.total.0,
        requests: sums.requests.unwrap_or(0),
    }
}

/// An instant in a time zone.
pub(crate) fn local_time(at: UtcMillis, time_zone: &TimeZone) -> Result<Zoned, ApiError> {
    Timestamp::from_millisecond(at.0)
        .map(|t| t.to_zoned(time_zone.clone()))
        .map_err(|e| internal(&e))
}
