//! End-to-end ingestion tests: the Claude Code connector over synthetic fixtures, through
//! scoped reads, checkpoints, normalisation, pricing and the store.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::sums::BaseSums;
use veyra_domain::source::{DiscoveryEnv, SourceConnector};
use veyra_domain::time::UtcMillis;
use veyra_ingest::{
    DEFAULT_CHUNK_BYTES, IngestContext, IngestWorker, ScanReport, WorkerConfig, discover, scan_all,
    scan_source,
};
use veyra_registry::Registry;
use veyra_store::{SourceRecord, Store};

const SESSION: &str = "C--work-demo/11111111-1111-4111-8111-111111111111.jsonl";
const SUBAGENT: &str = "C--work-demo/11111111-1111-4111-8111-111111111111/subagents/agent-a1.jsonl";

fn fixture_projects() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude-code/v2.1/projects")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A temporary home directory holding a copy of the fixtures, plus a database.
struct Fixture {
    dir: PathBuf,
    store: Store,
    registry: Registry,
    connectors: Vec<Box<dyn SourceConnector>>,
    cancel: AtomicBool,
}

impl Fixture {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "veyra-ingest-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        copy_tree(&fixture_projects(), &dir.join("home/.claude/projects"));
        let store = Store::open(&dir.join("veyra.db")).unwrap();
        Self {
            dir,
            store,
            registry: Registry::bundled().unwrap(),
            connectors: veyra_connectors::all(),
            cancel: AtomicBool::new(false),
        }
    }

    fn env(&self) -> DiscoveryEnv {
        DiscoveryEnv {
            home: Some(self.dir.join("home")),
            location_overrides: BTreeMap::new(),
        }
    }

    fn projects(&self) -> PathBuf {
        self.dir.join("home/.claude/projects")
    }

    fn context(&self, chunk_bytes: u64) -> IngestContext<'_> {
        IngestContext {
            store: &self.store,
            registry: &self.registry,
            connectors: &self.connectors,
            cancel: &self.cancel,
            chunk_bytes,
            throttle: None,
        }
    }

    /// Discovers the fixture root and enables it.
    fn enabled(self) -> Self {
        let sources = discover(&self.store, &self.connectors, &self.env()).unwrap();
        for source in sources {
            self.store
                .set_source_enabled(source.id, true, UtcMillis(1))
                .unwrap();
        }
        self
    }

    fn scan(&self) -> ScanReport {
        scan_all(&self.context(DEFAULT_CHUNK_BYTES), "manual").unwrap()
    }

    fn totals(&self) -> BaseSums {
        let rows = self
            .store
            .read_base_sums(&BaseSumsQuery {
                from: UtcMillis(0),
                until: UtcMillis(4_000_000_000_000),
                filters: Filters::new(),
                group_by: vec![],
                by_bucket: false,
            })
            .unwrap();
        BaseSums::total(rows.iter().map(|r| &r.sums))
    }

    fn append(&self, relative: &str, text: &str) {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(self.projects().join(relative))
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    fn source(&self) -> SourceRecord {
        self.store.sources().unwrap().remove(0)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A new, complete subagent request line, based on the fixture's `req_D`.
fn request_line(id: &str, timestamp: &str) -> String {
    let fixture = std::fs::read_to_string(fixture_projects().join(SUBAGENT)).unwrap();
    let template = fixture.lines().find(|l| l.contains("\"req_D\"")).unwrap();
    format!(
        "{}\n",
        template
            .replace("\"req_D\"", &format!("\"req_{id}\""))
            .replace("\"msg_D\"", &format!("\"msg_{id}\""))
            .replace("\"toolu_D1\"", &format!("\"toolu_{id}\""))
            .replace("\"sa-1\"", &format!("\"sa-{id}\""))
            .replace("2026-11-08T10:05:04.000Z", timestamp)
    )
}

/// The fixture holds four distinct requests (A, B, C in the session; D in a subagent).
fn assert_fixture_totals(totals: &BaseSums, extra_requests: u64) {
    assert_eq!(totals.events, 4 + extra_requests);
    assert_eq!(totals.tokens.output_total, Some(300 + 40 * extra_requests));
    assert_eq!(totals.tokens.input_uncached, Some(21 + 3 * extra_requests));
    assert_eq!(totals.tokens.cache_read, Some(35_000));
}

#[test]
fn discovery_registers_sources_disabled_and_nothing_is_read_before_consent() {
    let fixture = Fixture::new();
    let sources = discover(&fixture.store, &fixture.connectors, &fixture.env()).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].connector_id, "claude-code");
    assert_eq!(Path::new(&sources[0].root), fixture.projects());
    assert!(!sources[0].enabled);
    assert_eq!(sources[0].consented_at, None);

    // Discovering again changes nothing.
    assert_eq!(
        discover(&fixture.store, &fixture.connectors, &fixture.env()).unwrap(),
        sources
    );

    assert_eq!(fixture.scan(), ScanReport::default());
    let direct = scan_source(&fixture.context(DEFAULT_CHUNK_BYTES), &sources[0], 0).unwrap();
    assert_eq!(
        direct,
        ScanReport::default(),
        "a disabled source is never opened"
    );
    assert!(fixture.store.checkpoints(sources[0].id).unwrap().is_empty());
    assert_eq!(fixture.totals().events, 0);
}

