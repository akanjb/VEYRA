//! The TOML file formats, exactly as written. Unknown keys are rejected so typos fail
//! loudly instead of silently dropping a rate.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidersFile {
    pub provider: Vec<RawProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProvider {
    pub id: String,
    pub name: String,
    pub pricing_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsFile {
    pub model: Vec<RawModel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawModel {
    pub id: String,
    pub provider: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFile {
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingFile {
    pub defaults: RawDefaults,
    pub rule: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDefaults {
    pub provider: String,
    pub source_url: String,
    pub verified_at: String,
    #[serde(default)]
    pub history_assumed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRule {
    pub id: String,
    pub models: Vec<String>,
    pub from: Option<String>,
    pub until: Option<String>,
    pub rates: RawRates,
    #[serde(default)]
    pub tier: Vec<RawTier>,
    #[serde(default)]
    pub modifiers: RawModifiers,
    pub source_url: Option<String>,
    pub verified_at: Option<String>,
    pub history_assumed: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRates {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write_short: Option<f64>,
    pub cache_write_long: Option<f64>,
    pub reasoning: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTier {
    pub above_prompt_tokens: u64,
    pub rates: RawRates,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawModifiers {
    pub batch: Option<RawTierFactor>,
    pub flex: Option<RawTierFactor>,
    pub priority: Option<RawTierFactor>,
    pub fast: Option<RawTierFactor>,
    pub ultrafast: Option<RawTierFactor>,
    pub inference_geo_us: Option<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTierFactor {
    pub factor: f64,
    pub cache_read: Option<f64>,
}

/// Parses one TOML file, attaching the file name to errors.
pub fn parse<T: for<'de> Deserialize<'de>>(
    file: &str,
    text: &str,
) -> Result<T, crate::RegistryError> {
    toml::from_str(text).map_err(|source| crate::RegistryError::Toml {
        file: file.to_owned(),
        source,
    })
}
