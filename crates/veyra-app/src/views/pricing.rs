//! The cost assumptions panel: the price rules, assumptions and unpriced models behind a
//! scope's API-equivalent cost. The sums come from the analytics engine (ADR-008); this
//! module only joins them with the price catalogue and names.

use jiff::tz::TimeZone;
use veyra_analytics::port::Tally;
use veyra_analytics::pricing::{
    ASSUMPTIONS_DEFINITION, STALE_RATE_DAYS, Unpriced, is_stale, summarize,
};
use veyra_domain::money::MicroUsdPerMTok;
use veyra_domain::pricing::{
    Assumption, CostStatus, EffectiveRange, ModelMatch, Modifiers, PriceRule, RateCard,
};
use veyra_domain::time::UtcMillis;

use super::{internal, local_date};
use crate::DataServices;
use crate::dto::{
    AssumptionKind, AssumptionUse, CostAssumptions, ModifierKind, PriceModifier, PriceTier,
    PricingStatus, PricingTally, Rates, RuleUse, StatusUse, UnpricedModel,
};
use crate::error::ApiError;
use crate::names::DisplayNames;
use crate::scope::Resolved;

fn tally(t: Tally) -> PricingTally {
    PricingTally {
        events: t.events,
        tokens: t.tokens,
        api_equivalent_nano_usd: t.cost.0,
    }
}

const fn status(status: CostStatus) -> PricingStatus {
    match status {
        CostStatus::Priced => PricingStatus::Priced,
        CostStatus::Partial => PricingStatus::Partial,
        CostStatus::UnpricedModel => PricingStatus::UnpricedModel,
        CostStatus::NoUsage => PricingStatus::NoUsage,
    }
}

const fn assumption_kind(assumption: Assumption) -> AssumptionKind {
    match assumption {
        Assumption::UnspecifiedCacheWriteAtShortRate => {
            AssumptionKind::UnspecifiedCacheWriteAtShortRate
        }
        Assumption::PromptSizeUnknownBaseTierUsed => AssumptionKind::PromptSizeUnknownBaseTierUsed,
        Assumption::ServiceTierRateMissing => AssumptionKind::ServiceTierRateMissing,
        Assumption::InferenceGeoRateMissing => AssumptionKind::InferenceGeoRateMissing,
        Assumption::ComponentRateMissing => AssumptionKind::ComponentRateMissing,
        Assumption::HistoryAssumed => AssumptionKind::HistoryAssumed,
    }
}

fn assumption(assumption: Assumption, t: Tally) -> AssumptionUse {
    AssumptionUse {
        kind: assumption_kind(assumption),
        description: assumption.description().to_owned(),
        makes_partial: assumption.makes_partial(),
        tally: tally(t),
    }
}

fn rates(card: &RateCard) -> Rates {
    let price = |rate: Option<MicroUsdPerMTok>| rate.map(|r| r.0);
    Rates {
        input: card.input.0,
        output: card.output.0,
        cache_read: price(card.cache_read),
        cache_write_short: price(card.cache_write_short),
        cache_write_long: price(card.cache_write_long),
        reasoning: price(card.reasoning),
    }
}

fn modifiers(modifiers: &Modifiers) -> Vec<PriceModifier> {
    let tiers = [
        (ModifierKind::Batch, modifiers.batch),
        (ModifierKind::Flex, modifiers.flex),
        (ModifierKind::Priority, modifiers.priority),
        (ModifierKind::Fast, modifiers.fast),
        (ModifierKind::Ultrafast, modifiers.ultrafast),
    ]
    .into_iter()
    .filter_map(|(kind, factor)| {
        factor.map(|f| PriceModifier {
            kind,
            factor_bps: f.factor.0,
            cache_read_factor_bps: f.cache_read.map(|m| m.0),
        })
    });
    let geo = modifiers.inference_geo_us.map(|m| PriceModifier {
        kind: ModifierKind::InferenceGeoUs,
        factor_bps: m.0,
        cache_read_factor_bps: None,
    });
    tiers.chain(geo).collect()
}

/// The first and last UTC dates of a half-open range.
fn effective_dates(range: EffectiveRange) -> Result<(Option<String>, Option<String>), ApiError> {
    let date = |at: UtcMillis| local_date(at, &TimeZone::UTC);
    Ok((
        range.from.map(date).transpose()?,
        range.until.map(|u| date(UtcMillis(u.0 - 1))).transpose()?,
    ))
}

fn rule_use(
    rule: &PriceRule,
    t: Tally,
    names: &DisplayNames,
    now: UtcMillis,
) -> Result<RuleUse, ApiError> {
    let (models, is_prefix) = match &rule.model {
        ModelMatch::Exact(id) => (id.as_str().to_owned(), false),
        ModelMatch::Prefix(prefix) => (prefix.clone(), true),
    };
    let (effective_from, effective_until) = effective_dates(rule.effective)?;
    Ok(RuleUse {
        id: rule.id.clone(),
        provider: names.label_provider(rule.provider.as_str().to_owned()),
        models,
        is_prefix,
        effective_from,
        effective_until,
        base: rates(&rule.base),
        tiers: rule
            .tiers
            .iter()
            .map(|tier| PriceTier {
                above_prompt_tokens: tier.above_prompt_tokens,
                rates: rates(&tier.rates),
            })
            .collect(),
        modifiers: modifiers(&rule.modifiers),
        source_url: rule.source_url.clone(),
        verified_at: rule.verified_at.to_string(),
        is_stale: is_stale(rule.verified_at, now),
        history_assumed: rule.history_assumed,
        tally: tally(t),
    })
}

fn unpriced(model: Unpriced, names: &DisplayNames) -> UnpricedModel {
    UnpricedModel {
        provider: model.provider.map(|p| names.label_provider(p)),
        model: model.model.map(|m| names.label_model(m)),
        tally: tally(model.tally),
    }
}

/// How the scope's cost was estimated, with `now` deciding which prices are stale.
pub(crate) fn cost_assumptions(
    data: &DataServices,
    scope: &Resolved,
    now: UtcMillis,
) -> Result<CostAssumptions, ApiError> {
    let uses = data
        .analytics
        .pricing_uses(&scope.range(scope.period))
        .map_err(|e| internal(&e))?;
    let summary = summarize(&uses);
    let rules = summary
        .rules
        .into_iter()
        .map(|(id, t)| {
            let rule = data
                .catalog
                .rule(&id)
                .ok_or_else(|| internal(&format!("price rule `{id}` is not in the catalogue")))?;
            rule_use(rule, t, &data.names, now)
        })
        .collect::<Result<_, _>>()?;
    Ok(CostAssumptions {
        catalog_version: data.catalog.version().to_owned(),
        stale_after_days: u32::try_from(STALE_RATE_DAYS).unwrap_or(u32::MAX),
        statuses: summary
            .statuses
            .into_iter()
            .map(|(s, t)| StatusUse {
                status: status(s),
                tally: tally(t),
            })
            .collect(),
        assumptions: summary
            .assumptions
            .into_iter()
            .map(|(a, t)| assumption(a, t))
            .collect(),
        rules,
        unpriced: summary
            .unpriced
            .into_iter()
            .map(|m| unpriced(m, &data.names))
            .collect(),
        definition: ASSUMPTIONS_DEFINITION.to_owned(),
    })
}
