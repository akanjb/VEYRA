//! Converts pricing TOML into the domain's validated [`PriceCatalog`].

use veyra_domain::identity::ProviderId;
use veyra_domain::money::{MicroUsdPerMTok, Multiplier};
use veyra_domain::pricing::{
    EffectiveRange, ModelMatch, Modifiers, PriceCatalog, PriceRule, PromptTier, RateCard,
    TierFactor,
};
use veyra_domain::time::CivilDate;

use crate::model::Model;
use crate::raw::{self, CatalogFile, PricingFile, RawModifiers, RawRates, RawRule, RawTierFactor};
use crate::{RegistryError, Sources};

/// Context for error messages while converting one rule.
struct RuleContext<'a> {
    file: &'a str,
    rule: &'a str,
}

impl RuleContext<'_> {
    fn decimal(&self, source: veyra_domain::money::DecimalError) -> RegistryError {
        RegistryError::Decimal {
            file: self.file.to_owned(),
            rule: self.rule.to_owned(),
            source,
        }
    }

    fn date(&self, value: &str) -> Result<CivilDate, RegistryError> {
        CivilDate::parse(value).map_err(|source| RegistryError::Date {
            file: self.file.to_owned(),
            source,
        })
    }

    fn rate(&self, usd: f64) -> Result<MicroUsdPerMTok, RegistryError> {
        MicroUsdPerMTok::from_usd_per_mtok(usd).map_err(|e| self.decimal(e))
    }

    fn optional_rate(&self, usd: Option<f64>) -> Result<Option<MicroUsdPerMTok>, RegistryError> {
        usd.map(|v| self.rate(v)).transpose()
    }

    fn multiplier(&self, factor: f64) -> Result<Multiplier, RegistryError> {
        Multiplier::from_factor(factor).map_err(|e| self.decimal(e))
    }
}

pub fn parse_catalog(
    sources: &Sources<'_>,
    models: &[Model],
) -> Result<PriceCatalog, RegistryError> {
    let meta: CatalogFile = raw::parse("pricing/catalog.toml", sources.catalog)?;
    let mut rules = Vec::new();
    for (file, text) in sources.pricing {
        let parsed: PricingFile = raw::parse(file, text)?;
        for rule in &parsed.rule {
            rules.extend(expand_rule(file, &parsed, rule, models)?);
        }
    }
    Ok(PriceCatalog::new(meta.version, rules)?)
}

/// A TOML rule may list several models with identical prices; each becomes one domain
/// rule with id `<rule id>/<model id>`.
fn expand_rule(
    file: &str,
    parsed: &PricingFile,
    rule: &RawRule,
    models: &[Model],
) -> Result<Vec<PriceRule>, RegistryError> {
    let ctx = RuleContext {
        file,
        rule: &rule.id,
    };
    if rule.models.is_empty() {
        return Err(RegistryError::NoModels {
            file: file.to_owned(),
            rule: rule.id.clone(),
        });
    }
    let defaults = &parsed.defaults;
    let provider =
        ProviderId::new(defaults.provider.clone()).map_err(|source| RegistryError::Id {
            file: file.to_owned(),
            source,
        })?;
    let base = rate_card(&ctx, &rule.rates)?;
    let tiers = rule
        .tier
        .iter()
        .map(|t| {
            Ok(PromptTier {
                above_prompt_tokens: t.above_prompt_tokens,
                rates: rate_card(&ctx, &t.rates)?,
            })
        })
        .collect::<Result<Vec<_>, RegistryError>>()?;
    let effective = EffectiveRange {
        from: rule
            .from
            .as_deref()
            .map(|d| ctx.date(d))
            .transpose()?
            .map(CivilDate::start_utc),
        until: rule
            .until
            .as_deref()
            .map(|d| ctx.date(d))
            .transpose()?
            .map(CivilDate::start_utc),
    };
    let modifiers = modifiers(&ctx, &rule.modifiers)?;
    let verified_at = ctx.date(rule.verified_at.as_deref().unwrap_or(&defaults.verified_at))?;
    let source_url = rule
        .source_url
        .clone()
        .unwrap_or_else(|| defaults.source_url.clone());
    let history_assumed = rule.history_assumed.unwrap_or(defaults.history_assumed);

    rule.models
        .iter()
        .map(|model_id| {
            let model = owned_model(file, rule, model_id, &provider, models)?;
            Ok(PriceRule {
                id: format!("{}/{}", rule.id, model.id),
                provider: provider.clone(),
                model: ModelMatch::Exact(model.id.clone()),
                effective,
                base,
                tiers: tiers.clone(),
                modifiers,
                source_url: source_url.clone(),
                verified_at,
                history_assumed,
            })
        })
        .collect()
}

/// The registry model a rule prices, which must belong to the file's provider.
fn owned_model<'m>(
    file: &str,
    rule: &RawRule,
    model_id: &str,
    provider: &ProviderId,
    models: &'m [Model],
) -> Result<&'m Model, RegistryError> {
    let model = models
        .iter()
        .find(|m| m.id.as_str() == model_id)
        .ok_or_else(|| RegistryError::UnknownReference {
            file: file.to_owned(),
            id: rule.id.clone(),
            kind: "model",
            reference: model_id.to_owned(),
        })?;
    if &model.provider != provider {
        return Err(RegistryError::ProviderMismatch {
            file: file.to_owned(),
            rule: rule.id.clone(),
            model: model_id.to_owned(),
            owner: model.provider.to_string(),
        });
    }
    Ok(model)
}

fn rate_card(ctx: &RuleContext<'_>, raw: &RawRates) -> Result<RateCard, RegistryError> {
    Ok(RateCard {
        input: ctx.rate(raw.input)?,
        output: ctx.rate(raw.output)?,
        cache_read: ctx.optional_rate(raw.cache_read)?,
        cache_write_short: ctx.optional_rate(raw.cache_write_short)?,
        cache_write_long: ctx.optional_rate(raw.cache_write_long)?,
        reasoning: ctx.optional_rate(raw.reasoning)?,
    })
}

fn modifiers(ctx: &RuleContext<'_>, raw: &RawModifiers) -> Result<Modifiers, RegistryError> {
    let tier = |t: Option<RawTierFactor>| -> Result<Option<TierFactor>, RegistryError> {
        t.map(|t| {
            Ok(TierFactor {
                factor: ctx.multiplier(t.factor)?,
                cache_read: t.cache_read.map(|f| ctx.multiplier(f)).transpose()?,
            })
        })
        .transpose()
    };
    Ok(Modifiers {
        batch: tier(raw.batch)?,
        flex: tier(raw.flex)?,
        priority: tier(raw.priority)?,
        fast: tier(raw.fast)?,
        ultrafast: tier(raw.ultrafast)?,
        inference_geo_us: raw
            .inference_geo_us
            .map(|f| ctx.multiplier(f))
            .transpose()?,
    })
}
