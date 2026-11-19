//! Developer diagnostic: runs the local OpenTelemetry receiver (ADR-038) on its standard
//! port, writing to a scratch database, and prints what each export changed and the
//! stored totals. Prints counts only, never content.
//!
//! ```text
//! cargo run -p veyra-ingest --example receive -- <new-db-file> <seconds>
//! ```

#![allow(clippy::print_stdout, reason = "a command-line diagnostic")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::sums::BaseSums;
use veyra_domain::telemetry::TelemetryReceiver;
use veyra_domain::time::UtcMillis;
use veyra_ingest::{DEFAULT_PORT, Receiver, ReceiverParts};
use veyra_registry::Registry;
use veyra_store::Store;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [database, seconds] = args.as_slice() else {
        eprintln!("usage: receive <new-db-file> <seconds>");
        return ExitCode::FAILURE;
    };
    let database = PathBuf::from(database);
    if database.exists() {
        eprintln!("refusing to reuse {}: pass a new file", database.display());
        return ExitCode::FAILURE;
    }
    let Ok(seconds) = seconds.parse::<u64>() else {
        eprintln!("receive: seconds must be a whole number");
        return ExitCode::FAILURE;
    };
    match run(&database, Duration::from_secs(seconds)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("receive: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(database: &std::path::Path, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(Store::open(database)?);
    let receiver = Receiver::new(
        ReceiverParts {
            store: Arc::clone(&store),
            registry: Arc::new(Registry::bundled()?),
            connectors: veyra_connectors::all(),
            on_change: Box::new(|report| {
                println!(
                    "export stored: inserted {} updated {} unchanged {} superseded {}",
                    report.inserted, report.updated, report.unchanged, report.superseded
                );
            }),
        },
        DEFAULT_PORT,
    );
    receiver.set_enabled(true);
    let status = receiver.status();
    if let Some(error) = status.error {
        return Err(error.into());
    }
    println!(
        "listening on {} for {}s",
        status.endpoint,
        duration.as_secs()
    );
    let started = Instant::now();
    let mut shown = (0, 0, None);
    while started.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(1));
        let status = receiver.status();
        let now = (status.exports, status.records, status.refusal);
        if now != shown {
            println!(
                "exports accepted: {}, usage records: {}, latest refusal: {}",
                now.0,
                now.1,
                now.2.as_deref().unwrap_or("none")
            );
            shown = now;
        }
    }
    receiver.set_enabled(false);
    print_totals(&store)
}

fn print_totals(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let rows = store.read_base_sums(&BaseSumsQuery {
        from: UtcMillis(0),
        until: UtcMillis(i64::MAX / 2),
        filters: Filters::new(),
        group_by: vec![],
        by_bucket: false,
    })?;
    let totals = BaseSums::total(rows.iter().map(|r| &r.sums));
    let t = &totals.tokens;
    println!(
        "totals: events {} (priced {}, partial {}, unpriced {}), input {:?}, cache read {:?}, \
         cache write 5m {:?} 1h {:?} unspecified {:?}, output {:?}",
        totals.events,
        totals.events_priced,
        totals.events_partial,
        totals.events_unpriced,
        t.input_uncached,
        t.cache_read,
        t.cache_write_short,
        t.cache_write_long,
        t.cache_write_unspecified,
        t.output_total,
    );
    Ok(())
}
