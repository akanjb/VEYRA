//! The local capture proxy (ADR-039) over real loopback sockets, with a fake provider
//! in place of the HTTPS upstream: responses are relayed unchanged, usage is stored once
//! per request, and requests that are not local are refused.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use veyra_analytics::dimension::Filters;
use veyra_analytics::port::{BaseSumsQuery, BaseSumsReader};
use veyra_analytics::sums::BaseSums;
use veyra_connectors::capture::{Route, routes};
use veyra_domain::capture::{CaptureProxy, CaptureRoute, UsageScan};
use veyra_domain::source::ConnectorDescriptor;
use veyra_domain::time::UtcMillis;
use veyra_ingest::{Proxy, ProxyParts};
use veyra_registry::Registry;
use veyra_store::Store;
use veyra_testkit::TempDir;

/// A real route whose provider is the fake one.
struct Local {
    route: Route,
    upstream: &'static str,
}

impl CaptureRoute for Local {
    fn descriptor(&self) -> &ConnectorDescriptor {
        self.route.descriptor()
    }
    fn prefix(&self) -> &'static str {
        self.route.prefix()
    }
    fn upstream(&self) -> &'static str {
        self.upstream
    }
    fn client_suffix(&self) -> &'static str {
        self.route.client_suffix()
    }
    fn records(&self, method: &str, path: &str) -> bool {
        self.route.records(method, path)
    }
    fn scanner(&self, content_type: Option<&str>) -> Box<dyn UsageScan> {
        self.route.scanner(content_type)
    }
}

/// What the fake provider received: the request line and lower-case headers.
type Seen = Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>;

/// A provider that answers every request with `response`, recording what it got.
fn provider(response: String) -> (u16, Seen, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Seen = Arc::default();
    let record = Arc::clone(&seen);
    let thread = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.is_empty() {
                return;
            }
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }
                let (name, value) = header.split_once(':').unwrap();
                headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
            }
            let length: usize = headers
                .iter()
                .find(|(n, _)| n == "content-length")
                .map_or(0, |(_, v)| v.parse().unwrap());
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            record
                .lock()
                .unwrap()
                .push((line.trim_end().to_owned(), headers));
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (port, seen, thread)
}

const STREAM: &str = concat!(
    "event: message_start\n",
    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-5\",",
    "\"usage\":{\"input_tokens\":12,\"cache_read_input_tokens\":3000,\"output_tokens\":1}}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hello\"}}\n\n",
    "event: message_delta\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":250}}\n\n",
);

fn reply(status: &str, extra: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

struct Fixture {
    _dir: TempDir,
    store: Arc<Store>,
    proxy: Proxy,
}

impl Fixture {
    fn new(upstream_port: u16) -> Self {
        let dir = TempDir::new("veyra-proxy-test");
        let store = Arc::new(Store::open(&dir.path().join("veyra.db")).unwrap());
        let upstream: &'static str = format!("http://127.0.0.1:{upstream_port}").leak();
        let routes = routes()
            .into_iter()
            .map(|route| Box::new(Local { route, upstream }) as Box<dyn CaptureRoute>)
            .collect();
        let proxy = Proxy::new(
            ProxyParts {
                store: Arc::clone(&store),
                registry: Arc::new(Registry::bundled().unwrap()),
                routes,
                on_change: Box::new(|_| {}),
            },
            0,
        );
        proxy.set_enabled(true);
        Self {
            _dir: dir,
            store,
            proxy,
        }
    }

    fn port(&self) -> u16 {
        let endpoint = self.proxy.status().endpoint;
        endpoint.rsplit(':').next().unwrap().parse().unwrap()
    }

    /// Sends a request and returns the status code and the de-chunked body.
    fn send(&self, head: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port())).unwrap();
        write!(
            stream,
            "{head}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let raw = String::from_utf8(raw).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let code = head.split(' ').nth(1).unwrap().parse().unwrap();
        let body = if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            dechunk(body)
        } else {
            body.to_owned()
        };
        (code, body)
    }

    fn post(&self, path: &str, extra: &str) -> (u16, String) {
        let port = self.port();
        self.send(
            &format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}Content-Type: application/json"),
            r#"{"model":"claude-sonnet-4-5","stream":true}"#,
        )
    }

    fn totals(&self) -> BaseSums {
        let rows = self
            .store
            .read_base_sums(&BaseSumsQuery {
                from: UtcMillis(0),
                until: UtcMillis(i64::MAX / 2),
                filters: Filters::new(),
                group_by: vec![],
                by_bucket: false,
            })
            .unwrap();
        BaseSums::total(rows.iter().map(|r| &r.sums))
    }
}

