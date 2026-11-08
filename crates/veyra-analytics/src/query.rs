//! Query specifications: what to aggregate, over which period, in which time zone.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use veyra_domain::time::UtcMillis;

use crate::dimension::{Dimension, Filters};

/// Error raised for an invalid query specification.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("period bounds must fall on 15-minute boundaries")]
    Unaligned,
    #[error("period must end after it starts")]
    Empty,
    #[error("dimension {0:?} is grouped twice")]
    DuplicateDimension(Dimension),
    #[error("invalid time: {0}")]
    Time(#[from] jiff::Error),
}

/// A half-open UTC interval `[from, until)` on 15-minute boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    from: UtcMillis,
    until: UtcMillis,
}

impl Period {
    /// # Errors
    /// Returns [`QueryError`] when the bounds are unaligned or empty.
    pub fn new(from: UtcMillis, until: UtcMillis) -> Result<Self, QueryError> {
        if !from.is_bucket_aligned() || !until.is_bucket_aligned() {
            return Err(QueryError::Unaligned);
        }
        if until <= from {
            return Err(QueryError::Empty);
        }
        Ok(Self { from, until })
    }

    /// `days` local calendar days starting at `first` in `tz` (DST-aware).
    ///
    /// # Errors
    /// Returns [`QueryError`] for out-of-range dates or misaligned zone offsets.
    pub fn local_days(first: Date, days: i32, tz: &TimeZone) -> Result<Self, QueryError> {
        let start = first.to_zoned(tz.clone())?;
        let end = first.checked_add(days.days())?.to_zoned(tz.clone())?;
        Self::new(millis(start.timestamp()), millis(end.timestamp()))
    }

    /// The equal-length period immediately before this one.
    #[must_use]
    pub fn previous(&self) -> Self {
        let length = self.until.0 - self.from.0;
        Self {
            from: UtcMillis(self.from.0 - length),
            until: self.from,
        }
    }

    #[must_use]
    pub fn from(&self) -> UtcMillis {
        self.from
    }

    #[must_use]
    pub fn until(&self) -> UtcMillis {
        self.until
    }
}

pub(crate) fn millis(timestamp: Timestamp) -> UtcMillis {
    UtcMillis(timestamp.as_millisecond())
}

/// How results are split over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeGrain {
    /// One total for the whole period.
    Total,
    Hour,
    Day,
    /// ISO weeks, starting Monday.
    Week,
    Month,
}

/// A complete analytics query.
#[derive(Debug, Clone)]
pub struct QuerySpec {
    pub period: Period,
    /// Period to compare against; often [`Period::previous`].
    pub comparison: Option<Period>,
    pub filters: Filters,
    pub group_by: Vec<Dimension>,
    pub grain: TimeGrain,
    /// Local time zone for hours, days, weeks and months.
    pub time_zone: TimeZone,
}

impl QuerySpec {
    /// # Errors
    /// Returns [`QueryError::DuplicateDimension`] when a dimension is grouped twice.
    pub fn validate(&self) -> Result<(), QueryError> {
        for (i, dimension) in self.group_by.iter().enumerate() {
            if self.group_by[..i].contains(dimension) {
                return Err(QueryError::DuplicateDimension(*dimension));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use jiff::tz::TimeZone;
    use veyra_domain::time::UtcMillis;

    use super::{Period, QueryError};

    #[test]
    fn local_days_follow_the_time_zone() {
        // 2026-11-08 in Asia/Kolkata (+05:30) starts at 2026-11-07T18:30Z.
        let tz = TimeZone::get("Asia/Kolkata").unwrap();
        let period = Period::local_days(date(2026, 11, 8), 1, &tz).unwrap();
        assert_eq!(
            period.from(),
            UtcMillis(1_794_096_000_000 - 5 * 3_600_000 - 1_800_000)
        );
        assert_eq!(period.until().0 - period.from().0, 86_400_000);
    }

    #[test]
    fn rejects_unaligned_and_empty_periods() {
        assert!(matches!(
            Period::new(UtcMillis(1), UtcMillis(900_000)),
            Err(QueryError::Unaligned)
        ));
        assert!(matches!(
            Period::new(UtcMillis(0), UtcMillis(0)),
            Err(QueryError::Empty)
        ));
    }

    #[test]
    fn previous_period_has_equal_length() {
        let p = Period::new(UtcMillis(1_800_000), UtcMillis(3_600_000)).unwrap();
        let prev = p.previous();
        assert_eq!(
            (prev.from(), prev.until()),
            (UtcMillis(0), UtcMillis(1_800_000))
        );
    }
}
