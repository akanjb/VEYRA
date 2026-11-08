//! Accuracy classes (ADR-006). Every metric group on every event states how it was
//! obtained, so aggregates can report how much of a number is measured.

/// How a value was obtained, ordered from strongest to weakest evidence.
///
/// The order matters: an aggregate takes the *weakest* class of its contributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Accuracy {
    /// Recorded directly by the source.
    Exact,
    /// Supplied by the provider's own usage interface.
    ProviderReported,
    /// Some events or fields are missing.
    Partial,
    /// Calculated with a documented method.
    Estimated,
    /// No reliable, permitted source exists.
    Unsupported,
}

impl Accuracy {
    pub const ALL: [Self; 5] = [
        Self::Exact,
        Self::ProviderReported,
        Self::Partial,
        Self::Estimated,
        Self::Unsupported,
    ];

    /// Stable storage and wire representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::ProviderReported => "provider-reported",
            Self::Partial => "partial",
            Self::Estimated => "estimated",
            Self::Unsupported => "unsupported",
        }
    }

    /// Parses the representation produced by [`Accuracy::as_str`].
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == value)
    }

    /// The weaker of two classes.
    #[must_use]
    pub fn weakest(self, other: Self) -> Self {
        self.max(other)
    }
}

/// The weakest class among contributions, or `None` when there are none.
pub fn weakest_of(classes: impl IntoIterator<Item = Accuracy>) -> Option<Accuracy> {
    classes.into_iter().max()
}

/// Accuracy of each metric group on one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccuracyMap {
    pub tokens: Accuracy,
    pub cache: Accuracy,
    pub reasoning: Accuracy,
    pub requests: Accuracy,
    pub cost: Accuracy,
}

impl AccuracyMap {
    /// The same class for every group.
    #[must_use]
    pub fn uniform(accuracy: Accuracy) -> Self {
        Self {
            tokens: accuracy,
            cache: accuracy,
            reasoning: accuracy,
            requests: accuracy,
            cost: accuracy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Accuracy, weakest_of};

    #[test]
    fn weakest_class_wins() {
        assert_eq!(
            Accuracy::Exact.weakest(Accuracy::Estimated),
            Accuracy::Estimated
        );
        assert_eq!(
            weakest_of([
                Accuracy::Exact,
                Accuracy::Partial,
                Accuracy::ProviderReported
            ]),
            Some(Accuracy::Partial)
        );
        assert_eq!(weakest_of([]), None);
    }

    #[test]
    fn round_trips() {
        for accuracy in Accuracy::ALL {
            assert_eq!(Accuracy::parse(accuracy.as_str()), Some(accuracy));
        }
    }
}
