//! The local capture proxy (ADR-039).
//!
//! - Off until the user turns it on; it then listens on 127.0.0.1 only. Requests from
//!   web pages (with an `Origin` header) and requests naming another host are refused,
//!   so no page can use it as a relay.
//! - A request to `/<prefix>/<path>` is forwarded unchanged, over HTTPS, to the provider
//!   of that route. API keys pass through in headers; nothing about a request is
//!   stored or logged, and bodies are never stored.
//! - Responses are relayed as they arrive. For generation requests, the route's scanner
//!   reads the usage on the way; it is normalised, priced and stored like any other
//!   usage.
//! - Each connection is answered on its own thread, up to [`MAX_CONNECTIONS`]; a
//!   request in progress when the proxy is turned off is allowed to finish.

use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT_ENCODING, HeaderMap, HeaderName, HeaderValue};
use veyra_domain::capture::{CaptureProxy, CaptureRoute, Exchange, ProxyStatus, UsageScan};
use veyra_domain::source::{ConnectorDescriptor, RawUsage};
use veyra_domain::time::UtcMillis;
use veyra_registry::Registry;
use veyra_store::Store;

use crate::ChangeListener;
use crate::http::{Request, Status, read_request, write_response};
use crate::push::{local_only, store_usage};

/// The proxy's port, next to the OpenTelemetry receiver's.
pub const PROXY_PORT: u16 = 4320;
/// Largest request body forwarded. Requests with images or documents can be large.
const MAX_REQUEST: u64 = 64 * 1024 * 1024;
/// Connections answered at once; further ones are told to retry.
pub const MAX_CONNECTIONS: usize = 32;
/// How long the client may take to send its request or to read each part of the reply.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);
/// How long reaching a provider may take. Responses themselves may stream for minutes.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Size of each relayed piece of a response.
const PIECE: usize = 16 * 1024;
/// Headers that describe one connection rather than the request, plus those the proxy
/// sets itself.
const NOT_FORWARDED: [&str; 12] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
    "expect",
];

/// Everything the proxy needs to relay requests and store their usage.
pub struct ProxyParts {
    pub store: Arc<Store>,
    pub registry: Arc<Registry>,
    pub routes: Vec<Box<dyn CaptureRoute>>,
    /// Called after usage it stored changed the data.
    pub on_change: ChangeListener,
}

/// The capture proxy. Stopped until [`CaptureProxy::set_enabled`] turns it on.
pub struct Proxy {
    shared: Arc<Shared>,
    port: u16,
    running: Mutex<Option<Running>>,
}

struct Shared {
    store: Arc<Store>,
    registry: Arc<Registry>,
    routes: Vec<Box<dyn CaptureRoute>>,
    on_change: Mutex<ChangeListener>,
    stats: Mutex<Stats>,
    active: AtomicUsize,
    sequence: AtomicU64,
}

#[derive(Default)]
struct Stats {
    error: Option<String>,
    relayed: u64,
    records: u64,
    last_relayed_at: Option<UtcMillis>,
    failure: Option<String>,
}

struct Running {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Proxy {
    /// A stopped proxy for `port`.
    #[must_use]
    pub fn new(parts: ProxyParts, port: u16) -> Self {
        Self {
            shared: Arc::new(Shared {
                store: parts.store,
                registry: parts.registry,
                routes: parts.routes,
                on_change: Mutex::new(parts.on_change),
                stats: Mutex::new(Stats::default()),
                active: AtomicUsize::new(0),
                sequence: AtomicU64::new(0),
            }),
            port,
            running: Mutex::new(None),
        }
    }

    fn start(&self, running: &mut Option<Running>) {
        if running.is_some() {
            return;
        }
        let result = TcpListener::bind((Ipv4Addr::LOCALHOST, self.port)).and_then(|listener| {
            let address = listener.local_addr()?;
            let stop = Arc::new(AtomicBool::new(false));
            let (shared, flag) = (Arc::clone(&self.shared), Arc::clone(&stop));
            let thread = std::thread::Builder::new()
                .name("veyra-proxy".to_owned())
                .spawn(move || serve(&listener, &shared, address.port(), &flag))?;
            Ok(Running {
                address,
                stop,
                thread,
            })
        });
        lock(&self.shared.stats).error = match result {
            Ok(started) => {
                *running = Some(started);
                None
            }
            Err(error) => Some(format!("Cannot listen on 127.0.0.1:{}: {error}", self.port)),
        };
    }

