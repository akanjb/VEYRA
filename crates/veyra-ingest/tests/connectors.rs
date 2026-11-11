//! End-to-end ingestion of the connectors read in full or through snapshots, from their
//! synthetic fixtures: discovery, consent, totals, idempotent rescans and content safety.

#![expect(
    clippy::unwrap_used,
    reason = "test setup failures should abort the test"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader, LabelReader};
use veyra_analytics::sums::BaseSums;
use veyra_domain::source::{DiscoveryEnv, SourceConnector};
use veyra_domain::time::UtcMillis;
use veyra_ingest::{DEFAULT_CHUNK_BYTES, IngestContext, ScanReport, discover, scan_all};
use veyra_registry::Registry;
use veyra_store::Store;
use veyra_testkit::{TempDir, codex_home, copilot_home, gemini_tmp, opencode_database};

/// A temporary home holding one connector's fixtures, plus a database.
struct Fixture {
    dir: TempDir,
    home: PathBuf,
    store: Store,
    registry: Registry,
    connectors: Vec<Box<dyn SourceConnector>>,
    cancel: AtomicBool,
}

impl Fixture {
    /// Copies `fixture` to `<home>/<at>`, discovers it and enables it.
    fn enabled(fixture: &Path, at: &str) -> Self {
        Self::prepared(|dir| dir.with_home_copy(fixture, at))
    }

    /// Lets `prepare` build a home directory, then discovers and enables its sources.
    fn prepared(prepare: impl FnOnce(&TempDir) -> PathBuf) -> Self {
        let dir = TempDir::new("veyra-connectors-test");
        let home = prepare(&dir);
        let store = Store::open(&dir.path().join("veyra.db")).unwrap();
        let this = Self {
            dir,
            home,
            store,
            registry: Registry::bundled().unwrap(),
            connectors: veyra_connectors::all(),
            cancel: AtomicBool::new(false),
        };
        let env = DiscoveryEnv {
            home: Some(this.home.clone()),
            location_overrides: BTreeMap::new(),
        };
        for source in discover(&this.store, &this.connectors, &env).unwrap() {
            this.store
                .set_source_enabled(source.id, true, UtcMillis(1))
                .unwrap();
        }
        this
    }

    fn scan(&self) -> ScanReport {
        let context = IngestContext {
            store: &self.store,
            registry: &self.registry,
            connectors: &self.connectors,
            cancel: &self.cancel,
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            throttle: None,
        };
        scan_all(&context, "manual").unwrap()
    }

    fn totals(&self) -> BaseSums {
        let rows = self.grouped(vec![]);
        BaseSums::total(rows.iter().map(|(_, sums)| sums))
    }

    /// Events grouped by `dimensions`: each group's values and sums.
    fn grouped(&self, dimensions: Vec<Dimension>) -> Vec<(Vec<Option<String>>, BaseSums)> {
        self.store
            .read_base_sums(&BaseSumsQuery {
                from: UtcMillis(0),
                until: UtcMillis(4_000_000_000_000),
                filters: Filters::new(),
                group_by: dimensions,
                by_bucket: false,
            })
            .unwrap()
            .into_iter()
            .map(|row| (row.key, row.sums))
            .collect()
    }

    /// The project folders events were attributed to.
    fn projects(&self) -> Vec<String> {
        let mut paths: Vec<String> = self
            .store
            .projects()
            .unwrap()
            .into_iter()
            .map(|p| p.path)
            .collect();
        paths.sort();
        paths
    }

    /// No `VEYRA-CANARY` marker in any file of the database directory.
    fn assert_no_content_stored(&self) {
        let mut checked = 0;
        for entry in std::fs::read_dir(self.dir.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let bytes = std::fs::read(&path).unwrap();
                assert!(
                    !bytes.windows(12).any(|w| w == b"VEYRA-CANARY"),
                    "{} holds content",
                    path.display()
                );
                checked += 1;
            }
        }
        assert!(checked >= 1, "the database was checked");
    }
}

mod codex {
    use super::{Fixture, codex_home};

    const LEGACY: &str = "sessions/2026/11/08/rollout-2026-11-08T11-00-00-0190aaaa-0000-7000-8000-000000000002.jsonl";

