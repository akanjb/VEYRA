//! Contributions to a change between two periods, for the period-change waterfall.
//!
//! Each key's change is its value now minus its value before (zero where it is absent).
//! The largest changes by size are kept and the rest merge into "Other" exactly, so the
//! previous total plus every step always equals the current total.

use std::collections::BTreeMap;

/// What the period-change waterfall means.
pub const WATERFALL_DEFINITION: &str = "How API-equivalent cost changed from the previous period of the same length: each bar is one group's increase or decrease, the largest first, and the rest are merged into Other. The steps add up from the previous total to the current one.";

/// One step of a waterfall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<K> {
    pub key: K,
    pub previous: i64,
    pub current: i64,
}

impl<K> Step<K> {
    #[must_use]
    pub fn change(&self) -> i64 {
        self.current - self.previous
    }
}

/// The changes between two periods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contributions<K> {
    pub previous_total: i64,
    pub current_total: i64,
    /// The largest changes by size, largest first.
    pub steps: Vec<Step<K>>,
    /// Every other key merged, with how many keys it holds; `None` when nothing is left.
    pub other: Option<(usize, Step<()>)>,
}

/// The `n` largest changes from `previous` to `current` and an exact remainder. Keys
/// that did not change are left out.
#[must_use]
pub fn contributions<K: Ord + Clone>(
    current: impl IntoIterator<Item = (K, i64)>,
    previous: impl IntoIterator<Item = (K, i64)>,
    n: usize,
) -> Contributions<K> {
    let mut values: BTreeMap<K, (i64, i64)> = BTreeMap::new();
    for (key, value) in previous {
        values.entry(key).or_default().0 += value;
    }
    for (key, value) in current {
        values.entry(key).or_default().1 += value;
    }
    let previous_total = values.values().map(|v| v.0).sum();
    let current_total = values.values().map(|v| v.1).sum();
    let mut steps: Vec<Step<K>> = values
        .into_iter()
        .map(|(key, (previous, current))| Step {
            key,
            previous,
            current,
        })
        .filter(|step| step.change() != 0)
        .collect();
    // Largest change by size first; ties by key, so the order is stable.
    steps.sort_by(|a, b| {
        b.change()
            .unsigned_abs()
            .cmp(&a.change().unsigned_abs())
            .then_with(|| a.key.cmp(&b.key))
    });
    let rest = if steps.len() > n {
        steps.split_off(n)
    } else {
        Vec::new()
    };
    let other = (!rest.is_empty()).then(|| {
        (
            rest.len(),
            Step {
                key: (),
                previous: rest.iter().map(|s| s.previous).sum(),
                current: rest.iter().map(|s| s.current).sum(),
            },
        )
    });
    Contributions {
        previous_total,
        current_total,
        steps,
        other,
    }
}

#[cfg(test)]
mod tests {
    use super::contributions;

    #[test]
    fn steps_add_up_from_the_previous_total_to_the_current_one() {
        let result = contributions(
            [("a", 50), ("b", 10), ("c", 7), ("d", 1)],
            [("a", 20), ("b", 30), ("e", 4), ("d", 1)],
            2,
        );
        assert_eq!(result.previous_total, 55);
        assert_eq!(result.current_total, 68);
        let keys: Vec<&str> = result.steps.iter().map(|s| s.key).collect();
        // a +30 and b −20 are the largest; c +7 and e −4 merge; d did not change.
        assert_eq!(keys, ["a", "b"]);
        let (count, other) = result.other.unwrap();
        assert_eq!(count, 2);
        assert_eq!(other.change(), 3);
        let steps: i64 = result.steps.iter().map(super::Step::change).sum();
        assert_eq!(
            result.previous_total + steps + other.change(),
            result.current_total
        );
    }

    #[test]
    fn nothing_is_merged_when_few_keys_changed() {
        let result = contributions([("a", 5)], [("a", 2), ("b", 2)], 5);
        assert_eq!(result.steps.len(), 2);
        assert!(result.other.is_none());
    }

    #[test]
    fn equal_changes_keep_a_stable_order() {
        let result = contributions([("b", 1), ("a", 1)], [], 5);
        let keys: Vec<&str> = result.steps.iter().map(|s| s.key).collect();
        assert_eq!(keys, ["a", "b"]);
    }
}
