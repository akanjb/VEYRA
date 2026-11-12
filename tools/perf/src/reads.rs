//! The individual reads pages are built from, timed over the same 30 days as the pages.
//! Reported without a budget: they show which read makes a page miss its budget.

use jiff::tz::TimeZone;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::port::{
    ActivityReader, BaseSumsQuery, BaseSumsReader, PricingReader, RangeQuery, SizeReader,
};
use veyra_analytics::query::Period;
use veyra_analytics::sessions::{SessionQuery, SessionReader, SessionSort};
use veyra_store::Store;

use crate::pages::{FIRST_DAY, TIME_ZONE};
use crate::timing::{RUNS, report, timed};

/// Days in the timed period.
const DAYS: i32 = 30;

type Read<'a> = Box<dyn Fn() -> Result<(), Box<dyn std::error::Error>> + 'a>;

fn sums(store: &Store, period: Period, group_by: Vec<Dimension>, by_bucket: bool) -> Read<'_> {
    Box::new(move || {
        store.read_base_sums(&BaseSumsQuery {
            from: period.from(),
            until: period.until(),
            filters: Filters::new(),
            group_by: group_by.clone(),
            by_bucket,
        })?;
        Ok(())
    })
}

fn sessions<'a>(store: &'a Store, range: &RangeQuery, sort: SessionSort) -> Read<'a> {
    let (from, until) = (range.from, range.until);
    Box::new(move || {
        store.sessions(&SessionQuery {
            from,
            until,
            filters: Filters::new(),
            sort,
            limit: 50,
            offset: 0,
        })?;
        Ok(())
    })
}

/// Times each read and prints it.
pub fn data_reads(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let time_zone = TimeZone::get(TIME_ZONE)?;
    let period = Period::local_days(FIRST_DAY.parse()?, DAYS, &time_zone)?;
    let range = RangeQuery {
        from: period.from(),
        until: period.until(),
        filters: Filters::new(),
    };
    let reads: Vec<(&str, Read<'_>)> = vec![
        (
            "sums per bucket, ungrouped",
            sums(store, period, vec![], true),
        ),
        (
            "sums per bucket by model",
            sums(store, period, vec![Dimension::Model], true),
        ),
        (
            "sums by provider, model and app",
            sums(
                store,
                period,
                vec![Dimension::Provider, Dimension::Model, Dimension::App],
                false,
            ),
        ),
        (
            "event times per session",
            Box::new(|| {
                store.event_times(&range)?;
                Ok(())
            }),
        ),
        (
            "event times per project and session",
            Box::new(|| {
                store.project_event_times(&range)?;
                Ok(())
            }),
        ),
        (
            "request sizes",
            Box::new(|| {
                store.request_sizes(&range)?;
                Ok(())
            }),
        ),
        (
            "session sizes",
            Box::new(|| {
                store.session_sizes(&range)?;
                Ok(())
            }),
        ),
        (
            "pricing outcomes",
            Box::new(|| {
                store.pricing_uses(&range)?;
                Ok(())
            }),
        ),
        (
            "session page by last activity",
            sessions(store, &range, SessionSort::LastActive),
        ),
        (
            "session page by cost",
            sessions(store, &range, SessionSort::Cost),
        ),
    ];
    println!("Data reads over the same 30 days ({RUNS} runs each, for diagnosis):");
    for (name, read) in reads {
        report("·", name, timed(&*read)?);
    }
    Ok(())
}
