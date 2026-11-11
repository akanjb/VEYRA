//! Display names of the identifiers users see. The registry names providers and models;
//! connectors name the applications they read. An unknown identifier is shown as itself.

use std::collections::BTreeMap;

use veyra_domain::identity::Interface;
use veyra_domain::source::ConnectorDescriptor;

use crate::dto::Labelled;

/// Identifier → display name, per kind of identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayNames {
    providers: BTreeMap<String, String>,
    models: BTreeMap<String, String>,
    apps: BTreeMap<String, String>,
    interfaces: BTreeMap<String, String>,
}

/// The display name of each interface.
const fn interface_name(interface: Interface) -> &'static str {
    match interface {
        Interface::Cli => "CLI",
        Interface::Desktop => "Desktop app",
        Interface::Ide => "IDE",
        Interface::Browser => "Browser",
        Interface::Api => "API",
        Interface::LocalRuntime => "Local runtime",
    }
}

impl DisplayNames {
    /// Names from the registry's `(id, name)` pairs and the connectors of this build.
    pub fn new(
        providers: impl IntoIterator<Item = (String, String)>,
        models: impl IntoIterator<Item = (String, String)>,
        connectors: &[ConnectorDescriptor],
    ) -> Self {
        Self {
            providers: providers.into_iter().collect(),
            models: models.into_iter().collect(),
            apps: connectors
                .iter()
                .map(|c| (c.app.as_str().to_owned(), c.display_name.to_owned()))
                .collect(),
            interfaces: Interface::ALL
                .into_iter()
                .map(|i| (i.as_str().to_owned(), interface_name(i).to_owned()))
                .collect(),
        }
    }

    /// A model's display name.
    #[must_use]
    pub fn model(&self, id: &str) -> String {
        name_of(&self.models, id)
    }

    pub(crate) fn label_model(&self, id: String) -> Labelled {
        labelled(&self.models, id)
    }

    pub(crate) fn label_provider(&self, id: String) -> Labelled {
        labelled(&self.providers, id)
    }

    pub(crate) fn label_app(&self, id: String) -> Labelled {
        labelled(&self.apps, id)
    }

    pub(crate) fn label_interface(&self, id: String) -> Labelled {
        labelled(&self.interfaces, id)
    }
}

fn name_of(names: &BTreeMap<String, String>, id: &str) -> String {
    names.get(id).cloned().unwrap_or_else(|| id.to_owned())
}

fn labelled(names: &BTreeMap<String, String>, key: String) -> Labelled {
    Labelled {
        label: name_of(names, &key),
        key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_known_ids_and_shows_unknown_ids_as_themselves() {
        let connectors: Vec<ConnectorDescriptor> = veyra_connectors::all()
            .iter()
            .map(|c| c.descriptor().clone())
            .collect();
        let names = DisplayNames::new(
            [("anthropic".to_owned(), "Anthropic".to_owned())],
            [("m-1".to_owned(), "Model One".to_owned())],
            &connectors,
        );
        let label = |l: Labelled| (l.key, l.label);
        assert_eq!(
            label(names.label_provider("anthropic".to_owned())),
            ("anthropic".to_owned(), "Anthropic".to_owned())
        );
        assert_eq!(
            label(names.label_provider("someone-new".to_owned())),
            ("someone-new".to_owned(), "someone-new".to_owned())
        );
        assert_eq!(names.model("m-1"), "Model One");
        assert_eq!(
            names.label_app("claude-code".to_owned()).label,
            "Claude Code"
        );
    }
}