fn dechunk(mut body: &str) -> String {
    let mut out = String::new();
    loop {
        let (size, rest) = body.split_once("\r\n").unwrap();
        let size = usize::from_str_radix(size, 16).unwrap();
        if size == 0 {
            return out;
        }
        out.push_str(&rest[..size]);
        body = &rest[size + 2..];
    }
}

#[test]
fn relays_a_stream_unchanged_and_stores_its_usage_once() {
    let response = reply(
        "200 OK",
        "Content-Type: text/event-stream\r\nrequest-id: req_011XYZ\r\n",
        STREAM,
    );
    let (upstream, seen, _provider) = provider(response);
    let fixture = Fixture::new(upstream);
    for _ in 0..2 {
        let (code, body) = fixture.post(
            "/anthropic/v1/messages?beta=true",
            "x-api-key: sk-test\r\nAccept-Encoding: gzip\r\n",
        );
        assert_eq!(code, 200);
        assert_eq!(body, STREAM, "relayed byte for byte");
    }

    let seen = seen.lock().unwrap();
    let (line, headers) = &seen[0];
    assert_eq!(line, "POST /v1/messages?beta=true HTTP/1.1");
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(header("x-api-key"), Some("sk-test"), "keys pass through");
    assert_eq!(header("accept-encoding"), Some("identity"));
    assert_eq!(
        header("host"),
        Some(format!("127.0.0.1:{upstream}").as_str())
    );

    let totals = fixture.totals();
    assert_eq!(totals.events, 1, "the same request id is one request");
    assert_eq!(totals.tokens.output_total, Some(250));
    assert_eq!(totals.tokens.cache_read, Some(3000));
    let status = fixture.proxy.status();
    assert_eq!((status.relayed, status.records), (2, 2));
    assert_eq!(status.failure, None);
}

#[test]
fn relays_other_requests_without_recording_them() {
    let (upstream, _seen, _provider) = provider(reply(
        "200 OK",
        "Content-Type: application/json\r\n",
        r#"{"data":[]}"#,
    ));
    let fixture = Fixture::new(upstream);
    let port = fixture.port();
    let (code, body) = fixture.send(
        &format!("GET /openai/v1/models HTTP/1.1\r\nHost: localhost:{port}"),
        "",
    );
    assert_eq!((code, body.as_str()), (200, r#"{"data":[]}"#));
    assert_eq!(fixture.totals().events, 0);
    assert_eq!(fixture.proxy.status().records, 0);
}

#[test]
fn provider_errors_are_relayed_and_record_nothing() {
    let error = r#"{"type":"error","error":{"type":"authentication_error"}}"#;
    let (upstream, _seen, _provider) = provider(reply(
        "401 Unauthorized",
        "Content-Type: application/json\r\nrequest-id: req_1\r\n",
        error,
    ));
    let fixture = Fixture::new(upstream);
    assert_eq!(
        fixture.post("/anthropic/v1/messages", ""),
        (401, error.to_owned())
    );
    assert_eq!(fixture.totals().events, 0);
}

#[test]
fn refuses_web_pages_other_hosts_and_unknown_providers() {
    let (upstream, seen, _provider) = provider(reply("200 OK", "", "{}"));
    let fixture = Fixture::new(upstream);
    let port = fixture.port();
    assert_eq!(
        fixture
            .post("/anthropic/v1/messages", "Origin: https://example.com\r\n")
            .0,
        403
    );
    let (code, _) = fixture.send(
        &format!("POST /anthropic/v1/messages HTTP/1.1\r\nHost: attacker.example:{port}"),
        "{}",
    );
    assert_eq!(code, 403);
    assert_eq!(fixture.post("/somewhere/v1/chat", "").0, 404);
    assert!(seen.lock().unwrap().is_empty(), "nothing was forwarded");
}

#[test]
fn reports_a_provider_that_cannot_be_reached() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let fixture = Fixture::new(port);
    assert_eq!(fixture.post("/openai/v1/chat/completions", "").0, 502);
    assert_eq!(
        fixture.proxy.status().failure.as_deref(),
        Some("the provider could not be reached")
    );
}

#[test]
fn stops_listening_when_turned_off() {
    let fixture = Fixture::new(1);
    let port = fixture.port();
    assert!(fixture.proxy.status().listening);
    fixture.proxy.set_enabled(false);
    assert!(!fixture.proxy.status().listening);
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}
