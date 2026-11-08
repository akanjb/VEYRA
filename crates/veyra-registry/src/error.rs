use veyra_domain::identity::IdError;
use veyra_domain::money::DecimalError;
use veyra_domain::pricing::CatalogError;
use veyra_domain::time::DateError;

/// Error raised when registry data is malformed or inconsistent.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("{file}: invalid TOML: {source}")]
    Toml {
        file: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("{file}: {source}")]
    Id {
        file: String,
        #[source]
        source: IdError,
    },
    #[error("{file}: rule `{rule}`: {source}")]
    Decimal {
        file: String,
        rule: String,
        #[source]
        source: DecimalError,
    },
    #[error("{file}: {source}")]
    Date {
        file: String,
        #[source]
        source: DateError,
    },
    #[error("duplicate {kind} `{id}`")]
    Duplicate { kind: &'static str, id: String },
    #[error("{file}: `{id}` refers to unknown {kind} `{reference}`")]
    UnknownReference {
        file: String,
        id: String,
        kind: &'static str,
        reference: String,
    },
    #[error("{file}: rule `{rule}` prices `{model}`, which belongs to provider `{owner}`")]
    ProviderMismatch {
        file: String,
        rule: String,
        model: String,
        owner: String,
    },
    #[error("{file}: rule `{rule}` lists no models")]
    NoModels { file: String, rule: String },
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}
