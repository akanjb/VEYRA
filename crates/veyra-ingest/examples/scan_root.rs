//! Developer diagnostic: ingests one Claude Code `projects` directory into a scratch
//! database, twice, and prints totals. Prints counts only, never content.
//!
//! ```text
//! cargo run --release -p veyra-ingest --example scan_root -- <projects-dir> <new-db-file>
//! ```

#![allow(clippy::print_stdout, reason = "a command-line diagnostic")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::sums::BaseSums;
use veyra_domain::time::UtcMillis;
use veyra_ingest::{DEFAULT_CHUNK_BYTES, IngestContext, ScanReport, scan_all};
use veyra_registry::Registry;
use veyra_store::Store;

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [root, database] = args.as_slice() else {
        eprintln!("usage: scan_root <projects-dir> <new-db-file>");
        return ExitCode::FAILURE;
    };
    if database.exists() {
        eprintln!("refusing to reuse {}: pass a new file", database.display());
        return ExitCode::FAILURE;
    }
    match run(root, database) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("scan_root: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    root: &std::path::Path,
    database: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open(database)?;
    let registry = Registry::bundled()?;
    let connectors = veyra_connectors::all();
    let root = root.to_str().ok_or("the root must be valid UTF-8")?;
    let source = store.register_source("claude-code", root, veyra_ingest::now())?;
    store.set_source_enabled(source.id, true, veyra_ingest::now())?;
    let cancel = AtomicBool::new(false);
    let context = IngestContext {
        store: &store,
        registry: &registry,
        connectors: &connectors,
        cancel: &cancel,
        chunk_bytes: DEFAULT_CHUNK_BYTES,
        throttle: None,
    };
    for pass in ["first scan", "rescan"] {
        let started = Instant::now();
        let report = scan_all(&context, "manual")?;
        print_report(pass, &report, started.elapsed());
    }
    print_totals(&store)?;
    Ok(())
}

fn print_report(pass: &str, r: &ScanReport, elapsed: std::time::Duration) {
    println!(
        "{pass}: {:.2}s, files seen {} read {} missing {} restarted {}, lines {}, \
         events inserted {} updated {} unchanged {} superseded {}, issues {}",
        elapsed.as_secs_f64(),
        r.files_seen,
        r.files_read,
        r.files_missing,
        r.restarts,
        r.lines,
        r.inserted,
        r.updated,
        r.unchanged,
        r.superseded,
        r.issues
    );
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
         cache write 5m {:?} 1h {:?} unspecified {:?}, output {:?} (reasoning {:?})",
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
        t.reasoning
    );
    #[allow(clippy::cast_precision_loss, reason = "display only")]
    let dollars = totals.cost.total.0 as f64 / 1e9;
    println!("API-equivalent estimate: ${dollars:.2}");
    Ok(())
}
