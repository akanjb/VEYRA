//! Headline numbers: a value, its comparison and its definition from the metric registry.

use veyra_analytics::composition::AccuracyComposition;
use veyra_analytics::metrics::{MetricId, MetricValue, delta};
use veyra_analytics::sums::BaseSums;

use super::accuracy_class;
use crate::dto::{AccuracyClass, Kpi};

/// A headline value with its comparison and definition.
pub(crate) fn kpi(
    current: Option<MetricValue>,
    previous: Option<MetricValue>,
    accuracy: Option<AccuracyClass>,
    definition: &str,
) -> Kpi {
    Kpi {
        value: current.map(MetricValue::as_f64),
        previous: previous.map(MetricValue::as_f64),
        change: delta(current, previous).and_then(|d| d.relative),
        accuracy,
        definition: definition.to_owned(),
    }
}

/// A registry metric over the current and (optionally) the comparison sums.
pub(crate) fn metric_kpi(
    metric: MetricId,
    current: &BaseSums,
    previous: Option<&BaseSums>,
    accuracy: Option<AccuracyClass>,
) -> Kpi {
    kpi(
        metric.compute(current),
        previous.and_then(|p| metric.compute(p)),
        accuracy,
        metric.definition().description,
    )
}

/// The weakest accuracy class among the tokens of `sums`.
pub(crate) fn token_accuracy(sums: &BaseSums) -> Option<AccuracyClass> {
    AccuracyComposition::of(sums).weakest().map(accuracy_class)
}
