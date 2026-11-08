//! Normalisation (FR-NORM): turns a connector's raw record into a domain event.
//!
//! - The model string is resolved to a canonical model through the registry, which also
//!   gives the provider. Unknown models keep their raw string and are unpriced.
//! - The working directory becomes the project key.
//! - The application, connector and parser version come from the connector.

use veyra_domain::event::{Attribution, DedupKey, EventContext, EventError, SourceRef, UsageEvent};
use veyra_domain::identity::{IdError, ProviderId};
use veyra_domain::source::{ConnectorDescriptor, RawUsage};
use veyra_registry::Registry;

/// Why a raw record could not be normalised (counted as an issue, never fatal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizeError {
    Event(EventError),
    Id(IdError),
}

/// Converts a raw record into a validated domain event.
///
/// # Errors
/// Returns [`NormalizeError`] when the record violates domain invariants.
pub fn normalize(
    raw: &RawUsage,
    descriptor: &ConnectorDescriptor,
    registry: &Registry,
) -> Result<UsageEvent, NormalizeError> {
    let model = raw.model.as_deref().and_then(|m| registry.resolve_model(m));
    let provider = match model {
        Some(model) => Some(model.provider.clone()),
        None => raw
            .provider
            .as_deref()
            .map(|p| ProviderId::new(p.to_ascii_lowercase()))
            .transpose()
            .map_err(NormalizeError::Id)?,
    };
    let event = UsageEvent {
        dedup_key: DedupKey::new(raw.natural_key.clone()).map_err(NormalizeError::Event)?,
        occurred_at: raw.occurred_at,
        source: SourceRef {
            connector: descriptor.id.clone(),
            parser_version: descriptor.parser_version,
            tool_version: raw.tool_version.clone(),
        },
        attribution: Attribution {
            provider,
            model: model.map(|m| m.id.clone()),
            raw_model: raw.model.clone(),
            app: descriptor.app.clone(),
            interface: raw.interface,
            gateway: None,
        },
        context: EventContext {
            session_key: raw.session_key.clone(),
            parent_session_key: raw.parent_session_key.clone(),
            project_key: raw.cwd.as_deref().and_then(project_key),
            device_key: None,
            is_subagent: raw.is_subagent,
        },
        tokens: raw.tokens,
        requests: raw.requests,
        reported_cost: raw.reported_cost,
        accuracy: raw.accuracy,
        granularity: raw.granularity,
        service_tier: raw.service_tier,
        inference_geo_us: raw.inference_geo_us,
    };
    event.validate().map_err(NormalizeError::Event)?;
    Ok(event)
}

/// A stable project key from a working directory: `/` separators, no trailing slash, and
/// lower case for Windows paths (which are case-insensitive).
#[must_use]
pub fn project_key(cwd: &str) -> Option<String> {
    let mut path = cwd.trim().replace('\\', "/");
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    if path.is_empty() {
        return None;
    }
    let windows = path.as_bytes().get(1) == Some(&b':') || path.starts_with("//");
    Some(if windows { path.to_lowercase() } else { path })
}

#[cfg(test)]
mod tests {
    use veyra_domain::accuracy::{Accuracy, AccuracyMap};
    use veyra_domain::event::{Granularity, ServiceTier};
    use veyra_domain::identity::{AppId, ConnectorId, Interface};
    use veyra_domain::source::{ConnectorDescriptor, RawUsage, SourceFormat};
    use veyra_domain::time::UtcMillis;
    use veyra_domain::tokens::TokenCounts;
    use veyra_registry::Registry;

    use super::{normalize, project_key};

    fn descriptor() -> ConnectorDescriptor {
        ConnectorDescriptor {
            id: ConnectorId::new("claude-code").unwrap(),
            app: AppId::new("claude-code").unwrap(),
            display_name: "Claude Code",
            parser_version: 1,
            format: SourceFormat::JsonLines,
            documented_format: false,
        }
    }

    fn raw(model: &str) -> RawUsage {
        RawUsage {
            natural_key: "anthropic:req_1".to_owned(),
            precedence: 0,
            occurred_at: UtcMillis(0),
            model: Some(model.to_owned()),
            provider: Some("anthropic".to_owned()),
            tokens: TokenCounts {
                input_uncached: Some(1),
                ..TokenCounts::default()
            },
            requests: Some(1),
            reported_cost: None,
            interface: Interface::Desktop,
            service_tier: ServiceTier::Standard,
            inference_geo_us: false,
            granularity: Granularity::Request,
            session_key: Some("claude-code:s".to_owned()),
            parent_session_key: None,
            is_subagent: false,
            cwd: Some("C:\\Users\\Dev\\Work\\Veyra\\".to_owned()),
            tool_version: Some("2.1.300".to_owned()),
            accuracy: AccuracyMap::uniform(Accuracy::Exact),
            tool_calls: Vec::new(),
        }
    }

    #[test]
    fn resolves_models_through_the_registry() {
        let registry = Registry::bundled().unwrap();
        let event = normalize(&raw("claude-opus-4-5-20251101"), &descriptor(), &registry).unwrap();
        assert_eq!(event.attribution.model.unwrap().as_str(), "claude-opus-4-5");
        assert_eq!(event.attribution.provider.unwrap().as_str(), "anthropic");
        assert_eq!(
            event.context.project_key.as_deref(),
            Some("c:/users/dev/work/veyra")
        );
        assert_eq!(event.attribution.interface, Interface::Desktop);
    }

    #[test]
    fn keeps_unknown_models_raw_and_unresolved() {
        let registry = Registry::bundled().unwrap();
        let event = normalize(&raw("claude-future-9"), &descriptor(), &registry).unwrap();
        assert_eq!(event.attribution.model, None);
        assert_eq!(
            event.attribution.raw_model.as_deref(),
            Some("claude-future-9")
        );
        assert_eq!(event.attribution.provider.unwrap().as_str(), "anthropic");
    }

    #[test]
    fn normalises_project_paths() {
        assert_eq!(
            project_key("/home/dev/app/").as_deref(),
            Some("/home/dev/app")
        );
        assert_eq!(project_key("/").as_deref(), Some("/"));
        assert_eq!(project_key("D:\\Code\\App").as_deref(), Some("d:/code/app"));
        assert_eq!(project_key("   "), None);
    }
}