    fn fixture() -> Fixture {
        Fixture::enabled(&codex_home(), ".codex")
    }

    /// Seven responses: two in the current session, two increases in the older one, two
    /// in its fork and one in the archived subagent thread.
    fn assert_totals(fixture: &Fixture) {
        let totals = fixture.totals();
        assert_eq!(totals.events, 7);
        assert_eq!(totals.tokens.output_total, Some(170 + 300 + 110 + 40));
    }

    #[test]
    fn ingests_each_response_once_and_a_rescan_reads_nothing() {
        let fixture = fixture();
        let first = fixture.scan();
        assert_eq!(first.files_read, 4, "rollouts only: {first:?}");
        assert_eq!(first.issues, 1, "the malformed line");
        assert_totals(&fixture);

        let again = fixture.scan();
        assert_eq!((again.files_read, again.inserted, again.updated), (0, 0, 0));
        assert_totals(&fixture);
        fixture.assert_no_content_stored();
    }

    #[test]
    fn a_compressed_archived_copy_adds_nothing() {
        let fixture = fixture();
        fixture.scan();
        let codex = fixture.home.join(".codex");
        let plain = std::fs::read(codex.join(LEGACY)).unwrap();
        let name = LEGACY.rsplit('/').next().unwrap();
        let compressed = ruzstd::encoding::compress_to_vec(
            &plain[..],
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        std::fs::write(
            codex.join("archived_sessions").join(format!("{name}.zst")),
            compressed,
        )
        .unwrap();

        let report = fixture.scan();
        assert_eq!(report.files_read, 1, "the archive is read");
        assert_eq!(report.inserted, 0, "{report:?}");
        assert_totals(&fixture);
    }

    #[test]
    fn a_changed_rollout_is_reread_and_only_its_new_usage_is_added() {
        let fixture = fixture();
        fixture.scan();
        let path = fixture.home.join(".codex").join(LEGACY);
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(
            "{\"timestamp\":\"2026-11-08T11:02:00.000Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":3500,\"cached_input_tokens\":1000,\"output_tokens\":320,\"reasoning_output_tokens\":50,\"total_tokens\":3820}},\"rate_limits\":null}}\n",
        );
        std::fs::write(&path, text).unwrap();

        let report = fixture.scan();
        assert_eq!((report.files_read, report.inserted), (1, 1), "{report:?}");
        assert_eq!(fixture.totals().events, 8);
    }
}

mod gemini_cli {
    use super::{Fixture, gemini_tmp};

    fn fixture() -> Fixture {
        Fixture::enabled(&gemini_tmp(), ".gemini/tmp")
    }

    #[test]
    fn ingests_each_response_once_across_old_and_resumed_copies() {
        let fixture = fixture();
        let report = fixture.scan();
        assert_eq!(report.files_read, 4, "recordings only: {report:?}");
        assert_eq!(report.issues, 1, "the malformed line");
        // g1, g2 and g3; h1 (in the old document and its resumed copy) and h2; s1.
        let totals = fixture.totals();
        assert_eq!(totals.events, 6);
        assert_eq!(totals.tokens.reasoning, Some(150 + 30));
        assert_eq!(fixture.projects(), ["/home/dev/shop"], "from .project_root");

        let again = fixture.scan();
        assert_eq!((again.files_read, again.inserted, again.updated), (0, 0, 0));
        assert_eq!(fixture.totals().events, 6);
        fixture.assert_no_content_stored();
    }
}

mod opencode {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{Fixture, opencode_database};

    const LATE_MESSAGE: &str = r#"INSERT INTO session_message VALUES ('msg_late', 'ses_new',
        'assistant', 9, '{"model":{"id":"gpt-5.5","providerID":"openai"},"tokens":{"input":10,"output":5,"reasoning":0,"cache":{"read":0,"write":0}},"time":{"created":1794148300000,"completed":1794148310000}}',
        1794148300000, 1794148310000)"#;

    fn fixture() -> Fixture {
        Fixture::prepared(|dir| {
            let home = dir.path().join("home");
            let _ = opencode_database(&home.join(".local/share/opencode"));
            home
        })
    }

