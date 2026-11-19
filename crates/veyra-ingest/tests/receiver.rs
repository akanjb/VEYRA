//! The local OpenTelemetry receiver (ADR-038) over a real loopback socket: usage pushed
//! by Claude Code is stored once, alongside the same requests read from its logs, and
//! requests that are not plain local OTLP JSON exports are refused.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::sums::BaseSums;
use veyra_domain::source::DiscoveryEnv;
use veyra_domain::telemetry::TelemetryReceiver;
use veyra_domain::time::UtcMillis;
use veyra_ingest::{
    DEFAULT_CHUNK_BYTES, IngestContext, Receiver, ReceiverParts, discover, scan_all,
};
use veyra_registry::Registry;
use veyra_store::Store;
use veyra_testkit::TempDir;

const SESSION: &str = "11111111-1111-4111-8111-111111111111";

struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    changes: Arc<AtomicU64>,
    receiver: Receiver,
}

impl Fixture {
    /// A receiver on a port the system chooses, writing to a fresh database.
    fn new() -> Self {
        Self::on_port(0)
    }

    fn on_port(port: u16) -> Self {
        let dir = TempDir::new("veyra-receiver-test");
        let store = Arc::new(Store::open(&dir.path().join("veyra.db")).unwrap());
        let changes = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&changes);
        let receiver = Receiver::new(
            ReceiverParts {
                store: Arc::clone(&store),
                registry: Arc::new(Registry::bundled().unwrap()),
                connectors: veyra_connectors::all(),
                on_change: Box::new(move |_| {
                    counter.fetch_add(1, Ordering::Relaxed);
                }),
            },
            port,
        );
        Self {
            dir,
            store,
            changes,
            receiver,
        }
    }

    fn port(&self) -> u16 {
        let endpoint = self.receiver.status().endpoint;
        endpoint.rsplit(':').next().unwrap().parse().unwrap()
    }

    /// Sends a raw request and returns the response status code.
    fn send(&self, head: &str, body: &str) -> u16 {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port())).unwrap();
        write!(
            stream,
            "{head}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response[9..12].parse().unwrap()
    }

    fn post(&self, body: &str) -> u16 {
        let head = format!(
            "POST /v1/logs HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json",
            self.port()
        );
        self.send(&head, body)
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

    /// Reads the synthetic Claude Code logs, as the file connector does.
    fn scan_logs(&self) {
        let _home = self.dir.with_claude_code_home();
        let connectors = veyra_connectors::all();
        let env = DiscoveryEnv {
            home: Some(self.dir.path().join("home")),
            location_overrides: BTreeMap::new(),
        };
        for source in discover(&self.store, &connectors, &env).unwrap() {
            self.store
                .set_source_enabled(source.id, true, UtcMillis(1))
                .unwrap();
        }
        let registry = Registry::bundled().unwrap();
        let cancel = AtomicBool::new(false);
        let context = IngestContext {
            store: &self.store,
            registry: &registry,
            connectors: &connectors,
            cancel: &cancel,
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            throttle: None,
        };
        scan_all(&context, "manual").unwrap();
    }
}

/// A Claude Code log export holding one `api_request` event for `request`.
fn export(request: &str) -> String {
    format!(
        r#"{{"resourceLogs":[{{
            "resource":{{"attributes":[{{"key":"service.name","value":{{"stringValue":"claude-code"}}}}]}},
            "scopeLogs":[{{"logRecords":[{{"attributes":[
                {{"key":"event.name","value":{{"stringValue":"api_request"}}}},
                {{"key":"event.timestamp","value":{{"stringValue":"2026-11-08T10:00:03.000Z"}}}},
                {{"key":"request_id","value":{{"stringValue":"{request}"}}}},
                {{"key":"session.id","value":{{"stringValue":"{SESSION}"}}}},
                {{"key":"model","value":{{"stringValue":"claude-opus-5-5"}}}},
                {{"key":"input_tokens","value":{{"intValue":"4"}}}},
                {{"key":"output_tokens","value":{{"intValue":"120"}}}},
                {{"key":"cache_read_tokens","value":{{"intValue":"10000"}}}},
                {{"key":"cache_creation_tokens","value":{{"intValue":"2000"}}}}
            ]}}]}}]}}]}}"#
    )
}

