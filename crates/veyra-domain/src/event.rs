//! The unified usage event: one request (or one provider-reported bucket) in canonical
//! form, independent of the source it came from.

use crate::accuracy::AccuracyMap;
use crate::identity::{AppId, ConnectorId, GatewayId, Interface, ModelId, ProviderId};
use crate::money::NanoUsd;
use crate::time::UtcMillis;
use crate::tokens::{TokenCounts, TokenError};

/// The natural key that makes ingestion idempotent, e.g. `anthropic:req_011…`.
/// Connectors derive it so the same request from two sources collapses to one event.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DedupKey(String);

impl DedupKey {
    /// # Errors
    /// Returns [`EventError::InvalidDedupKey`] for an empty key or one over 512 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, EventError> {
        let value = value.into();
        if value.is_empty() || value.len() > 512 {
            return Err(EventError::InvalidDedupKey);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether an event is one request or a provider-reported aggregate bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Granularity {
    Request,
    /// Aggregated by the provider (e.g. a daily usage API row); per-request prompt
    /// size is unknown, so prompt-length price tiers cannot be applied exactly.
    Bucket,
}

/// The service tier a request was billed under, when the source records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceTier {
    Standard,
    Batch,
    Flex,
    Priority,
    Fast,
    Ultrafast,
}

macro_rules! str_enum {
    ($ty:ty { $($variant:ident => $text:literal),* $(,)? }) => {
        impl $ty {
            /// Stable storage and wire representation.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),* }
            }

            #[must_use]
            pub fn parse(value: &str) -> Option<Self> {
                match value { $($text => Some(Self::$variant),)* _ => None }
            }
        }
    };
}

str_enum!(Granularity { Request => "request", Bucket => "bucket" });
str_enum!(ServiceTier {
    Standard => "standard",
    Batch => "batch",
    Flex => "flex",
    Priority => "priority",
    Fast => "fast",
    Ultrafast => "ultrafast",
});

/// Where an event came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceRef {
    pub connector: ConnectorId,
    pub parser_version: u32,
    /// Version of the tool that wrote the source record, when known.
    pub tool_version: Option<String>,
}

/// Who served the request and through which application (PRD FR-NORM-01).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attribution {
    pub provider: Option<ProviderId>,
    /// Canonical model id from the registry, if recognised.
    pub model: Option<ModelId>,
    /// The model string exactly as the source reported it.
    pub raw_model: Option<String>,
    pub app: AppId,
    pub interface: Interface,
    pub gateway: Option<GatewayId>,
}

/// Natural keys linking the event to sessions, projects and devices. The store maps
/// them to its own identifiers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct EventContext {
    /// Source-scoped session key, e.g. `claude-code:<session-id>`.
    pub session_key: Option<String>,
    pub parent_session_key: Option<String>,
    /// Normalised project root path.
    pub project_key: Option<String>,
    pub device_key: Option<String>,
    pub is_subagent: bool,
}

/// One unit of AI usage in canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageEvent {
    pub dedup_key: DedupKey,
    pub occurred_at: UtcMillis,
    pub source: SourceRef,
    pub attribution: Attribution,
    pub context: EventContext,
    pub tokens: TokenCounts,
    pub requests: Option<u32>,
    /// Cost the tool or provider itself reported (never an estimate).
    pub reported_cost: Option<NanoUsd>,
    pub accuracy: AccuracyMap,
    pub granularity: Granularity,
    pub service_tier: ServiceTier,
    /// US-only inference was requested (priced with a regional multiplier).
    pub inference_geo_us: bool,
}

/// Error raised when an event violates domain invariants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
    #[error("dedup key must be 1–512 bytes")]
    InvalidDedupKey,
    #[error(transparent)]
    Tokens(#[from] TokenError),
    #[error("reported cost must not be negative")]
    NegativeReportedCost,
}

impl UsageEvent {
    /// Checks every invariant. Writers must call this before persisting (fail fast).
    ///
    /// # Errors
    /// Returns the first violated invariant.
    pub fn validate(&self) -> Result<(), EventError> {
        self.tokens.validate()?;
        if self.reported_cost.is_some_and(|cost| cost.0 < 0) {
            return Err(EventError::NegativeReportedCost);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DedupKey, Granularity, ServiceTier};

    #[test]
    fn dedup_keys_are_bounded() {
        assert!(DedupKey::new("anthropic:req_1").is_ok());
        assert!(DedupKey::new("").is_err());
        assert!(DedupKey::new("k".repeat(513)).is_err());
    }

    #[test]
    fn enums_round_trip() {
        for tier in [
            ServiceTier::Standard,
            ServiceTier::Batch,
            ServiceTier::Flex,
            ServiceTier::Priority,
            ServiceTier::Fast,
            ServiceTier::Ultrafast,
        ] {
            assert_eq!(ServiceTier::parse(tier.as_str()), Some(tier));
        }
        assert_eq!(Granularity::parse("bucket"), Some(Granularity::Bucket));
        assert_eq!(Granularity::parse("hour"), None);
    }
}
