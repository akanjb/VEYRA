//! Identifiers for the separate attribution concepts of a usage event (PRD FR-NORM-01):
//! the model **provider**, the **model**, the **application** that issued the request,
//! the **interface** it was used through, and an optional **gateway** that routed it.

use std::fmt;

/// Error raised for an identifier that is empty, too long or contains invalid characters.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} `{value}`: use 1–128 lowercase ASCII letters, digits, '-', '_' or '.'")]
pub struct IdError {
    kind: &'static str,
    value: String,
}

fn validate(kind: &'static str, value: &str) -> Result<(), IdError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.')
        });
    if valid {
        Ok(())
    } else {
        Err(IdError {
            kind,
            value: value.to_owned(),
        })
    }
}

macro_rules! identifier {
    ($(#[$doc:meta])* $name:ident, $kind:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Creates the identifier after validating its characters.
            ///
            /// # Errors
            /// Returns [`IdError`] when the value is not a valid identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                validate($kind, &value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

identifier!(
    /// The organisation that trains and serves a model, e.g. `anthropic`, `openai`.
    ProviderId,
    "provider id"
);
identifier!(
    /// A canonical model identifier, e.g. `claude-sonnet-5-5`, `gpt-6.1-sol`.
    ModelId,
    "model id"
);
identifier!(
    /// The application that issued the request, e.g. `claude-code`, `cursor`.
    AppId,
    "app id"
);
identifier!(
    /// A service that routed or resold the request, e.g. `openrouter`, `github-copilot`.
    GatewayId,
    "gateway id"
);
identifier!(
    /// The connector (source parser) that produced an event, e.g. `claude-code-jsonl`.
    ConnectorId,
    "connector id"
);

/// The surface through which an application was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interface {
    Cli,
    Desktop,
    Ide,
    Browser,
    Api,
    LocalRuntime,
}

impl Interface {
    pub const ALL: [Self; 6] = [
        Self::Cli,
        Self::Desktop,
        Self::Ide,
        Self::Browser,
        Self::Api,
        Self::LocalRuntime,
    ];

    /// Stable storage and wire representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Desktop => "desktop",
            Self::Ide => "ide",
            Self::Browser => "browser",
            Self::Api => "api",
            Self::LocalRuntime => "local-runtime",
        }
    }

    /// Parses the representation produced by [`Interface::as_str`].
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.as_str() == value)
    }
}

#[cfg(test)]
mod tests {
    use super::{Interface, ModelId, ProviderId};

    #[test]
    fn accepts_canonical_identifiers() {
        for value in ["anthropic", "claude-sonnet-5-5", "gpt-6.1-sol", "x_ai"] {
            assert!(ModelId::new(value).is_ok(), "{value}");
        }
    }

    #[test]
    fn rejects_invalid_identifiers() {
        for value in [
            "",
            "Anthropic",
            "has space",
            "slash/model",
            &"a".repeat(129),
        ] {
            assert!(ProviderId::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn interface_round_trips() {
        for interface in Interface::ALL {
            assert_eq!(Interface::parse(interface.as_str()), Some(interface));
        }
        assert_eq!(Interface::parse("web"), None);
    }
}
