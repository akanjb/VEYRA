//! Performance budget check for the data core (PRD NFR-06).
//!
//! Generates a synthetic year of usage, ingests it through the real pricing engine and
//! store, then times the overview's analytics queries and the command behind every
//! dashboard page, and reports p50/p95 against the budget: 150 ms up to one million
//! events, 400 ms above.
//!
//! Usage: `cargo run --release -p veyra-perf -- [events] [--scattered] [--keep]`.
//! `--database <path>` times a database kept by an earlier `--keep` run instead of
//! generating one; `events` must then match the run that made it.
//! Events arrive session by session in time order (as log files are ingested) unless
//! `--scattered` is given, which spreads every event randomly (worst case).
//!
//! All data is synthetic (no real transcripts), generated deterministically.

#![expect(clippy::print_stdout, reason = "command-line report")]

mod dataset;
mod pages;
mod reads;
mod timing;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use jiff::tz::TimeZone;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::engine::evaluate;
use veyra_analytics::metrics::MetricId;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_analytics::top_n::top_n;
use veyra_app::DisplayNames;
use veyra_domain::time::UtcMillis;
use veyra_registry::Registry;
use veyra_store::Store;

use timing::{Budget, timed};

const DAY: i64 = 86_400_000;

/// What to measure, from the command line.
struct Options {
    events: usize,
    order: dataset::Order,
    keep: bool,
    /// An existing database to time instead of a generated one.
    database: Option<PathBuf>,
}

fn options() -> Options {
    let mut options = Options {
        events: 1_000_000,
        order: dataset::Order::Chronological,
        keep: false,
        database: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--scattered" {
            options.order = dataset::Order::Scattered;
        } else if arg == "--keep" {
            options.keep = true;
        } else if arg == "--database" {
            options.database = args.next().map(PathBuf::from);
        } else if let Ok(n) = arg.replace('_', "").parse() {
            options.events = n;
        }
    }
    options
}

fn main() -> ExitCode {
    match run(&options()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("veyra-perf: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: &Options) -> Result<bool, Box<dyn std::error::Error>> {
    let registry = Registry::bundled()?;
    let catalog = registry.catalog().clone();
    let (store, generated) = if let Some(path) = &options.database {
        println!("Timing the database at {}", path.display());
        (Arc::new(Store::open(path)?), None)
    } else {
        let dir = std::env::temp_dir().join(format!("veyra-perf-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("veyra.db");
        let store = Arc::new(Store::open(&path)?);
        generate(&store, &catalog, options, &path)?;
        (store, Some(dir))
    };

    let budget = Budget::for_events(options.events);
    let names = DisplayNames::new(registry.provider_names(), registry.model_names(), &[]);
    let queries = overview_queries(&store, budget)?;
    reads::data_reads(&store)?;
    let pages = pages::page_commands(&store, catalog, names, budget)?;
    drop(store);
    if let Some(dir) = generated {
        if options.keep {
            println!("Kept database at {}", dir.join("veyra.db").display());
        } else {
            std::fs::remove_dir_all(&dir)?;
        }
    }
    Ok(queries && pages)
}

/// Generates and ingests the synthetic dataset, reporting throughput and size.
fn generate(
    store: &Store,
    catalog: &veyra_domain::pricing::PriceCatalog,
    options: &Options,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (events, order) = (options.events, options.order);
    println!("Generating and ingesting {events} synthetic events ({order:?} order)…");
    let started = Instant::now();
    let summary = dataset::ingest(store, catalog, events, order)?;
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
        bytes_to_mb(std::fs::metadata(path)?.len())
    );
    Ok(())
}

#[expect(clippy::cast_precision_loss, reason = "report only")]
fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

/// Times the overview page's queries: 30-day daily trend by application with comparison,
/// the 30-day model ranking and 12 monthly totals. Returns whether every p95 is within
/// the budget.
fn overview_queries(store: &Store, budget: Budget) -> Result<bool, Box<dyn std::error::Error>> {
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

    budget.heading("Overview analytics queries");
    let mut passed = true;
    for (name, spec) in [
        ("30-day daily trend by app, with comparison", &daily_by_app),
        ("30-day top models (top 8 + other)", &by_model),
        ("12-month monthly totals", &year),
    ] {
        let timings = timed(|| {
            let result = evaluate(store, spec)?;
            let _ = top_n(result.groups, MetricId::ApiEquivalentCost, 8);
            Ok::<_, veyra_analytics::engine::EngineError>(())
        })?;
        passed &= budget.check(name, timings);
    }
    Ok(passed)
}
