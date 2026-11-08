//! Time primitives. Instants are UTC milliseconds since the Unix epoch; calendar dates
//! (used for price effective dates) start at 00:00 UTC.

use std::fmt;

const MILLIS_PER_DAY: i64 = 86_400_000;

/// An instant as UTC milliseconds since 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcMillis(pub i64);

impl UtcMillis {
    /// Length of a rollup bucket. Every real-world UTC offset is a multiple of
    /// 15 minutes, so local days and hours are exact unions of buckets (ADR-009).
    pub const BUCKET_MILLIS: i64 = 15 * 60_000;

    /// Start of the 15-minute UTC bucket containing this instant.
    #[must_use]
    pub fn bucket_start(self) -> Self {
        Self(self.0.div_euclid(Self::BUCKET_MILLIS) * Self::BUCKET_MILLIS)
    }

    /// Whether this instant falls exactly on a bucket boundary.
    #[must_use]
    pub fn is_bucket_aligned(self) -> bool {
        self.0.rem_euclid(Self::BUCKET_MILLIS) == 0
    }

    /// The system clock now. The single source of "now" for adapters and use cases;
    /// domain rules never read the clock and take instants as inputs instead.
    #[must_use]
    pub fn now() -> Self {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Self(millis)
    }
}

/// Error raised for an invalid calendar date.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid date `{input}`: expected a real date in YYYY-MM-DD form")]
pub struct DateError {
    input: String,
}

/// A proleptic Gregorian calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CivilDate {
    /// Creates a date, rejecting impossible days (e.g. 2026-02-29).
    ///
    /// # Errors
    /// Returns [`DateError`] when the month or day is out of range.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, DateError> {
        let valid = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        if valid {
            Ok(Self { year, month, day })
        } else {
            Err(DateError {
                input: format!("{year:04}-{month:02}-{day:02}"),
            })
        }
    }

    /// Parses `YYYY-MM-DD`.
    ///
    /// # Errors
    /// Returns [`DateError`] for any other shape or an impossible date.
    pub fn parse(input: &str) -> Result<Self, DateError> {
        let error = || DateError {
            input: input.to_owned(),
        };
        let bytes = input.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(error());
        }
        let year = input[0..4].parse::<i32>().map_err(|_| error())?;
        let month = input[5..7].parse::<u8>().map_err(|_| error())?;
        let day = input[8..10].parse::<u8>().map_err(|_| error())?;
        Self::new(year, month, day).map_err(|_| error())
    }

    /// The instant this date begins, at 00:00 UTC.
    #[must_use]
    pub fn start_utc(self) -> UtcMillis {
        UtcMillis(days_from_civil(self.year, self.month, self.day) * MILLIS_PER_DAY)
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let (month, day) = (i64::from(month), i64::from(day));
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::{CivilDate, UtcMillis};

    #[test]
    fn known_dates_map_to_epoch_days() {
        let cases = [
            ("1970-01-01", 0),
            ("2000-01-01", 10_957),
            ("2024-01-01", 19_723),
            ("2024-02-29", 19_782),
        ];
        for (input, days) in cases {
            let date = CivilDate::parse(input).unwrap();
            assert_eq!(date.start_utc(), UtcMillis(days * 86_400_000), "{input}");
            assert_eq!(date.to_string(), input);
        }
    }

    #[test]
    fn rejects_malformed_and_impossible_dates() {
        for input in [
            "2026-2-01",
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "26-01-01",
            "",
        ] {
            assert!(CivilDate::parse(input).is_err(), "{input}");
        }
        assert!(CivilDate::parse("2028-02-29").is_ok());
    }

    #[test]
    fn buckets_are_fifteen_minutes_including_before_epoch() {
        let at = UtcMillis(15 * 60_000 + 59_999);
        assert_eq!(at.bucket_start(), UtcMillis(15 * 60_000));
        assert_eq!(UtcMillis(-1).bucket_start(), UtcMillis(-15 * 60_000));
        assert!(UtcMillis(30 * 60_000).is_bucket_aligned());
        assert!(!UtcMillis(1).is_bucket_aligned());
    }
}