#[test]
fn stores_pushed_usage_once_per_request() {
    let fixture = Fixture::new();
    fixture.receiver.set_enabled(true);
    assert!(fixture.receiver.status().listening);

    assert_eq!(fixture.post(&export("req_X")), 200);
    assert_eq!(fixture.post(&export("req_X")), 200);
    let totals = fixture.totals();
    assert_eq!(totals.events, 1);
    assert_eq!(totals.tokens.output_total, Some(120));
    assert_eq!(totals.events_unpriced, 0);

    let status = fixture.receiver.status();
    assert_eq!(status.exports, 2);
    assert_eq!(status.records, 2);
    assert!(status.last_received_at.is_some());
    assert_eq!(
        fixture.changes.load(Ordering::Relaxed),
        1,
        "the repeat changed nothing"
    );
}

#[test]
fn a_request_in_both_the_log_and_telemetry_counts_once() {
    let fixture = Fixture::new();
    fixture.scan_logs();
    let before = fixture.totals();
    fixture.receiver.set_enabled(true);
    assert_eq!(fixture.post(&export("req_A")), 200);
    assert_eq!(fixture.totals(), before);
}

#[test]
fn ignores_records_of_unknown_tools_and_events() {
    let fixture = Fixture::new();
    fixture.receiver.set_enabled(true);
    let other_tool = export("req_X").replace("claude-code", "some-tool");
    let other_event = export("req_Y").replace("api_request", "user_prompt");
    assert_eq!(fixture.post(&other_tool), 200);
    assert_eq!(fixture.post(&other_event), 200);
    assert_eq!(fixture.totals().events, 0);
    let status = fixture.receiver.status();
    assert_eq!((status.exports, status.records), (2, 0));
}

#[test]
fn refuses_anything_but_local_json_log_exports() {
    let fixture = Fixture::new();
    fixture.receiver.set_enabled(true);
    let port = fixture.port();
    let host = format!("Host: 127.0.0.1:{port}");
    let json = "Content-Type: application/json";
    let cases = [
        (
            format!("POST /v1/logs HTTP/1.1\r\n{host}\r\n{json}\r\nOrigin: https://example.com"),
            403,
        ),
        (
            format!("POST /v1/logs HTTP/1.1\r\nHost: attacker.example:{port}\r\n{json}"),
            403,
        ),
        (
            format!("POST /v1/logs HTTP/1.1\r\n{host}\r\nContent-Type: application/x-protobuf"),
            415,
        ),
        (
            format!("POST /v1/logs HTTP/1.1\r\n{host}\r\n{json}\r\nContent-Encoding: gzip"),
            415,
        ),
        (
            format!("POST /v1/metrics HTTP/1.1\r\n{host}\r\n{json}"),
            404,
        ),
        (format!("GET /v1/logs HTTP/1.1\r\n{host}"), 405),
    ];
    for (head, status) in cases {
        assert_eq!(fixture.send(&head, &export("req_X")), status, "{head}");
    }
    assert_eq!(fixture.post("not json"), 400);
    assert_eq!(fixture.totals().events, 0);
    let status = fixture.receiver.status();
    assert_eq!(status.exports, 0);
    assert_eq!(
        status.refusal.as_deref(),
        Some("the body is not an OTLP JSON log export (HTTP 400)")
    );
    assert_eq!(fixture.post(&export("req_X")), 200);
    assert_eq!(
        fixture.receiver.status().refusal,
        None,
        "cleared once one is accepted"
    );
}

#[test]
fn stops_listening_when_turned_off() {
    let fixture = Fixture::new();
    fixture.receiver.set_enabled(true);
    let port = fixture.port();
    fixture.receiver.set_enabled(false);
    let status = fixture.receiver.status();
    assert!(!status.listening);
    assert_eq!(status.error, None);
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[test]
fn reports_a_port_that_is_taken() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let fixture = Fixture::on_port(taken.local_addr().unwrap().port());
    fixture.receiver.set_enabled(true);
    let status = fixture.receiver.status();
    assert!(!status.listening);
    assert!(
        status
            .error
            .unwrap()
            .starts_with("Cannot listen on 127.0.0.1:")
    );
}
