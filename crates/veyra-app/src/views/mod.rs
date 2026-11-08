//! View builders: turn analytics results into DTOs. They contain no formulas of their
//! own; every metric comes from the analytics crate.

pub(crate) mod overview;
pub(crate) mod sessions;
pub(crate) mod sources;

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::sums::BaseSums;
use veyra_domain::accuracy::Accuracy;
use veyra_domain::time::UtcMillis;

use crate::dto::{AccuracyClass, Measures};
use crate::error::ApiError;

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
