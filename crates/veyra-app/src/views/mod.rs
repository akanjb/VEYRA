//! View builders: turn analytics results into DTOs. They contain no formulas of their
//! own; every metric comes from the analytics crate.

pub(crate) mod activity;
pub(crate) mod billing;
pub(crate) mod cache;
pub(crate) mod cost;
mod highlights;
mod kpis;
pub(crate) mod models;
pub(crate) mod overview;
pub(crate) mod pricing;
pub(crate) mod projects;
mod rankings;
pub(crate) mod sessions;
pub(crate) mod sources;
mod time_grids;
pub(crate) mod usage;

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use veyra_analytics::activity::{Activity, DEFAULT_IDLE_THRESHOLD_MS, activity};
use veyra_analytics::engine::{QueryResult, evaluate};
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::{Period, QuerySpec};
use veyra_analytics::sums::BaseSums;
use veyra_domain::accuracy::Accuracy;
use veyra_domain::time::UtcMillis;

use crate::DataServices;
use crate::dto::{AccuracyClass, Measures};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Evaluates an analytics query.
pub(crate) fn evaluated(data: &DataServices, spec: &QuerySpec) -> Result<QueryResult, ApiError> {
    evaluate(data.analytics.as_ref(), spec).map_err(|e| internal(&e))
}

/// Sessions and active time of a scope's events in `period`.
pub(crate) fn activity_of(
    data: &DataServices,
    scope: &Resolved,
    period: Period,
) -> Result<Activity, ApiError> {
    let times = data
        .analytics
        .event_times(&scope.range(period))
        .map_err(|e| internal(&e))?;
    Ok(activity(&times, DEFAULT_IDLE_THRESHOLD_MS))
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
