//! Repeated timing and the p95 budget it is checked against (PRD NFR-06).

use std::time::{Duration, Instant};

/// Runs per measured query.
const RUNS: usize = 30;
/// Datasets up to this many events get [`SMALL_P95`]; larger ones get [`LARGE_P95`].
const SMALL_DATASET: usize = 1_000_000;
const SMALL_P95: Duration = Duration::from_millis(150);
const LARGE_P95: Duration = Duration::from_millis(400);

/// The median and 95th percentile of [`RUNS`] runs.
#[derive(Debug, Clone, Copy)]
pub struct Timings {
    pub p50: Duration,
    pub p95: Duration,
}

/// Times `run` [`RUNS`] times.
pub fn timed<E>(mut run: impl FnMut() -> Result<(), E>) -> Result<Timings, E> {
    let mut timings = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let started = Instant::now();
        run()?;
        timings.push(started.elapsed());
    }
    timings.sort();
    Ok(Timings {
        p50: timings[RUNS / 2],
        p95: timings[RUNS * 95 / 100],
    })
}

/// The p95 a query must stay within.
#[derive(Debug, Clone, Copy)]
pub struct Budget(Duration);

impl Budget {
    /// The budget for a dataset of `events` events.
    pub fn for_events(events: usize) -> Self {
        Self(if events <= SMALL_DATASET {
            SMALL_P95
        } else {
            LARGE_P95
        })
    }

    /// Prints a section heading naming the budget.
    pub fn heading(self, title: &str) {
        println!(
            "{title} ({RUNS} runs each, budget p95 ≤ {} ms):",
            self.0.as_millis()
        );
    }

    /// Prints one result and returns whether it is within the budget.
    pub fn check(self, name: &str, timings: Timings) -> bool {
        let ok = timings.p95 <= self.0;
        println!(
            "  {} {name}: p50 {:.1} ms, p95 {:.1} ms",
            if ok { "PASS" } else { "FAIL" },
            timings.p50.as_secs_f64() * 1000.0,
            timings.p95.as_secs_f64() * 1000.0
        );
        ok
    }
}