    fn stop(running: &mut Option<Running>) {
        if let Some(Running {
            address,
            stop,
            thread,
        }) = running.take()
        {
            stop.store(true, Ordering::Release);
            // Wakes the blocking accept so the thread sees the flag.
            let _ = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT);
            let _ = thread.join();
        }
    }
}

impl CaptureProxy for Proxy {
    fn set_enabled(&self, enabled: bool) {
        let mut running = lock(&self.running);
        if enabled {
            self.start(&mut running);
        } else {
            Self::stop(&mut running);
            lock(&self.shared.stats).error = None;
        }
    }

    fn status(&self) -> ProxyStatus {
        let running = lock(&self.running);
        let stats = lock(&self.shared.stats);
        let port = running.as_ref().map_or(self.port, |r| r.address.port());
        ProxyStatus {
            // The thread ends early only when the HTTPS client could not be built.
            listening: running.as_ref().is_some_and(|r| !r.thread.is_finished()),
            endpoint: format!("http://127.0.0.1:{port}"),
            error: stats.error.clone(),
            relayed: stats.relayed,
            records: stats.records,
            last_relayed_at: stats.last_relayed_at,
            failure: stats.failure.clone(),
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        Self::stop(&mut lock(&self.running));
    }
}

/// The HTTPS client. Built on the listener's own thread: the blocking client must not
/// be created or dropped inside an async runtime.
fn client() -> reqwest::Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(None::<Duration>)
        .build()
}

/// Accepts connections until `stop` is set, answering each on its own thread.
fn serve(listener: &TcpListener, shared: &Arc<Shared>, port: u16, stop: &AtomicBool) {
    let client = match client() {
        Ok(client) => client,
        Err(error) => {
            lock(&shared.stats).error = Some(format!(
                "Cannot start the proxy's HTTPS client: {}",
                error.without_url()
            ));
            return;
        }
    };
    for stream in listener.incoming() {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let Ok(mut stream) = stream else { continue };
        if shared.active.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            shared.active.fetch_sub(1, Ordering::AcqRel);
            let _ = write_response(
                &mut stream,
                Status::new(503, "too many requests at once; try again"),
            );
            continue;
        }
        let (own, own_client) = (Arc::clone(shared), client.clone());
        let spawned = std::thread::Builder::new()
            .name("veyra-proxy-request".to_owned())
            .spawn(move || {
                relay(stream, &own, &own_client, port);
                own.active.fetch_sub(1, Ordering::AcqRel);
            });
        if spawned.is_err() {
            shared.active.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// The route named by the first path segment, and the path after it.
fn route<'a>(shared: &'a Shared, path: &'a str) -> Option<(&'a dyn CaptureRoute, &'a str)> {
    let path = path.strip_prefix('/')?;
    let (prefix, rest) = path.split_at(path.find('/').unwrap_or(path.len()));
    let route = shared.routes.iter().find(|r| r.prefix() == prefix)?;
    Some((route.as_ref(), rest))
}

fn forwarded_headers(request: &Request) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in &request.headers {
        if NOT_FORWARDED.contains(&name.as_str()) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            headers.append(name, value);
        }
    }
    // Usage is read from the body on the way, so it must arrive uncompressed.
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    headers
}

/// Answers one connection: checks the request, forwards it, relays the response and
/// stores its usage.
fn relay(mut stream: TcpStream, shared: &Shared, client: &Client, port: u16) {
    let _ = stream.set_read_timeout(Some(CLIENT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_TIMEOUT));
    let request = match read_request(&mut BufReader::new(&stream), MAX_REQUEST) {
        Ok(request) => request,
        Err(status) => {
            let _ = write_response(&mut stream, status);
            return;
        }
    };
    if let Err(status) = local_only(&request, port) {
        let _ = write_response(&mut stream, status);
        return;
    }
    let Some((route, rest)) = route(shared, &request.path) else {
        let _ = write_response(
            &mut stream,
            Status::new(
                404,
                "unknown provider: start the path with a prefix Veyra lists",
            ),
        );
        return;
    };
    let Ok(method) = reqwest::Method::from_bytes(request.method.as_bytes()) else {
        let _ = write_response(&mut stream, Status::new(405, "unsupported method"));
        return;
    };
    let mut url = format!("{}{rest}", route.upstream());
    if !request.query.is_empty() {
        url.push('?');
        url.push_str(&request.query);
    }
    let sent = client
        .request(method, url)
        .headers(forwarded_headers(&request))
        .body(request.body.clone())
        .send();
    let response = match sent {
        Ok(response) => response,
        Err(error) => {
            // The error is described by kind only: its URL may carry an API key.
            let reason = if error.is_timeout() {
                "the provider did not answer in time"
            } else {
                "the provider could not be reached"
            };
            shared.failed(reason);
            let _ = write_response(&mut stream, Status::new(502, reason));
            return;
        }
    };
    let recording = response.status().is_success() && route.records(&request.method, rest);
    match pass_on(&mut stream, response, recording.then_some(route)) {
        Ok((scanner, request_id)) => shared.relayed(route, rest, request_id.as_deref(), scanner),
        Err(Cut::Provider) => shared.failed("the provider's response was cut off"),
        // The client went away; there is nothing to report to it.
        Err(Cut::Client) => {}
    }
}

