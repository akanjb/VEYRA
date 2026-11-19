//! The local OpenTelemetry receiver (ADR-038).
//!
//! - Off until the user turns it on; it then listens on 127.0.0.1 only.
//! - Accepts OTLP/HTTP JSON log exports at `/v1/logs`. Requests from web pages (with an
//!   `Origin` header) and requests naming another host (DNS rebinding) are refused.
//! - Each record goes to the connector whose tool reports that `service.name`; the
//!   connector reads the tool's documented usage attributes only. Records are never
//!   stored or logged, so prompt or tool content a tool may be set to send is dropped.
//! - Usage is normalised, priced and written exactly like usage read from files, with
//!   the same natural keys, so a request seen both ways is stored once.

use std::io::BufReader;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use veyra_domain::source::SourceConnector;
use veyra_domain::telemetry::{ReceiverStatus, TelemetryReceiver, TelemetryRecord};
use veyra_domain::time::UtcMillis;
use veyra_registry::Registry;
use veyra_store::{Store, StoreError};

use crate::ChangeListener;
use crate::http::{MAX_BODY, Request, Status, read_request, write_response};
use crate::otlp::decode_logs;
use crate::push::{local_only, store_usage};

/// The standard OTLP/HTTP port.
pub const DEFAULT_PORT: u16 = 4318;
/// The OTLP/HTTP path of log exports.
const LOGS_PATH: &str = "/v1/logs";
/// How long a connection may take to send its request or read the response.
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Everything the receiver needs to turn records into stored usage.
pub struct ReceiverParts {
    pub store: Arc<Store>,
    pub registry: Arc<Registry>,
    pub connectors: Vec<Box<dyn SourceConnector>>,
    pub on_change: ChangeListener,
}

/// The receiver; dropping it stops listening.
pub struct Receiver {
    shared: Arc<Shared>,
    /// The port asked for; 0 lets the system choose one (tests).
    port: u16,
    running: Mutex<Option<Running>>,
}

struct Shared {
    store: Arc<Store>,
    registry: Arc<Registry>,
    connectors: Vec<Box<dyn SourceConnector>>,
    on_change: Mutex<ChangeListener>,
    stats: Mutex<Stats>,
}

#[derive(Default)]
struct Stats {
    error: Option<String>,
    exports: u64,
    records: u64,
    last_received_at: Option<UtcMillis>,
    refusal: Option<String>,
}

struct Running {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Receiver {
    /// A stopped receiver for `port`.
    #[must_use]
    pub fn new(parts: ReceiverParts, port: u16) -> Self {
        Self {
            shared: Arc::new(Shared {
                store: parts.store,
                registry: parts.registry,
                connectors: parts.connectors,
                on_change: Mutex::new(parts.on_change),
                stats: Mutex::new(Stats::default()),
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
                .name("veyra-otlp".to_owned())
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
            let _ = TcpStream::connect_timeout(&address, IO_TIMEOUT);
            let _ = thread.join();
        }
    }
}

impl TelemetryReceiver for Receiver {
    fn set_enabled(&self, enabled: bool) {
        let mut running = lock(&self.running);
        if enabled {
            self.start(&mut running);
        } else {
            Self::stop(&mut running);
            lock(&self.shared.stats).error = None;
        }
    }

    fn status(&self) -> ReceiverStatus {
        let running = lock(&self.running);
        let stats = lock(&self.shared.stats);
        let port = running.as_ref().map_or(self.port, |r| r.address.port());
        ReceiverStatus {
            listening: running.is_some(),
            endpoint: format!("http://127.0.0.1:{port}"),
            error: stats.error.clone(),
            exports: stats.exports,
            records: stats.records,
            last_received_at: stats.last_received_at,
            refusal: stats.refusal.clone(),
        }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        Self::stop(&mut lock(&self.running));
    }
}

/// Answers connections one at a time until `stop` is set.
fn serve(listener: &TcpListener, shared: &Shared, port: u16, stop: &AtomicBool) {
    for stream in listener.incoming() {
        if stop.load(Ordering::Acquire) {
            break;
        }
        if let Ok(stream) = stream {
            handle(stream, shared, port);
        }
    }
}

fn handle(mut stream: TcpStream, shared: &Shared, port: u16) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let status = match read_request(&mut BufReader::new(&stream), MAX_BODY) {
        Ok(request) => respond(&request, shared, port),
        Err(status) => status,
    };
    shared.note(status);
    let _ = write_response(&mut stream, status);
}

/// Checks the request, then stores the usage it carries.
fn respond(request: &Request, shared: &Shared, port: u16) -> Status {
    if let Err(status) = check(request, port) {
        return status;
    }
    let Ok(records) = decode_logs(&request.body) else {
        return Status::new(400, "the body is not an OTLP JSON log export");
    };
    match shared.ingest(&records) {
        Ok(()) => Status::OK,
        // The exporter retries later.
        Err(_) => Status::new(503, "usage could not be stored; try again"),
    }
}

fn check(request: &Request, port: u16) -> Result<(), Status> {
    local_only(request, port)?;
    if request.path != LOGS_PATH {
        return Err(Status::new(
            404,
            "only OTLP log exports to /v1/logs are accepted",
        ));
    }
    if request.method != "POST" {
        return Err(Status::new(405, "use POST"));
    }
    if request
        .header("content-encoding")
        .is_some_and(|e| !e.eq_ignore_ascii_case("identity"))
    {
        return Err(Status::new(415, "turn off OTLP compression"));
    }
    let json = request.header("content-type").is_some_and(|t| {
        t.split(';')
            .next()
            .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
    });
    if !json {
        return Err(Status::new(415, "send OTLP as http/json"));
    }
    Ok(())
}

impl Shared {
    /// Counts an accepted export, or keeps why one was refused so the user can fix
    /// the tool's settings.
    fn note(&self, answer: Status) {
        let mut stats = lock(&self.stats);
        if answer == Status::OK {
            stats.exports += 1;
            stats.refusal = None;
        } else {
            stats.refusal = Some(format!("{} (HTTP {})", answer.message, answer.code));
        }
    }

    /// Normalises, prices and stores the usage in `records`.
    fn ingest(&self, records: &[TelemetryRecord]) -> Result<(), StoreError> {
        let usage = records.iter().filter_map(|record| {
            let connector = self
                .connectors
                .iter()
                .find(|c| c.telemetry_service() == Some(record.service.as_str()))?;
            Some((connector.telemetry_usage(record)?, connector.descriptor()))
        });
        let (stored, report) = store_usage(&self.store, &self.registry, usage)?;
        if stored == 0 {
            return Ok(());
        }
        {
            let mut stats = lock(&self.stats);
            stats.records += stored;
            stats.last_received_at = Some(UtcMillis::now());
        }
        if report.changed() {
            (lock(&self.on_change))(&report);
        }
        Ok(())
    }
}