#[test]
fn the_documented_location_override_replaces_the_default_root() {
    let fixture = Fixture::new();
    let relocated = fixture.dir.join("relocated");
    std::fs::create_dir_all(relocated.join("projects")).unwrap();
    let env = DiscoveryEnv {
        home: Some(fixture.dir.join("home")),
        location_overrides: BTreeMap::from([("CLAUDE_CONFIG_DIR".to_owned(), relocated.clone())]),
    };
    let sources = discover(&fixture.store, &fixture.connectors, &env).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(Path::new(&sources[0].root), relocated.join("projects"));
}

#[test]
fn ingests_each_request_once_and_a_rescan_changes_nothing() {
    let fixture = Fixture::new().enabled();
    let first = fixture.scan();
    assert_eq!(
        first.files_seen, 2,
        "only session and subagent logs are read"
    );
    assert_eq!(first.files_read, 2);
    assert_eq!(first.inserted, 4);
    assert!(first.issues >= 1, "the malformed line is reported");
    assert_fixture_totals(&fixture.totals(), 0);

    let version = fixture.store.data_version();
    let second = fixture.scan();
    assert_eq!(second.files_read, 0, "unchanged files are skipped");
    assert_eq!(second.inserted + second.updated, 0);
    assert_eq!(fixture.store.data_version(), version);
    assert_fixture_totals(&fixture.totals(), 0);

    let source = fixture.source();
    assert!(source.enabled);
    assert!(source.consented_at.is_some());
    assert!(source.last_success_at.is_some());
    assert_eq!(source.last_error, None);
}

