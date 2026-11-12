//! Resolves a dashboard [`Scope`] into analytics terms: a time zone, a period of local
//! days, an optional comparison period and dimension filters.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::port::RangeQuery;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};

use crate::dto::{Comparison, Scope};
use crate::error::ApiError;

/// The longest range a view may cover.
const MAX_DAYS: i32 = 731;

/// A validated scope.
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub time_zone: TimeZone,
    pub start: Date,
    pub end: Date,
    /// Local days from `start` to `end`, inclusive.
    pub days: i32,
    pub period: Period,
    pub previous: Option<Period>,
    pub filters: Filters,
}

fn validation(message: impl Into<String>) -> ApiError {
    ApiError::Validation {
        message: message.into(),
    }
}

fn date(value: &str, what: &str) -> Result<Date, ApiError> {
    value
        .parse::<Date>()
        .map_err(|_| validation(format!("{what} `{value}` is not a YYYY-MM-DD date")))
}

/// Validates a scope.
///
/// # Errors
/// [`ApiError::Validation`] for an unknown time zone, invalid dates, a reversed or overlong
/// range.
pub(crate) fn resolve(scope: &Scope) -> Result<Resolved, ApiError> {
    let time_zone = TimeZone::get(&scope.time_zone)
        .map_err(|_| validation(format!("unknown time zone `{}`", scope.time_zone)))?;
    let start = date(&scope.start, "start")?;
    let end = date(&scope.end, "end")?;
    let days = start
        .until(end)
        .map_err(|e| validation(e.to_string()))?
        .get_days()
        + 1;
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(validation(format!(
            "the range must cover 1 to {MAX_DAYS} days, not {days}"
        )));
    }
    let period =
        Period::local_days(start, days, &time_zone).map_err(|e| validation(e.to_string()))?;
    let previous = match scope.comparison {
        Comparison::None => None,
        Comparison::Previous => Some(period.previous()),
    };
    let f = &scope.filters;
    let mut filters = Filters::new();
    for (dimension, values) in [
        (Dimension::Provider, &f.providers),
        (Dimension::Model, &f.models),
        (Dimension::App, &f.apps),
        (Dimension::Interface, &f.interfaces),
        (Dimension::Project, &f.projects),
    ] {
        if !values.is_empty() {
            filters = filters.with(dimension, values.iter().cloned().map(Some));
        }
    }
    Ok(Resolved {
        time_zone,
        start,
        end,
        days,
        period,
        previous,
        filters,
    })
}

impl Resolved {
    /// A query over this scope.
    pub fn spec(&self, group_by: Vec<Dimension>, grain: TimeGrain, compare: bool) -> QuerySpec {
        QuerySpec {
            period: self.period,
            comparison: if compare { self.previous } else { None },
            filters: self.filters.clone(),
            group_by,
            grain,
            time_zone: self.time_zone.clone(),
        }
    }

    /// This scope narrowed to one project, whatever project filter it had.
    pub fn for_project(&self, id: i64) -> Self {
        Self {
            filters: self
                .filters
                .clone()
                .only(Dimension::Project, [Some(id.to_string())]),
            ..self.clone()
        }
    }

    /// The events of `period` that match this scope's filters.
    pub fn range(&self, period: Period) -> RangeQuery {
        RangeQuery {
            from: period.from(),
            until: period.until(),
            filters: self.filters.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use crate::ApiError;
    use crate::dto::{Comparison, Scope, ScopeFilters};

    fn scope(start: &str, end: &str, tz: &str) -> Scope {
        Scope {
            time_zone: tz.to_owned(),
            start: start.to_owned(),
            end: end.to_owned(),
            comparison: Comparison::Previous,
            filters: ScopeFilters {
                models: vec!["claude-opus-5-5".to_owned()],
                ..ScopeFilters::default()
            },
        }
    }

    #[test]
    fn resolves_local_days_and_the_previous_period() {
        let resolved = resolve(&scope("2026-11-01", "2026-11-07", "Asia/Kolkata")).unwrap();
        let (from, until) = (resolved.period.from().0, resolved.period.until().0);
        assert_eq!(until - from, 7 * 86_400_000);
        // Midnight in Kolkata is 18:30 UTC the day before.
        assert_eq!(from % 86_400_000, 66_600_000);
        let previous = resolved.previous.unwrap();
        assert_eq!(previous.until(), resolved.period.from());
        assert!(!resolved.filters.is_empty());
    }

    #[test]
    fn rejects_invalid_scopes() {
        for bad in [
            scope("2026-11-07", "2026-11-01", "UTC"),
            scope("2026-13-01", "2026-13-02", "UTC"),
            scope("2026-11-01", "2026-11-02", "Mars/Olympus"),
            scope("2020-01-01", "2026-01-01", "UTC"),
        ] {
            assert!(
                matches!(resolve(&bad), Err(ApiError::Validation { .. })),
                "{bad:?}"
            );
        }
    }
}
