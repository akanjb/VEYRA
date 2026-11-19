//! API usage relayed through Veyra's local capture proxy (ADR-039).
//!
//! A client pointed at the proxy sends its requests through Veyra, which forwards them
//! unchanged to the provider and reads the usage in each response. Connectors describe
//! the providers and read their response formats; the ingest layer does the network
//! work and never sees a format. The proxy is off until the user turns it on, and only
//! ever listens on the loopback interface.

use crate::source::{ConnectorDescriptor, RawUsage};
use crate::time::UtcMillis;

/// Facts about one relayed request that the response body does not carry.
#[derive(Debug, Clone, Copy)]
pub struct Exchange<'a> {
    /// The request path after the route prefix, without the query.
    pub path: &'a str,
    /// The provider's request id response header, when it sends one.
    pub request_id: Option<&'a str>,
    /// When the response finished.
    pub at: UtcMillis,
    /// A unique id for the key of a response that names none.
    pub fallback_id: &'a str,
}

/// Reads the usage of one response from its body as it is relayed. Implementations
/// keep only identifiers, the model and usage counts, never content.
pub trait UsageScan: Send {
    /// Reads the next chunk of the body, in order.
    fn feed(&mut self, chunk: &[u8]);
    /// The usage of the finished response, if it reported any or named itself.
    fn finish(self: Box<Self>, exchange: &Exchange<'_>) -> Option<RawUsage>;
}

/// One provider the proxy forwards to.
pub trait CaptureRoute: Send + Sync {
    /// The application its usage is recorded under.
    fn descriptor(&self) -> &ConnectorDescriptor;
    /// First path segment clients use: `http://127.0.0.1:<port>/<prefix>/…`.
    fn prefix(&self) -> &'static str;
    /// The provider's official HTTPS origin, with any base path.
    fn upstream(&self) -> &'static str;
    /// What the client's base URL ends with after the prefix, e.g. `/v1`.
    fn client_suffix(&self) -> &'static str;
    /// Whether a request with `method` to `path` (after the prefix, without the query)
    /// produces usage worth recording.
    fn records(&self, method: &str, path: &str) -> bool;
    /// A scanner for a successful response of `content_type`.
    fn scanner(&self, content_type: Option<&str>) -> Box<dyn UsageScan>;
}

/// What the proxy is doing now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyStatus {
    /// Whether it is accepting connections.
    pub listening: bool,
    /// The base address clients use, e.g. `http://127.0.0.1:4320`.
    pub endpoint: String,
    /// Why it could not start, e.g. the port is taken.
    pub error: Option<String>,
    /// Requests relayed since Veyra started.
    pub relayed: u64,
    /// Usage records stored from them since Veyra started.
    pub records: u64,
    pub last_relayed_at: Option<UtcMillis>,
    /// Why the latest failed request failed, until one succeeds, e.g. the provider
    /// could not be reached.
    pub failure: Option<String>,
}

/// Control of the local capture proxy (implemented by the ingest layer).
pub trait CaptureProxy: Send + Sync {
    /// Starts or stops listening; a failure to start is reported by [`status`](Self::status).
    fn set_enabled(&self, enabled: bool);
    fn status(&self) -> ProxyStatus;
}