/// Why relaying a response stopped early.
enum Cut {
    Provider,
    Client,
}

/// A relayed response's scanner, when it is recorded, and the provider's request id.
type Relayed = (Option<Box<dyn UsageScan>>, Option<String>);

/// Relays `response` to the client as it arrives, with chunked framing, feeding it to
/// the scanner of the `recording` route.
fn pass_on(
    stream: &mut TcpStream,
    mut response: Response,
    recording: Option<&dyn CaptureRoute>,
) -> Result<Relayed, Cut> {
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let request_id = header("request-id").or_else(|| header("x-request-id"));
    let mut scanner = recording.map(|route| route.scanner(header("content-type").as_deref()));
    write_head(stream, &response).map_err(|_| Cut::Client)?;
    let mut piece = vec![0; PIECE];
    loop {
        let read = match response.read(&mut piece) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Cut::Provider),
        };
        let data = &piece[..read];
        write!(stream, "{read:x}\r\n")
            .and_then(|()| stream.write_all(data))
            .and_then(|()| stream.write_all(b"\r\n"))
            .and_then(|()| stream.flush())
            .map_err(|_| Cut::Client)?;
        if let Some(scanner) = scanner.as_mut() {
            scanner.feed(data);
        }
    }
    stream
        .write_all(b"0\r\n\r\n")
        .and_then(|()| stream.flush())
        .map_err(|_| Cut::Client)?;
    Ok((scanner, request_id))
}

/// The status line and the provider's headers, except those describing its connection.
fn write_head(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let status = response.status();
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("")
    )
    .into_bytes();
    for (name, value) in response.headers() {
        if NOT_FORWARDED.contains(&name.as_str()) {
            continue;
        }
        head.extend_from_slice(name.as_str().as_bytes());
        head.extend_from_slice(b": ");
        head.extend_from_slice(value.as_bytes());
        head.extend_from_slice(b"\r\n");
    }
    head.extend_from_slice(b"Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
    stream.write_all(&head)
}

impl Shared {
    fn failed(&self, reason: &str) {
        lock(&self.stats).failure = Some(reason.to_owned());
    }

    /// Counts a relayed request and stores the usage its response reported.
    fn relayed(
        &self,
        route: &dyn CaptureRoute,
        path: &str,
        request_id: Option<&str>,
        scanner: Option<Box<dyn UsageScan>>,
    ) {
        let at = UtcMillis::now();
        {
            let mut stats = lock(&self.stats);
            stats.relayed += 1;
            stats.last_relayed_at = Some(at);
            stats.failure = None;
        }
        let Some(scanner) = scanner else { return };
        let fallback_id = format!("{}-{}", at.0, self.sequence.fetch_add(1, Ordering::Relaxed));
        let usage = scanner.finish(&Exchange {
            path,
            request_id,
            at,
            fallback_id: &fallback_id,
        });
        if let Some(usage) = usage {
            self.store(usage, route.descriptor());
        }
    }

    fn store(&self, usage: RawUsage, descriptor: &ConnectorDescriptor) {
        match store_usage(&self.store, &self.registry, [(usage, descriptor)]) {
            Ok((stored, report)) => {
                lock(&self.stats).records += stored;
                if report.changed() {
                    (lock(&self.on_change))(&report);
                }
            }
            Err(_) => self.failed("usage could not be stored"),
        }
    }
}
