//! Accuracy composition: how much of a token total is exact, reported or estimated
//! (PRD FR-AN-04).

use veyra_domain::accuracy::Accuracy;

use crate::sums::BaseSums;

/// Tokens per accuracy class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AccuracyComposition {
    tokens: [u64; 5],
}

impl AccuracyComposition {
    #[must_use]
    pub fn of(sums: &BaseSums) -> Self {
        Self {
            tokens: sums.tokens_by_accuracy,
        }
    }

    #[must_use]
    pub fn total(&self) -> u64 {
        self.tokens.iter().sum()
    }

    #[must_use]
    pub fn tokens(&self, accuracy: Accuracy) -> u64 {
        Accuracy::ALL
            .iter()
            .position(|a| *a == accuracy)
            .map_or(0, |i| self.tokens[i])
    }

    /// Share of tokens in a class; `None` when there are no tokens.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "display share")]
    pub fn share(&self, accuracy: Accuracy) -> Option<f64> {
        let total = self.total();
        (total > 0).then(|| self.tokens(accuracy) as f64 / total as f64)
    }

    /// The weakest class present: the badge an aggregate carries.
    #[must_use]
    pub fn weakest(&self) -> Option<Accuracy> {
        Accuracy::ALL
            .into_iter()
            .filter(|a| self.tokens(*a) > 0)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::AccuracyComposition;
    use crate::sums::BaseSums;
    use veyra_domain::accuracy::Accuracy;

    #[test]
    fn reports_shares_and_weakest_class() {
        let sums = BaseSums {
            tokens_by_accuracy: [960, 0, 0, 40, 0],
            ..BaseSums::default()
        };
        let c = AccuracyComposition::of(&sums);
        assert_eq!(c.total(), 1_000);
        assert_eq!(c.share(Accuracy::Exact), Some(0.96));
        assert_eq!(c.share(Accuracy::Estimated), Some(0.04));
        assert_eq!(c.weakest(), Some(Accuracy::Estimated));
        assert_eq!(AccuracyComposition::default().weakest(), None);
    }
}
