//! Performance budget check for the data core (PRD NFR-06).
//!
//! Generates a synthetic year of usage, ingests it through the real pricing engine and
//! store, then times the dashboard's overview queries and reports p50/p95 against the
//! budget. Usage: `cargo run --release -p veyra-perf -- [events] [--scattered] [--keep]`.
//! Events arrive session by session in time order (as log files are ingested) unless
//! `--scattered` is given, which spreads every event randomly (worst case).
//!
//! All data is synthetic (no real transcripts), generated deterministically.

#![expect(clippy::print_stdout, reason = "command-line report")]

mod dataset;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use jiff::tz::TimeZone;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::engine::evaluate;
use veyra_analytics::metrics::MetricId;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_analytics::top_n::top_n;
use veyra_domain::time::UtcMillis;
use veyra_store::Store;

/// Overview query budget at 1M events (p95).
const BUDGET_P95: Duration = Duration::from_millis(150);
const QUERY_RUNS: usize = 30;
const DAY: i64 = 86_400_000;

fn main() -> ExitCode {
    let mut events: usize = 1_000_000;
    let mut keep = false;
    let mut order = dataset::Order::Chronological;
    for arg in std::env::args().skip(1) {
        if arg == "--scattered" {
            order = dataset::Order::Scattered;
        } else if arg == "--keep" {
            keep = true;
        } else if let Ok(n) = arg.replace('_', "").parse() {
            events = n;
        }
    }
    match run(events, order, keep) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("veyra-perf: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    events: usize,
    order: dataset::Order,
    keep: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("veyra-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path: PathBuf = dir.join("veyra.db");
    let store = Store::open(&path)?;
    let catalog = veyra_registry::Registry::bundled()?.catalog().clone();

    println!("Generating and ingesting {events} synthetic events ({order:?} order)…");
    let started = Instant::now();
    let summary = dataset::ingest(&store, &catalog, events, order)?;
    let elapsed = started.elapsed();
    #[expect(clippy::cast_precision_loss, reason = "report only")]
    let rate = events as f64 / elapsed.as_secs_f64();
    println!(
        "  ingested {} events in {:.1}s ({rate:.0} events/s), {} rollup buckets rebuilt",
        summary.0,
        elapsed.as_secs_f64(),
        summary.1
    );
    println!(
        "  database size: {:.1} MB",
        bytes_to_mb(std::fs::metadata(&path)?.len())
    );

    let passed = overview_queries(&store)?;
    drop(store);
    if keep {
        println!("Kept database at {}", path.display());
    } else {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(passed)
}

#[expect(clippy::cast_precision_loss, reason = "report only")]
fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

/// Times the overview page's queries: 30-day daily trend by application with comparison,
/// and the 30-day model ranking. Returns whether every p95 is within budget.
fn overview_queries(store: &Store) -> Result<bool, Box<dyn std::error::Error>> {
    let tz = TimeZone::get("Asia/Kolkata")?;
    let end = UtcMillis(dataset::END.0 - dataset::END.0.rem_euclid(DAY));
    let period = Period::new(UtcMillis(end.0 - 30 * DAY), end)?;
    let daily_by_app = QuerySpec {
        period,
        comparison: Some(period.previous()),
        filters: Filters::new(),
        group_by: vec![Dimension::App],
        grain: TimeGrain::Day,
        time_zone: tz.clone(),
    };
    let by_model = QuerySpec {
        group_by: vec![Dimension::Model],
        grain: TimeGrain::Total,
        ..daily_by_app.clone()
    };
    let year = QuerySpec {
        period: Period::new(UtcMillis(end.0 - 365 * DAY), end)?,
        comparison: None,
        group_by: vec![],
        grain: TimeGrain::Month,
        ..daily_by_app.clone()
    };

    println!(
        "Overview queries ({QUERY_RUNS} runs each, budget p95 ≤ {} ms):",
        BUDGET_P95.as_millis()
    );
    let mut passed = true;
    for (name, spec) in [
        ("30-day daily trend by app, with comparison", &daily_by_app),
        ("30-day top models (top 8 + other)", &by_model),
        ("12-month monthly totals", &year),
    ] {
        let mut timings = Vec::with_capacity(QUERY_RUNS);
        for _ in 0..QUERY_RUNS {
            let started = Instant::now();
            let result = evaluate(store, spec)?;
            let _ = top_n(result.groups, MetricId::ApiEquivalentCost, 8);
            timings.push(started.elapsed());
        }
        timings.sort();
        let p50 = timings[QUERY_RUNS / 2];
        let p95 = timings[QUERY_RUNS * 95 / 100];
        let ok = p95 <= BUDGET_P95;
        passed &= ok;
        println!(
            "  {} {name}: p50 {:.1} ms, p95 {:.1} ms",
            if ok { "PASS" } else { "FAIL" },
            p50.as_secs_f64() * 1000.0,
            p95.as_secs_f64() * 1000.0
        );
    }
    Ok(passed)
}
