//! Top-N ranking with an exact "Other" remainder, so shares always add up to 100%.

use std::cmp::Ordering;

use crate::engine::{GroupResult, SeriesPoint};
use crate::metrics::{MetricId, MetricValue};
use crate::sums::BaseSums;

/// The largest groups by a metric plus everything else merged exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    pub top: Vec<GroupResult>,
    /// All remaining groups merged; `None` when nothing remains.
    pub other: Option<OtherGroup>,
}

/// The merged remainder of a ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherGroup {
    pub group_count: usize,
    pub totals: BaseSums,
    pub previous: Option<BaseSums>,
    pub series: Vec<SeriesPoint>,
}

/// Ranks groups by `metric` (descending; groups without a value last) and keeps `n`.
#[must_use]
pub fn top_n(mut groups: Vec<GroupResult>, metric: MetricId, n: usize) -> Ranked {
    let value = |g: &GroupResult| metric.compute(&g.totals).map(MetricValue::as_f64);
    groups.sort_by(|a, b| {
        match (value(a), value(b)) {
            (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| a.key.cmp(&b.key))
    });

    if groups.len() <= n {
        return Ranked {
            top: groups,
            other: None,
        };
    }
    let rest = groups.split_off(n);
    Ranked {
        top: groups,
        other: Some(merge(&rest)),
    }
}

fn merge(groups: &[GroupResult]) -> OtherGroup {
    let totals = BaseSums::total(groups.iter().map(|g| &g.totals));
    let previous = groups
        .first()
        .and_then(|g| g.previous)
        .map(|_| BaseSums::total(groups.iter().filter_map(|g| g.previous.as_ref())));
    let series = groups.first().map_or_else(Vec::new, |first| {
        first
            .series
            .iter()
            .enumerate()
            .map(|(i, point)| SeriesPoint {
                start: point.start,
                sums: BaseSums::total(
                    groups
                        .iter()
                        .filter_map(|g| g.series.get(i).map(|p| &p.sums)),
                ),
            })
            .collect()
    });
    OtherGroup {
        group_count: groups.len(),
        totals,
        previous,
        series,
    }
}

#[cfg(test)]
mod tests {
    use super::top_n;
    use crate::engine::GroupResult;
    use crate::metrics::MetricId;
    use crate::sums::BaseSums;

    fn group(name: &str, tokens: u64) -> GroupResult {
        let mut totals = BaseSums {
            events: 1,
            ..BaseSums::default()
        };
        totals.tokens.output_total = Some(tokens);
        GroupResult {
            key: vec![Some(name.to_owned())],
            totals,
            previous: None,
            series: Vec::new(),
        }
    }

    #[test]
    fn keeps_the_largest_and_merges_the_rest_exactly() {
        let groups = vec![
            group("a", 10),
            group("b", 50),
            group("c", 30),
            group("d", 5),
        ];
        let ranked = top_n(groups, MetricId::TotalTokens, 2);
        let keys: Vec<_> = ranked
            .top
            .iter()
            .map(|g| g.key[0].clone().unwrap())
            .collect();
        assert_eq!(keys, ["b", "c"]);
        let other = ranked.other.unwrap();
        assert_eq!(other.group_count, 2);
        assert_eq!(other.totals.tokens.output_total, Some(15));
        assert_eq!(other.totals.events, 2);
    }

    #[test]
    fn no_other_group_when_everything_fits() {
        let ranked = top_n(vec![group("a", 1)], MetricId::TotalTokens, 5);
        assert_eq!(ranked.top.len(), 1);
        assert!(ranked.other.is_none());
    }
}
