//! Providers and canonical models.

use std::collections::{HashMap, HashSet};

use veyra_domain::identity::{ModelId, ProviderId};

use crate::RegistryError;
use crate::raw::{self, ModelsFile, ProvidersFile};

/// A model provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub id: ProviderId,
    pub name: String,
    pub pricing_url: String,
}

/// A canonical model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub id: ModelId,
    pub provider: ProviderId,
    pub name: String,
    /// Other strings sources report for this model, lower-cased.
    pub aliases: Vec<String>,
}

const PROVIDERS_FILE: &str = "providers.toml";
const MODELS_FILE: &str = "models.toml";

pub fn parse_providers(text: &str) -> Result<Vec<Provider>, RegistryError> {
    let file: ProvidersFile = raw::parse(PROVIDERS_FILE, text)?;
    let mut seen = HashSet::new();
    file.provider
        .into_iter()
        .map(|p| {
            let id = ProviderId::new(p.id).map_err(|source| RegistryError::Id {
                file: PROVIDERS_FILE.to_owned(),
                source,
            })?;
            if !seen.insert(id.clone()) {
                return Err(RegistryError::Duplicate {
                    kind: "provider",
                    id: id.to_string(),
                });
            }
            Ok(Provider {
                id,
                name: p.name,
                pricing_url: p.pricing_url,
            })
        })
        .collect()
}

pub fn parse_models(text: &str, providers: &[Provider]) -> Result<Vec<Model>, RegistryError> {
    let file: ModelsFile = raw::parse(MODELS_FILE, text)?;
    let known: HashSet<&str> = providers.iter().map(|p| p.id.as_str()).collect();
    file.model
        .into_iter()
        .map(|m| {
            let id_error = |source| RegistryError::Id {
                file: MODELS_FILE.to_owned(),
                source,
            };
            if !known.contains(m.provider.as_str()) {
                return Err(RegistryError::UnknownReference {
                    file: MODELS_FILE.to_owned(),
                    id: m.id,
                    kind: "provider",
                    reference: m.provider,
                });
            }
            Ok(Model {
                id: ModelId::new(m.id).map_err(id_error)?,
                provider: ProviderId::new(m.provider).map_err(id_error)?,
                name: m.name,
                aliases: m.aliases.iter().map(|a| a.to_ascii_lowercase()).collect(),
            })
        })
        .collect()
}

/// Maps every id and alias to its model, rejecting any string claimed twice.
pub fn build_lookup(models: &[Model]) -> Result<HashMap<String, usize>, RegistryError> {
    let mut lookup = HashMap::new();
    for (index, model) in models.iter().enumerate() {
        let names =
            std::iter::once(model.id.as_str()).chain(model.aliases.iter().map(String::as_str));
        for name in names {
            if lookup.insert(name.to_owned(), index).is_some() {
                return Err(RegistryError::Duplicate {
                    kind: "model id or alias",
                    id: name.to_owned(),
                });
            }
        }
    }
    Ok(lookup)
}
