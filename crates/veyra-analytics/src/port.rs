//! The read port the data layer implements (hexagonal: the port belongs to its consumer).

use veyra_domain::time::UtcMillis;

use crate::dimension::{Dimension, DimensionValue, Filters};
use crate::sums::BaseSums;

/// A request for additive sums over `[from, until)`, grouped by dimensions and,
/// optionally, by 15-minute bucket. Both bounds are bucket-aligned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseSumsQuery {
    pub from: UtcMillis,
    pub until: UtcMillis,
    pub filters: Filters,
    pub group_by: Vec<Dimension>,
    pub by_bucket: bool,
}

/// One group of sums. `key` is aligned with [`BaseSumsQuery::group_by`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseSumsRow {
    pub bucket_start: Option<UtcMillis>,
    pub key: Vec<DimensionValue>,
    pub sums: BaseSums,
}

/// Error raised by a reader.
#[derive(Debug, thiserror::Error)]
#[error("analytics read failed: {0}")]
pub struct ReadError(pub String);

/// Implemented by the data layer to serve analytics queries.
pub trait BaseSumsReader {
    /// Returns sums grouped as requested.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn read_base_sums(&self, query: &BaseSumsQuery) -> Result<Vec<BaseSumsRow>, ReadError>;
}
