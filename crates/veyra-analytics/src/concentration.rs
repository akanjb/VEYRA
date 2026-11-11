//! Shares and concentration: how usage is spread across items (PRD §7).

/// What the concentration index means.
pub const HERFINDAHL_DEFINITION: &str = "The Herfindahl index: the sum of each item's squared share. 1 means everything is in one item; an even split over n items gives 1/n.";

/// `part` as a share of `whole` (0–1); 0 when `whole` is zero.
#[must_use]
#[expect(clippy::cast_precision_loss, reason = "a share for display")]
pub fn share(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// The running share of the total after each value, in the given order (a Pareto line).
#[must_use]
pub fn cumulative_shares(values: &[u64]) -> Vec<f64> {
    let total: u64 = values.iter().sum();
    let mut running = 0;
    values
        .iter()
        .map(|value| {
            running += value;
            share(running, total)
        })
        .collect()
}

/// The Herfindahl index of the values' shares; `None` when they add up to zero.
#[must_use]
pub fn herfindahl(values: &[u64]) -> Option<f64> {
    let total: u64 = values.iter().sum();
    (total > 0).then(|| {
        values
            .iter()
            .map(|value| share(*value, total).powi(2))
            .sum()
    })
}

#[cfg(test)]
mod tests {
    use super::{cumulative_shares, herfindahl, share};

    #[test]
    fn shares_and_cumulative_shares() {
        assert!((share(1, 4) - 0.25).abs() < f64::EPSILON);
        assert!(share(1, 0).abs() < f64::EPSILON);
        assert_eq!(cumulative_shares(&[50, 30, 20]), [0.5, 0.8, 1.0]);
        assert_eq!(cumulative_shares(&[0, 0]), [0.0, 0.0]);
    }

    #[test]
    fn herfindahl_spans_an_even_split_to_one_item() {
        assert_eq!(herfindahl(&[]), None);
        assert_eq!(herfindahl(&[0]), None);
        assert_eq!(herfindahl(&[9]), Some(1.0));
        assert_eq!(herfindahl(&[5, 5, 5, 5]), Some(0.25));
    }
}
