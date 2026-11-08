//! The registry: Veyra's single source of truth for providers, models and prices.
//!
//! The data lives in `/registry/*.toml` (reviewed by pull request) and is compiled into
//! the binary. Loading validates every cross-reference, so an inconsistent registry fails
//! the build's tests rather than producing wrong numbers at runtime.

mod error;
mod model;
mod pricing;
mod raw;

use std::collections::HashMap;

use veyra_domain::identity::ModelId;
use veyra_domain::pricing::PriceCatalog;

pub use error::RegistryError;
pub use model::{Model, Provider};

/// The raw TOML sources of a registry.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    pub providers: &'a str,
    pub models: &'a str,
    pub catalog: &'a str,
    /// `(file name, contents)` of each pricing file.
    pub pricing: &'a [(&'a str, &'a str)],
}

/// The registry compiled into this build.
pub const BUNDLED: Sources<'static> = Sources {
    providers: include_str!("../../../registry/providers.toml"),
    models: include_str!("../../../registry/models.toml"),
    catalog: include_str!("../../../registry/pricing/catalog.toml"),
    pricing: &[
        (
            "anthropic.toml",
            include_str!("../../../registry/pricing/anthropic.toml"),
        ),
        (
            "openai.toml",
            include_str!("../../../registry/pricing/openai.toml"),
        ),
        (
            "google.toml",
            include_str!("../../../registry/pricing/google.toml"),
        ),
    ],
};

/// A validated registry.
#[derive(Debug, Clone)]
pub struct Registry {
    providers: Vec<Provider>,
    models: Vec<Model>,
    catalog: PriceCatalog,
    /// Lower-cased id or alias → index into `models`.
    model_lookup: HashMap<String, usize>,
}

impl Registry {
    /// Loads the registry compiled into this build.
    ///
    /// # Errors
    /// Returns [`RegistryError`] if the bundled data is invalid (caught by tests).
    pub fn bundled() -> Result<Self, RegistryError> {
        Self::parse(&BUNDLED)
    }

    /// Parses and validates registry sources.
    ///
    /// # Errors
    /// Returns [`RegistryError`] for malformed TOML, duplicate ids, unknown references or
    /// an inconsistent price catalogue.
    pub fn parse(sources: &Sources<'_>) -> Result<Self, RegistryError> {
        let providers = model::parse_providers(sources.providers)?;
        let models = model::parse_models(sources.models, &providers)?;
        let model_lookup = model::build_lookup(&models)?;
        let catalog = pricing::parse_catalog(sources, &models)?;
        Ok(Self {
            providers,
            models,
            catalog,
            model_lookup,
        })
    }

    #[must_use]
    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    #[must_use]
    pub fn models(&self) -> &[Model] {
        &self.models
    }

    #[must_use]
    pub fn catalog(&self) -> &PriceCatalog {
        &self.catalog
    }

    /// Finds the canonical model for a model string reported by a source, by id or alias
    /// (case-insensitive).
    #[must_use]
    pub fn resolve_model(&self, reported: &str) -> Option<&Model> {
        self.model_lookup
            .get(&reported.trim().to_ascii_lowercase())
            .map(|&index| &self.models[index])
    }

    /// The canonical model with this exact id.
    #[must_use]
    pub fn model(&self, id: &ModelId) -> Option<&Model> {
        self.models.iter().find(|model| &model.id == id)
    }
}

#[cfg(test)]
mod tests;
