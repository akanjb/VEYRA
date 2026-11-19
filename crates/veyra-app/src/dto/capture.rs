use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request for [`crate::Api::get_capture_proxy`]. Intentionally empty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CaptureProxyRequest {}

/// Request for [`crate::Api::set_capture_proxy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetCaptureProxyRequest {
    pub enabled: bool,
}

/// One provider the capture proxy forwards to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CaptureProvider {
    /// The application its usage is recorded under, e.g. `OpenAI API`.
    pub name: String,
    /// The base URL to give a client of this provider instead of the official one.
    pub base_url: String,
    /// The official address requests are forwarded to.
    pub upstream: String,
}

/// The local capture proxy (ADR-039).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CaptureProxyInfo {
    /// Whether the user turned it on (kept across restarts).
    pub enabled: bool,
    /// Whether it is accepting connections now.
    pub listening: bool,
    /// The base address clients send to.
    pub endpoint: String,
    /// Why it is not listening although turned on, e.g. the port is taken.
    pub error: Option<String>,
    /// Requests relayed since Veyra started.
    #[ts(type = "number")]
    pub relayed: u64,
    /// Usage records stored from them since Veyra started.
    #[ts(type = "number")]
    pub records: u64,
    /// UTC epoch milliseconds.
    #[ts(type = "number | null")]
    pub last_relayed_at: Option<i64>,
    /// Why the latest failed request failed, until one succeeds.
    pub last_failure: Option<String>,
    pub providers: Vec<CaptureProvider>,
}
