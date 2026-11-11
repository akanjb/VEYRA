//! Dimensions that usage can be grouped and filtered by.

use std::collections::{BTreeMap, BTreeSet};

/// A groupable attribute of usage. All are available in the 15-minute rollups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dimension {
    Provider,
    Model,
    App,
    Interface,
    Gateway,
    Project,
    Device,
}

impl Dimension {
    pub const ALL: [Self; 7] = [
        Self::Provider,
        Self::Model,
        Self::App,
        Self::Interface,
        Self::Gateway,
        Self::Project,
        Self::Device,
    ];
}

/// The value of one dimension for a group. `None` means the attribute is unknown
/// (e.g. an unrecognised model), which is a distinct group, not "everything".
pub type DimensionValue = Option<String>;

/// Restricts a query to the listed values per dimension. A dimension with no entry is
/// unrestricted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    values: BTreeMap<Dimension, BTreeSet<DimensionValue>>,
}

impl Filters {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds allowed values for a dimension; values for the same dimension are alternatives.
    #[must_use]
    pub fn with(
        mut self,
        dimension: Dimension,
        values: impl IntoIterator<Item = DimensionValue>,
    ) -> Self {
        self.values.entry(dimension).or_default().extend(values);
        self
    }

    /// Restricts a dimension to exactly these values, replacing any earlier restriction.
    #[must_use]
    pub fn only(
        mut self,
        dimension: Dimension,
        values: impl IntoIterator<Item = DimensionValue>,
    ) -> Self {
        self.values.insert(dimension, values.into_iter().collect());
        self
    }

    /// Restricted dimensions and their allowed values.
    pub fn iter(&self) -> impl Iterator<Item = (Dimension, &BTreeSet<DimensionValue>)> {
        self.values.iter().map(|(d, v)| (*d, v))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