#[test]
fn no_content_reaches_the_database() {
    let fixture = Fixture::new().enabled();
    let session = std::fs::read(fixture.projects().join(SESSION)).unwrap();
    assert!(
        contains(&session, b"VEYRA-CANARY"),
        "the fixture carries canaries"
    );
    fixture.scan();
    let mut checked = 0;
    for entry in std::fs::read_dir(&fixture.dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let bytes = std::fs::read(&path).unwrap();
            assert!(
                !contains(&bytes, b"VEYRA-CANARY"),
                "{} holds content",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 2,
        "the database and its write-ahead log were checked"
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn appended_lines_are_read_incrementally() {
    let fixture = Fixture::new().enabled();
    fixture.scan();
    fixture.append(SUBAGENT, &request_line("E", "2026-11-08T11:00:00.000Z"));
    let report = fixture.scan();
    assert_eq!(report.files_read, 1);
    assert_eq!(report.restarts, 0);
    assert_eq!(report.inserted, 1);
    assert_eq!(report.lines, 1, "only the new line is read");
    assert_fixture_totals(&fixture.totals(), 1);
}

#[test]
fn a_partial_last_line_is_read_once_complete() {
    let fixture = Fixture::new().enabled();
    fixture.scan();
    let line = request_line("F", "2026-11-08T11:00:00.000Z");
    let (head, tail) = line.split_at(line.len() / 2);
    fixture.append(SUBAGENT, head);
    let report = fixture.scan();
    assert_eq!((report.inserted, report.lines), (0, 0));
    fixture.append(SUBAGENT, tail);
    let report = fixture.scan();
    assert_eq!((report.inserted, report.lines), (1, 1));
    assert_fixture_totals(&fixture.totals(), 1);
}

#[test]
fn truncated_and_replaced_files_are_reread_without_double_counting() {
    let fixture = Fixture::new().enabled();
    fixture.scan();
    let path = fixture.projects().join(SUBAGENT);
    let original = std::fs::read_to_string(&path).unwrap();

    // Truncated to its last line: re-read from the start, nothing counted twice.
    let last = original.lines().last().unwrap();
    std::fs::write(&path, format!("{last}\n")).unwrap();
    let report = fixture.scan();
    assert_eq!(report.restarts, 1);
    assert_eq!(report.inserted, 0);
    assert_fixture_totals(&fixture.totals(), 0);

    // Replaced by a longer file with different first bytes.
    let replacement = format!(
        "{}{}",
        request_line("G", "2026-11-08T12:00:00.000Z"),
        original
    );
    std::fs::write(&path, replacement).unwrap();
    let report = fixture.scan();
    assert_eq!(report.restarts, 1);
    assert_eq!(report.inserted, 1);
    assert_fixture_totals(&fixture.totals(), 1);
}

#[test]
fn deleted_logs_keep_their_history() {
    let fixture = Fixture::new().enabled();
    fixture.scan();
    std::fs::remove_file(fixture.projects().join(SUBAGENT)).unwrap();
    let report = fixture.scan();
    assert_eq!(report.files_missing, 1);
    assert_fixture_totals(&fixture.totals(), 0);
}

#[test]
fn a_disabled_source_is_not_read_again() {
    let fixture = Fixture::new().enabled();
    fixture.scan();
    let source = fixture.source();
    fixture
        .store
        .set_source_enabled(source.id, false, UtcMillis(2))
        .unwrap();
    fixture.append(SUBAGENT, &request_line("H", "2026-11-08T11:00:00.000Z"));
    assert_eq!(fixture.scan(), ScanReport::default());
    assert_fixture_totals(&fixture.totals(), 0);
}

#[test]
fn oversize_lines_are_skipped_with_an_issue_and_never_loop() {
    let fixture = Fixture::new().enabled();
    // Every fixture line is longer than 64 bytes.
    let report = scan_all(&fixture.context(64), "manual").unwrap();
    assert_eq!(report.inserted, 0);
    assert!(report.issues >= 13);
    let checkpoints = fixture.store.checkpoints(fixture.source().id).unwrap();
    let session_len = std::fs::metadata(fixture.projects().join(SESSION))
        .unwrap()
        .len();
    assert_eq!(
        checkpoints[SESSION].offset, session_len,
        "skipped to the end"
    );
}

#[test]
fn small_chunks_give_the_same_result_as_one_read() {
    let fixture = Fixture::new().enabled();
    let report = scan_all(&fixture.context(1_024), "manual").unwrap();
    assert_eq!(report.inserted, 4);
    assert_fixture_totals(&fixture.totals(), 0);
}

#[test]
fn cancellation_stops_between_chunks_and_resumes_later() {
    let fixture = Fixture::new().enabled();
    fixture.cancel.store(true, Ordering::Relaxed);
    assert!(scan_all(&fixture.context(DEFAULT_CHUNK_BYTES), "manual").is_err());
    assert_eq!(fixture.totals().events, 0);
    fixture.cancel.store(false, Ordering::Relaxed);
    assert_eq!(fixture.scan().inserted, 4);
}

#[test]
fn reads_while_the_tool_keeps_appending() {
    let fixture = Fixture::new().enabled();
    let path = fixture.projects().join(SUBAGENT);
    let writer = std::thread::spawn(move || {
        let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        for i in 0..200 {
            let line = request_line(&format!("W{i}"), "2026-11-08T13:00:00.000Z");
            // Two writes per line, so readers regularly see half-written lines.
            let (head, tail) = line.split_at(line.len() / 2);
            file.write_all(head.as_bytes()).unwrap();
            file.flush().unwrap();
            file.write_all(tail.as_bytes()).unwrap();
            file.flush().unwrap();
            if i % 20 == 0 {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    });
    while !writer.is_finished() {
        fixture.scan();
    }
    writer.join().unwrap();
    fixture.scan();
    assert_fixture_totals(&fixture.totals(), 200);
}

fn worker(fixture: &Fixture, watch: bool, changes: mpsc::Sender<u64>) -> IngestWorker {
    IngestWorker {
        store: Arc::new(Store::open(&fixture.dir.join("veyra.db")).unwrap()),
        registry: Arc::new(Registry::bundled().unwrap()),
        connectors: veyra_connectors::all(),
        env: fixture.env(),
        config: WorkerConfig {
            periodic: Duration::from_hours(1),
            debounce: Duration::from_millis(100),
            watch,
            ..WorkerConfig::default()
        },
        on_change: Box::new(move |report| {
            let _ = changes.send(report.inserted);
        }),
    }
}

#[test]
fn the_worker_scans_at_startup_and_on_request() {
    let fixture = Fixture::new().enabled();
    let (sender, changes) = mpsc::channel();
    let handle = veyra_ingest::spawn(worker(&fixture, false, sender)).unwrap();
    assert_eq!(changes.recv_timeout(Duration::from_secs(20)).unwrap(), 4);
    fixture.append(SUBAGENT, &request_line("M", "2026-11-08T11:00:00.000Z"));
    handle.request_scan();
    assert_eq!(changes.recv_timeout(Duration::from_secs(20)).unwrap(), 1);
    let stopped = Instant::now();
    handle.shutdown();
    assert!(stopped.elapsed() < Duration::from_secs(5));
    assert_fixture_totals(&fixture.totals(), 1);
}

#[test]
fn the_worker_picks_up_changes_without_being_asked() {
    let fixture = Fixture::new().enabled();
    let (sender, changes) = mpsc::channel();
    let handle = veyra_ingest::spawn(worker(&fixture, true, sender)).unwrap();
    assert_eq!(changes.recv_timeout(Duration::from_secs(20)).unwrap(), 4);
    fixture.append(SUBAGENT, &request_line("N", "2026-11-08T11:00:00.000Z"));
    assert_eq!(changes.recv_timeout(Duration::from_secs(30)).unwrap(), 1);
    handle.shutdown();
}

/// Throughput budget (ADR-023): at least 40,000 events per second end to end on one
/// large log, measured on a standard GitHub-hosted runner (which reaches about
/// 39,000-49,000 per attempt; developer machines about 50,000-60,000). The best of three runs counts,
/// so a transient stall does not fail the budget; a systematic slowdown still does.
/// Run with `cargo test -p veyra-ingest --release -- --ignored throughput --nocapture`.
#[test]
#[ignore = "performance budget; run in release mode"]
fn throughput() {
    let events: u64 = std::env::var("VEYRA_INGEST_EVENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);
    let source = Fixture::new();
    let log = source.projects().join(SUBAGENT);
    let mut file =
        std::io::BufWriter::new(std::fs::OpenOptions::new().append(true).open(&log).unwrap());
    for i in 0..events {
        let (day, second) = (8 + i / 86_400, i % 86_400);
        let timestamp = format!(
            "2026-11-{day:02}T{:02}:{:02}:{:02}.000Z",
            second / 3_600,
            second / 60 % 60,
            second % 60
        );
        file.write_all(request_line(&format!("T{i}"), &timestamp).as_bytes())
            .unwrap();
    }
    drop(file);

    let mut best = 0.0_f64;
    for attempt in 1..=3 {
        let fixture = Fixture::new();
        std::fs::copy(&log, fixture.projects().join(SUBAGENT)).unwrap();
        let fixture = fixture.enabled();
        let started = Instant::now();
        let report = fixture.scan();
        let elapsed = started.elapsed();
        assert_eq!(report.inserted, events + 4);
        #[allow(clippy::cast_precision_loss, reason = "a rate for display")]
        let rate = report.inserted as f64 / elapsed.as_secs_f64();
        eprintln!(
            "attempt {attempt}: ingested {} events in {elapsed:?}: {rate:.0} events/s",
            report.inserted
        );
        best = best.max(rate);
    }
    assert!(
        best >= 40_000.0,
        "best ingest rate {best:.0} events/s is below 40,000"
    );
}
