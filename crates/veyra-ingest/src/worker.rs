//! The background ingest worker (FR-ING-07).
//!
//! One thread scans enabled sources at startup, shortly after their files change,
//! periodically and on request. File-system notifications are only a hint to scan
//! sooner: each scan compares files with their checkpoints, and the periodic scan
//! catches anything a watcher missed, so correctness never depends on notifications.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use veyra_domain::source::{DiscoveryEnv, IngestControl, SourceConnector};
use veyra_registry::Registry;
use veyra_store::Store;

use crate::discovery::discover;
use crate::pipeline::{DEFAULT_CHUNK_BYTES, IngestContext, ScanReport, scan_all};

/// Worker timing and resource settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    /// Interval of the safety-net scan (also re-runs discovery).
    pub periodic: Duration,
    /// Delay between the first change notification and the scan it triggers. Later
    /// notifications within the window join the same scan.
    pub debounce: Duration,
    pub chunk_bytes: u64,
    pub throttle: Option<Duration>,
    /// Whether to watch enabled roots for changes.
    pub watch: bool,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            periodic: Duration::from_mins(15),
            debounce: Duration::from_secs(2),
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            throttle: None,
            watch: true,
        }
    }
}

/// Callback run after a scan that changed stored data.
pub type ChangeListener = Box<dyn Fn(&ScanReport) + Send>;

/// Everything the worker thread owns.
pub struct IngestWorker {
    pub store: Arc<Store>,
    pub registry: Arc<Registry>,
    pub connectors: Vec<Box<dyn SourceConnector>>,
    pub env: DiscoveryEnv,
    pub config: WorkerConfig,
    pub on_change: ChangeListener,
}

enum Command {
    Scan(&'static str),
    FilesChanged,
    SourcesChanged,
    Shutdown,
}

/// Controls a running worker. Dropping the handle stops the worker.
#[derive(Debug)]
pub struct IngestHandle {
    sender: Sender<Command>,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Scan(_) => "Scan",
            Self::FilesChanged => "FilesChanged",
            Self::SourcesChanged => "SourcesChanged",
            Self::Shutdown => "Shutdown",
        })
    }
}

impl IngestHandle {
    /// Asks for a scan of every enabled source now.
    pub fn request_scan(&self) {
        let _ = self.sender.send(Command::Scan("manual"));
    }

    /// Tells the worker that sources were enabled or disabled: it re-watches the enabled
    /// roots and scans, so a newly enabled source is read at once.
    pub fn sources_changed(&self) {
        let _ = self.sender.send(Command::SourcesChanged);
    }

    /// Stops the worker, interrupting a scan between chunks, and waits for it to exit.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.sender.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl IngestControl for IngestHandle {
    fn request_scan(&self) {
        Self::request_scan(self);
    }

    fn sources_changed(&self) {
        Self::sources_changed(self);
    }
}

impl Drop for IngestHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts the worker thread. It discovers sources and scans the enabled ones at once.
///
/// # Errors
/// Returns the operating-system error if the thread cannot be created.
pub fn spawn(worker: IngestWorker) -> std::io::Result<IngestHandle> {
    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let thread = std::thread::Builder::new()
        .name("veyra-ingest".to_owned())
        .spawn({
            let sender = sender.clone();
            let cancel = Arc::clone(&cancel);
            move || worker.run(&receiver, &sender, &cancel)
        })?;
    Ok(IngestHandle {
        sender,
        cancel,
        thread: Some(thread),
    })
}

impl IngestWorker {
    fn run(self, commands: &Receiver<Command>, sender: &Sender<Command>, cancel: &AtomicBool) {
        let _ = discover(&self.store, &self.connectors, &self.env);
        let mut watcher = self.watch(sender);
        self.scan("startup", cancel);
        let mut next_periodic = Instant::now() + self.config.periodic;
        let mut pending_change: Option<Instant> = None;
        while !cancel.load(Ordering::Relaxed) {
            let wake = pending_change.map_or(next_periodic, |at| at.min(next_periodic));
            match commands.recv_timeout(wake.saturating_duration_since(Instant::now())) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Command::FilesChanged) => {
                    pending_change.get_or_insert_with(|| Instant::now() + self.config.debounce);
                }
                Ok(Command::Scan(trigger)) => self.scan(trigger, cancel),
                Ok(Command::SourcesChanged) => {
                    watcher = self.watch(sender);
                    self.scan("sources", cancel);
                }
                Err(RecvTimeoutError::Timeout) => {
                    let now = Instant::now();
                    if pending_change.is_some_and(|at| at <= now) {
                        pending_change = None;
                        self.scan("change", cancel);
                    }
                    if next_periodic <= now {
                        next_periodic = now + self.config.periodic;
                        let _ = discover(&self.store, &self.connectors, &self.env);
                        self.scan("periodic", cancel);
                    }
                }
            }
        }
        drop(watcher);
    }

    /// Watches the enabled roots; notifications for `.jsonl` files request a scan.
    fn watch(&self, sender: &Sender<Command>) -> Option<RecommendedWatcher> {
        if !self.config.watch {
            return None;
        }
        let sender = sender.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let relevant = result.is_ok_and(|event| {
                    event
                        .paths
                        .iter()
                        .any(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
                });
                if relevant {
                    let _ = sender.send(Command::FilesChanged);
                }
            })
            .ok()?;
        for source in self.store.sources().ok()?.iter().filter(|s| s.enabled) {
            // A root that cannot be watched is still covered by the periodic scan.
            let _ = watcher.watch(Path::new(&source.root), RecursiveMode::Recursive);
        }
        Some(watcher)
    }

    fn scan(&self, trigger: &'static str, cancel: &AtomicBool) {
        let context = IngestContext {
            store: &self.store,
            registry: &self.registry,
            connectors: &self.connectors,
            cancel,
            chunk_bytes: self.config.chunk_bytes,
            throttle: self.config.throttle,
        };
        // Failures are recorded on the run and the source by the pipeline.
        if let Ok(report) = scan_all(&context, trigger)
            && report.changed()
        {
            (self.on_change)(&report);
        }
    }
}
