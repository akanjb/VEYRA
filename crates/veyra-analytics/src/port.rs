//! The read port the data layer implements (hexagonal: the port belongs to its consumer).

use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{Assumptions, CostStatus};
use veyra_domain::time::UtcMillis;

use crate::activity::Span;
use crate::dimension::{Dimension, DimensionValue, Filters};
use crate::engine::period_index;
use crate::metrics::{MetricId, MetricValue};
use crate::sizes::{SessionSize, SizeRow};
use crate::sums::BaseSums;
use crate::tools::{CategoryCalls, ToolUse};

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

/// The total tokens of one group, aligned with [`BaseSumsQuery::group_by`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRow {
    pub bucket_start: Option<UtcMillis>,
    pub key: Vec<DimensionValue>,
    pub tokens: u64,
}

/// The API-equivalent cost of one group, aligned with [`BaseSumsQuery::group_by`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostRow {
    pub bucket_start: Option<UtcMillis>,
    pub key: Vec<DimensionValue>,
    pub cost: NanoUsd,
}

/// Implemented by the data layer to serve analytics queries.
pub trait BaseSumsReader {
    /// Returns sums grouped as requested.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn read_base_sums(&self, query: &BaseSumsQuery) -> Result<Vec<BaseSumsRow>, ReadError>;

    /// Sums grouped as requested and by local period: each row's `bucket_start` is the
    /// start of the period in `starts` that contains its buckets (`query.by_bucket` is
    /// ignored). `starts` are ascending, bucket-aligned, and the first is at or before
    /// `query.from`. A key may appear more than once per period; callers merge rows.
    /// A store may answer this with far fewer rows than one per 15-minute bucket.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn read_base_sums_by_period(
        &self,
        query: &BaseSumsQuery,
        starts: &[UtcMillis],
    ) -> Result<Vec<BaseSumsRow>, ReadError> {
        let by_bucket = BaseSumsQuery {
            by_bucket: true,
            ..query.clone()
        };
        Ok(in_periods(self.read_base_sums(&by_bucket)?, starts))
    }

    /// Total tokens ([`MetricId::TotalTokens`], zero when none were reported) grouped as
    /// requested, for cards that show tokens only. A store may answer this more cheaply
    /// than [`read_base_sums`](Self::read_base_sums), with the same values.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn read_tokens(&self, query: &BaseSumsQuery) -> Result<Vec<TokenRow>, ReadError> {
        Ok(self
            .read_base_sums(query)?
            .into_iter()
            .map(|row| TokenRow {
                tokens: match MetricId::TotalTokens.compute(&row.sums) {
                    Some(MetricValue::Count(n)) => n,
                    _ => 0,
                },
                bucket_start: row.bucket_start,
                key: row.key,
            })
            .collect())
    }

    /// API-equivalent cost ([`MetricId::ApiEquivalentCost`]) grouped as requested, for
    /// cards that show cost only. A store may answer this more cheaply than
    /// [`read_base_sums`](Self::read_base_sums), with the same values.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn read_costs(&self, query: &BaseSumsQuery) -> Result<Vec<CostRow>, ReadError> {
        Ok(self
            .read_base_sums(query)?
            .into_iter()
            .map(|row| CostRow {
                cost: row.sums.cost.total,
                bucket_start: row.bucket_start,
                key: row.key,
            })
            .collect())
    }
}

/// Rows per 15-minute bucket relabelled with the start of the period in `starts` that
/// contains their bucket, as [`BaseSumsReader::read_base_sums_by_period`] returns them.
#[must_use]
pub fn in_periods(rows: Vec<BaseSumsRow>, starts: &[UtcMillis]) -> Vec<BaseSumsRow> {
    rows.into_iter()
        .map(|row| BaseSumsRow {
            bucket_start: row
                .bucket_start
                .map(|bucket| period_index(starts, bucket).map_or(bucket, |i| starts[i])),
            ..row
        })
        .collect()
}

/// Events in `[from, until)` matching the filters, for activity metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeQuery {
    pub from: UtcMillis,
    pub until: UtcMillis,
    pub filters: Filters,
}

/// Session activity as spans, one per session and 15-minute bucket (implemented by the
/// data layer), with the idle threshold [`crate::activity::DEFAULT_IDLE_THRESHOLD_MS`].
pub trait ActivityReader {
    /// Spans of matching events, ordered by session and then time. Events without a
    /// session are omitted.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn spans(&self, query: &RangeQuery) -> Result<Vec<Span>, ReadError>;

    /// The spans of the given sessions only, ordered by session and then time.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn session_spans(&self, query: &RangeQuery, sessions: &[i64]) -> Result<Vec<Span>, ReadError>;

    /// `(project id, span)` of matching events with both a project and a session,
    /// ordered by project, session and then time.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn project_spans(&self, query: &RangeQuery) -> Result<Vec<(i64, Span)>, ReadError>;
}

/// Request and session sizes (implemented by the data layer).
pub trait SizeReader {
    /// Request counts per 15-minute bucket and size bin, for events in `[from, until)`
    /// that record individual requests.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn request_sizes(&self, query: &RangeQuery) -> Result<Vec<SizeRow>, ReadError>;

    /// Each session's tokens over its events in `[from, until)`, with the time of the
    /// first of them. Events without a session are omitted.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn session_sizes(&self, query: &RangeQuery) -> Result<Vec<SessionSize>, ReadError>;
}

/// Tool calls (implemented by the data layer from the tool rollup, ADR-034).
pub trait ToolReader {
    /// Each tool called by matching events in `[from, until)`, with its counts.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn tool_uses(&self, query: &RangeQuery) -> Result<Vec<ToolUse>, ReadError>;

    /// Calls per 15-minute bucket and category, ordered by bucket.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn category_calls(&self, query: &RangeQuery) -> Result<Vec<CategoryCalls>, ReadError>;
}

/// A project as stored: its id (the analytics dimension value), normalised path and the
/// name the user gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLabel {
    pub id: i64,
    pub path: String,
    pub name: Option<String>,
}

impl ProjectLabel {
    /// What the project is called: its name, or else its path.
    #[must_use]
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.path)
    }
}

/// Human-readable labels for dimension values stored as ids.
pub trait LabelReader {
    /// Every known project.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn projects(&self) -> Result<Vec<ProjectLabel>, ReadError>;
}

/// How a group of events was priced: one row per provider, model, rule, outcome and set
/// of assumptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingUse {
    /// `None` when the source did not say which provider served the request.
    pub provider: Option<String>,
    /// The canonical model id, or the reported name when the model is unknown.
    pub model: Option<String>,
    /// The price rule applied; `None` when no rule covers the model.
    pub rule_id: Option<String>,
    pub status: CostStatus,
    pub assumptions: Assumptions,
    pub tally: Tally,
}

/// Events, tokens and API-equivalent cost of a group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub events: u64,
    pub tokens: u64,
    pub cost: NanoUsd,
}

impl std::ops::AddAssign for Tally {
    fn add_assign(&mut self, other: Self) {
        self.events += other.events;
        self.tokens += other.tokens;
        self.cost = self.cost + other.cost;
    }
}

/// How events were priced (implemented by the data layer).
pub trait PricingReader {
    /// Events in `[from, until)` matching the filters, grouped by how they were priced.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the underlying store fails.
    fn pricing_uses(&self, query: &RangeQuery) -> Result<Vec<PricingUse>, ReadError>;
}
