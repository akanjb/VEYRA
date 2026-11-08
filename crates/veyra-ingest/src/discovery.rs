//! Source discovery (FR-ING-01/02): finds candidate roots and registers them disabled.
//!
//! Discovery only checks that a documented root directory exists; it never opens files.
//! A source is read only after the user enables it (consent is recorded by the store).

use std::path::PathBuf;

use veyra_domain::source::{DiscoveryEnv, SourceConnector};
use veyra_store::{SourceRecord, Store};

use crate::{IngestError, now};

/// Documented location variables connectors may consult (ADR-014). No other environment
/// variable is read: in particular, never one that holds a credential.
const LOCATION_VARIABLES: [&str; 3] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "COPILOT_HOME"];

/// The discovery environment of this process: the home directory and the documented
/// location overrides that are set.
#[must_use]
pub fn discovery_env() -> DiscoveryEnv {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let home = if cfg!(windows) {
        var("USERPROFILE").or_else(|| var("HOME"))
    } else {
        var("HOME")
    };
    let location_overrides = LOCATION_VARIABLES
        .iter()
        .filter_map(|name| var(name).map(|path| ((*name).to_owned(), path)))
        .collect();
    DiscoveryEnv {
        home,
        location_overrides,
    }
}

/// Registers every existing default root of every connector (new ones disabled) and
/// returns all known sources.
///
/// # Errors
/// Returns [`IngestError::Store`] on database failure.
pub fn discover(
    store: &Store,
    connectors: &[Box<dyn SourceConnector>],
    env: &DiscoveryEnv,
) -> Result<Vec<SourceRecord>, IngestError> {
    for connector in connectors {
        for root in connector.default_roots(env) {
            if root.is_dir()
                && let Some(text) = root.to_str()
            {
                store.register_source(connector.descriptor().id.as_str(), text, now())?;
            }
        }
    }
    Ok(store.sources()?)
}
