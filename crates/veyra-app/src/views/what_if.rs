//! The what-if card on the Cost page: the period's tokens priced on another model
//! (ADR-036). The figures come from the analytics crate (ADR-008); this module only picks
//! the target's price rule and names the rows.

use veyra_analytics::dimension::Dimension;
use veyra_analytics::metrics::{MetricValue, delta};
use veyra_analytics::what_if::{WHAT_IF_DEFINITION, what_if};
use veyra_domain::identity::ModelId;
use veyra_domain::money::NanoUsd;
use veyra_domain::pricing::{ModelMatch, PriceRule};
use veyra_domain::time::UtcMillis;

use super::rankings::{Labeller, groups_of};
use crate::DataServices;
use crate::dto::{WhatIf, WhatIfChoice, WhatIfComparison, WhatIfRow};
use crate::error::ApiError;
use crate::scope::Resolved;

/// A model a rule prices exactly, with the rule.
fn priced_model(rule: &PriceRule) -> Option<&ModelId> {
    match &rule.model {
        ModelMatch::Exact(model) => Some(model),
        ModelMatch::Prefix(_) => None,
    }
}

fn choice(data: &DataServices, rule: &PriceRule, model: &ModelId) -> WhatIfChoice {
    WhatIfChoice {
        model: data.names.label_model(model.as_str().to_owned()),
        provider: data.names.label_provider(rule.provider.as_str().to_owned()),
    }
}

/// Every model with a price in effect at `now`, by provider then name.
fn choices(data: &DataServices, now: UtcMillis) -> Vec<WhatIfChoice> {
    let mut choices: Vec<WhatIfChoice> = data
        .catalog
        .rules()
        .iter()
        .filter(|rule| rule.effective.contains(now))
        .filter_map(|rule| priced_model(rule).map(|model| choice(data, rule, model)))
        .collect();
    choices.sort_by(|a, b| {
        (&a.provider.label, &a.model.label).cmp(&(&b.provider.label, &b.model.label))
    });
    choices
}

/// The relative change from `current` to `repriced`.
fn change(current: NanoUsd, repriced: NanoUsd) -> Option<f64> {
    delta(
        Some(MetricValue::Money(repriced)),
        Some(MetricValue::Money(current)),
    )
    .and_then(|d| d.relative)
}

fn validation(message: String) -> ApiError {
    ApiError::Validation { message }
}

/// The scope's tokens priced on `model` at the rates in effect at `now`.
fn comparison(
    data: &DataServices,
    scope: &Resolved,
    model: &str,
    now: UtcMillis,
) -> Result<WhatIfComparison, ApiError> {
    let id = ModelId::new(model).map_err(|e| validation(e.to_string()))?;
    let rule = data
        .catalog
        .resolve(&id, now)
        .ok_or_else(|| validation(format!("model `{model}` has no current published price")))?;
    let labeller = Labeller::new(data, Dimension::Model)?;
    let groups = groups_of(data, scope, &labeller)?;
    let result = what_if(groups.into_iter().map(|g| (g.key, g.totals)), rule);
    Ok(WhatIfComparison {
        target: choice(data, rule, &id),
        current_nano_usd: result.current.0,
        repriced_nano_usd: result.repriced.0,
        change: change(result.current, result.repriced),
        tokens_compared: result.tokens_compared,
        tokens_left_out: result.tokens_left_out,
        rows: result
            .rows
            .into_iter()
            .map(|row| {
                let label = labeller.label_key(data, &row.key);
                WhatIfRow {
                    key: label.key,
                    label: label.label,
                    provider: label.provider,
                    tokens: row.tokens,
                    current_nano_usd: row.current.0,
                    repriced_nano_usd: row.repriced.0,
                    change: change(row.current, row.repriced),
                }
            })
            .collect(),
        assumptions: result
            .assumptions
            .iter()
            .map(|a| a.description().to_owned())
            .collect(),
    })
}

/// The choices of model and, when `model` is given, the scope priced on it.
pub(crate) fn what_if_view(
    data: &DataServices,
    scope: &Resolved,
    model: Option<&str>,
    now: UtcMillis,
) -> Result<WhatIf, ApiError> {
    Ok(WhatIf {
        choices: choices(data, now),
        comparison: model
            .map(|model| comparison(data, scope, model, now))
            .transpose()?,
        definition: WHAT_IF_DEFINITION.to_owned(),
    })
}
