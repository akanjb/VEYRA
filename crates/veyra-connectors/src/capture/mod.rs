//! Usage in API responses relayed by Veyra's local capture proxy (ADR-039).
//!
//! A client whose base URL points at the proxy sends its requests through Veyra, which
//! forwards them unchanged to the provider. This module is the pure half: the routes,
//! and a scanner that reads only the response's identifiers, model and `usage` block
//! from a body or an event stream. Text, tool arguments and every other field are
//! skipped as they arrive and never kept.

mod scanner;
mod wire;

use veyra_domain::capture::{CaptureRoute, UsageScan};
use veyra_domain::connector::{Capabilities, Support};
use veyra_domain::identity::{AppId, ConnectorId};
use veyra_domain::source::ConnectorDescriptor;

pub use scanner::UsageScanner;

/// Bump when usage reading changes. Captured events are never re-read, so this only
/// records which rules produced an event.
const PARSER_VERSION: u32 = 1;

/// What relayed API responses report.
const CAPABILITIES: Capabilities = Capabilities {
    tokens: Support::Full,
    cache: Support::Full,
    reasoning: Support::Full,
    tools: Support::Partial("Requests are relayed, not interpreted"),
    sessions: Support::Partial("API requests carry no session"),
    projects: Support::Partial("API requests carry no working folder"),
};

/// The response shape a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// OpenAI Chat Completions, Responses, Completions and Embeddings, and the
    /// OpenAI-compatible APIs that copy them.
    OpenAi,
    /// Anthropic Messages.
    Anthropic,
    /// Google Gemini `generateContent` and `streamGenerateContent`.
    Gemini,
}

/// One provider the proxy forwards to.
#[derive(Debug, Clone)]
pub struct Route {
    /// First path segment clients use: `http://127.0.0.1:<port>/<prefix>/…`.
    pub prefix: &'static str,
    /// The provider's official HTTPS origin, with any base path.
    pub upstream: &'static str,
    /// What the client's base URL must end with after the prefix, e.g. `/v1`.
    pub client_suffix: &'static str,
    pub wire: Wire,
    /// Used when the model is not in the registry.
    pub provider: &'static str,
    /// Set for routing services that resell other providers' models.
    pub gateway: Option<&'static str>,
    pub descriptor: ConnectorDescriptor,
}

impl CaptureRoute for Route {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    fn prefix(&self) -> &'static str {
        self.prefix
    }

    fn upstream(&self) -> &'static str {
        self.upstream
    }

    fn client_suffix(&self) -> &'static str {
        self.client_suffix
    }

    /// Model lists, file uploads and token counting record nothing.
    fn records(&self, method: &str, path: &str) -> bool {
        if method != "POST" {
            return false;
        }
        match self.wire {
            Wire::OpenAi => [
                "/chat/completions",
                "/responses",
                "/completions",
                "/embeddings",
            ]
            .iter()
            .any(|end| path.ends_with(end)),
            Wire::Anthropic => path.ends_with("/messages"),
            Wire::Gemini => {
                path.ends_with(":generateContent") || path.ends_with(":streamGenerateContent")
            }
        }
    }

    fn scanner(&self, content_type: Option<&str>) -> Box<dyn UsageScan> {
        Box::new(UsageScanner::new(self.clone(), content_type))
    }
}

fn route(
    prefix: &'static str,
    upstream: &'static str,
    client_suffix: &'static str,
    wire: Wire,
    provider: &'static str,
    gateway: Option<&'static str>,
    display_name: &'static str,
) -> Route {
    let id = format!("{prefix}-api");
    Route {
        prefix,
        upstream,
        client_suffix,
        wire,
        provider,
        gateway,
        descriptor: ConnectorDescriptor {
            id: ConnectorId::new(id.clone()).unwrap_or_else(|_| unreachable!()),
            app: AppId::new(id).unwrap_or_else(|_| unreachable!()),
            display_name,
            parser_version: PARSER_VERSION,
            documented_format: true,
            capabilities: CAPABILITIES,
        },
    }
}

/// Every provider the proxy forwards to, in display order.
///
/// # Panics
/// Never: the identifiers are valid constants.
#[must_use]
pub fn routes() -> Vec<Route> {
    vec![
        route(
            "openai",
            "https://api.openai.com",
            "/v1",
            Wire::OpenAi,
            "openai",
            None,
            "OpenAI API",
        ),
        route(
            "anthropic",
            "https://api.anthropic.com",
            "",
            Wire::Anthropic,
            "anthropic",
            None,
            "Anthropic API",
        ),
        route(
            "gemini",
            "https://generativelanguage.googleapis.com",
            "",
            Wire::Gemini,
            "google",
            None,
            "Gemini API",
        ),
        route(
            "openrouter",
            "https://openrouter.ai/api",
            "/v1",
            Wire::OpenAi,
            "openrouter",
            Some("openrouter"),
            "OpenRouter",
        ),
    ]
}

/// Every provider the proxy forwards to, for the ingest layer.
#[must_use]
pub fn all() -> Vec<Box<dyn CaptureRoute>> {
    routes()
        .into_iter()
        .map(|r| Box::new(r) as Box<dyn CaptureRoute>)
        .collect()
}

/// The applications the proxy records usage for.
#[must_use]
pub fn descriptors() -> Vec<ConnectorDescriptor> {
    routes().into_iter().map(|r| r.descriptor).collect()
}

#[cfg(test)]
mod tests;
