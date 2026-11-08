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

/// Events in `[from, until)` matching the filters, for activity metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTimesQuery {
    pub from: UtcMillis,
    pub until: UtcMillis,
    pub filters: Filters,
}

/// Event timestamps per session (implemented by the data layer).
pub trait ActivityReader {
    /// `(session id, occurred_at)` of matching events, ordered by session and then time.
    /// Events without a session are omitted.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn event_times(&self, query: &EventTimesQuery) -> Result<Vec<(i64, UtcMillis)>, ReadError>;
}

/// A project as stored: its id (the analytics dimension value) and normalised path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLabel {
    pub id: i64,
    pub path: String,
}

/// Human-readable labels for dimension values stored as ids.
pub trait LabelReader {
    /// Every known project.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn projects(&self) -> Result<Vec<ProjectLabel>, ReadError>;
}