    fn database(fixture: &Fixture) -> PathBuf {
        fixture.home.join(".local/share/opencode/opencode.db")
    }

    /// Six messages: the imported 1.x message (counted once), the Copilot one, a
    /// response and a compaction in the 2.2 session, and one each in the subagent
    /// session and the fork.
    fn assert_totals(fixture: &Fixture) {
        let totals = fixture.totals();
        assert_eq!(totals.events, 6);
        assert_eq!(
            totals.tokens.output_total,
            Some(60 + 40 + 120 + 300 + 20 + 10)
        );
    }

    #[test]
    fn ingests_each_message_once_from_both_table_generations() {
        let fixture = fixture();
        let report = fixture.scan();
        assert_eq!((report.files_read, report.issues), (1, 0), "{report:?}");
        assert_totals(&fixture);
        let projects = fixture.projects();
        assert!(
            projects.contains(&"/home/dev/api".to_owned()),
            "{projects:?}"
        );

        let again = fixture.scan();
        assert_eq!((again.files_read, again.inserted), (0, 0), "unchanged");
        assert_totals(&fixture);
        fixture.assert_no_content_stored();
    }

    /// OpenCode keeps writing, with a busy timeout a fiftieth of its own, while Veyra reads its
    /// database: no write is refused, and the next scan picks up the new message.
    #[test]
    fn never_blocks_opencode_and_reads_its_new_messages() {
        let fixture = fixture();
        let stop = Arc::new(AtomicBool::new(false));
        let writer = {
            let (path, stop) = (database(&fixture), Arc::clone(&stop));
            std::thread::spawn(move || {
                let db = rusqlite::Connection::open(&path).unwrap();
                db.busy_timeout(veyra_testkit::OWNER_BUSY_TIMEOUT).unwrap();
                let mut written = 0_u32;
                while !stop.load(Ordering::Relaxed) {
                    written += 1;
                    db.execute(
                        "INSERT INTO event VALUES (?1, '{}')",
                        [format!("evt_w{written}")],
                    )
                    .expect("a write was refused while Veyra was reading");
                }
                db.execute(LATE_MESSAGE, []).unwrap();
                written
            })
        };
        for _ in 0..5 {
            fixture.scan();
        }
        stop.store(true, Ordering::Relaxed);
        assert!(writer.join().unwrap() > 0, "the writer kept writing");

        fixture.scan();
        assert_eq!(fixture.totals().events, 7, "the new message was read");
    }
}

mod copilot_cli {
    use veyra_analytics::dimension::Dimension;

    use super::{Fixture, copilot_home};

    fn fixture() -> Fixture {
        Fixture::enabled(&copilot_home(), ".copilot")
    }

    #[test]
    fn ingests_session_summaries_once() {
        let fixture = fixture();
        let report = fixture.scan();
        assert_eq!(report.files_read, 3, "event logs only: {report:?}");
        assert_eq!(report.issues, 1, "the malformed line");
        let totals = fixture.totals();
        // Two runs of the new session (one model each) and two of the old one.
        assert_eq!(totals.events, 4);
        assert_eq!(totals.requests, Some(5 + 2 + 1 + 2));

        let again = fixture.scan();
        assert_eq!((again.files_read, again.inserted), (0, 0));
        fixture.assert_no_content_stored();
    }

    /// Claude used through Copilot is Anthropic usage, routed by GitHub Copilot, in the
    /// Copilot CLI application.
    #[test]
    fn attributes_vendor_gateway_and_application_separately() {
        let fixture = fixture();
        fixture.scan();
        let groups = fixture.grouped(vec![
            Dimension::Provider,
            Dimension::Gateway,
            Dimension::App,
        ]);
        let mut keys: Vec<Vec<Option<String>>> = groups.into_iter().map(|(key, _)| key).collect();
        keys.sort();
        let row = |provider: &str| {
            vec![
                Some(provider.to_owned()),
                Some("github-copilot".to_owned()),
                Some("copilot-cli".to_owned()),
            ]
        };
        assert_eq!(keys, [row("anthropic"), row("openai")]);
    }
}
