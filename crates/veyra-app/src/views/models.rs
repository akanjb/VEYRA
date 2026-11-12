//! The Models & Providers page: which models are used, how they compare, and where.
//! Every number comes from the analytics engine (ADR-008).

use std::collections::BTreeMap;
use std::thread;

use veyra_analytics::concentration::share;
use veyra_analytics::dimension::{Dimension, DimensionValue};
use veyra_analytics::engine::GroupResult;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::port::TokenRow;
use veyra_analytics::query::TimeGrain;

use super::kpis::token_accuracy;
use super::rankings::{Labeller, RowLabel};
use super::{evaluated, joined, local_date, measures, tokens_by};
use crate::DataServices;
use crate::dto::{Labelled, Matrix, ModelLane, ModelRow, Models, ShareDay};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Models shown in the timeline and the model × application matrix.
const TOP_MODELS: usize = 12;

/// Groups of one query, largest by tokens first (ties by key, so the order is stable).
fn by_tokens(mut groups: Vec<GroupResult>) -> Vec<GroupResult> {
    groups.sort_by(|a, b| {
        measures(&b.totals)
            .tokens
            .cmp(&measures(&a.totals).tokens)
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
}

fn grouped(
    data: &DataServices,
    scope: &Resolved,
    group_by: Vec<Dimension>,
    grain: TimeGrain,
) -> Result<Vec<GroupResult>, ApiError> {
    Ok(by_tokens(
        evaluated(data, &scope.spec(group_by, grain, false))?.groups,
    ))
}

fn labelled(label: RowLabel) -> Labelled {
    Labelled {
        key: label.key.unwrap_or_default(),
        label: label.label,
    }
}

fn model_row(
    data: &DataServices,
    labeller: &Labeller,
    group: &GroupResult,
    dates: &[String],
) -> ModelRow {
    let label = labeller.label(data, group);
    let sums = &group.totals;
    let active: Vec<usize> = group
        .series
        .iter()
        .enumerate()
        .filter(|(_, point)| point.sums.events > 0)
        .map(|(i, _)| i)
        .collect();
    let provider_label = label.provider.clone().map_or_else(
        || "Unknown provider".to_owned(),
        |p| data.names.label_provider(p).label,
    );
    ModelRow {
        key: label.key,
        label: label.label,
        provider: label.provider,
        provider_label,
        measures: measures(sums),
        cost_per_million_output_nano_usd: match MetricId::CostPerMillionOutputTokens.compute(sums) {
            Some(MetricValue::Money(money)) => Some(money.0),
            _ => None,
        },
        cache_hit_ratio: MetricId::CacheHitRatio
            .compute(sums)
            .map(MetricValue::as_f64),
        pricing_coverage: MetricId::PricingCoverage
            .compute(sums)
            .map(MetricValue::as_f64),
        first_day: active
            .first()
            .and_then(|i| dates.get(*i).cloned())
            .unwrap_or_default(),
        last_day: active
            .last()
            .and_then(|i| dates.get(*i).cloned())
            .unwrap_or_default(),
    }
}

/// Each provider's share of each day's tokens.
fn provider_share(
    data: &DataServices,
    scope: &Resolved,
    dates: &[String],
) -> Result<(Vec<Labelled>, Vec<ShareDay>), ApiError> {
    let labeller = Labeller::new(data, Dimension::Provider)?;
    let groups = grouped(data, scope, vec![Dimension::Provider], TimeGrain::Day)?;
    let daily: Vec<Vec<u64>> = groups
        .iter()
        .map(|g| g.series.iter().map(|p| measures(&p.sums).tokens).collect())
        .collect();
    let days = dates
        .iter()
        .enumerate()
        .map(|(day, date)| {
            let total: u64 = daily
                .iter()
                .map(|series| series.get(day).copied().unwrap_or(0))
                .sum();
            ShareDay {
                date: date.clone(),
                shares: daily
                    .iter()
                    .map(|series| share(series.get(day).copied().unwrap_or(0), total))
                    .collect(),
            }
        })
        .collect();
    let providers = groups
        .iter()
        .map(|g| labelled(labeller.label(data, g)))
        .collect();
    Ok((providers, days))
}

/// Tokens of `cells` arranged by the row and column keys they belong to.
fn matrix(
    rows: Vec<(Vec<DimensionValue>, Labelled)>,
    columns: Vec<(DimensionValue, Labelled)>,
    cells: &[TokenRow],
    split: impl Fn(&TokenRow) -> (Vec<DimensionValue>, DimensionValue),
) -> Matrix {
    let row_index: BTreeMap<&Vec<DimensionValue>, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, (key, _))| (key, i))
        .collect();
    let column_index: BTreeMap<&DimensionValue, usize> = columns
        .iter()
        .enumerate()
        .map(|(i, (key, _))| (key, i))
        .collect();
    let mut values = vec![vec![0_u64; columns.len()]; rows.len()];
    for cell in cells {
        let (row, column) = split(cell);
        if let (Some(r), Some(c)) = (row_index.get(&row), column_index.get(&column)) {
            values[*r][*c] += cell.tokens;
        }
    }
    Matrix {
        rows: rows.into_iter().map(|(_, l)| l).collect(),
        columns: columns.into_iter().map(|(_, l)| l).collect(),
        values,
    }
}

/// Builds the Models & Providers page for a scope.
pub(crate) fn models(data: &DataServices, scope: &Resolved) -> Result<Models, ApiError> {
    let daily = evaluated(data, &scope.spec(vec![], TimeGrain::Day, false))?;
    let dates = daily
        .series
        .iter()
        .map(|p| local_date(p.start, &scope.time_zone))
        .collect::<Result<Vec<_>, _>>()?;
    let model_labels = Labeller::new(data, Dimension::Model)?;
    thread::scope(|s| {
        let share = s.spawn(|| provider_share(data, scope, &dates));
        let model_groups =
            s.spawn(|| grouped(data, scope, model_labels.group_by(), TimeGrain::Day));
        let app_groups = s.spawn(|| grouped(data, scope, vec![Dimension::App], TimeGrain::Total));
        let provider_groups =
            s.spawn(|| grouped(data, scope, vec![Dimension::Provider], TimeGrain::Total));
        let provider_cells =
            s.spawn(|| tokens_by(data, scope, vec![Dimension::Provider, Dimension::App]));
        let model_cells = tokens_by(
            data,
            scope,
            vec![Dimension::Provider, Dimension::Model, Dimension::App],
        )?;
        let (providers, provider_share) = joined(share)?;
        let model_groups = joined(model_groups)?;
        let leaderboard: Vec<ModelRow> = model_groups
            .iter()
            .map(|g| model_row(data, &model_labels, g, &dates))
            .collect();
        let top_models: Vec<&GroupResult> = model_groups.iter().take(TOP_MODELS).collect();

        let app_labels = Labeller::new(data, Dimension::App)?;
        let app_groups = joined(app_groups)?;
        let apps: Vec<(DimensionValue, Labelled)> = app_groups
            .iter()
            .map(|g| {
                (
                    g.key.first().cloned().flatten(),
                    labelled(app_labels.label(data, g)),
                )
            })
            .collect();

        let provider_labels = Labeller::new(data, Dimension::Provider)?;
        let provider_rows = joined(provider_groups)?
            .iter()
            .map(|g| (g.key.clone(), labelled(provider_labels.label(data, g))))
            .collect();
        let provider_cells = joined(provider_cells)?;
        let model_rows = top_models
            .iter()
            .map(|g| (g.key.clone(), labelled(model_labels.label(data, g))))
            .collect();

        Ok(Models {
            accuracy: token_accuracy(&daily.totals),
            providers,
            provider_share,
            leaderboard,
            provider_by_app: matrix(provider_rows, apps.clone(), &provider_cells, |g| {
                (g.key[..1].to_vec(), g.key.get(1).cloned().flatten())
            }),
            model_by_app: matrix(model_rows, apps.clone(), &model_cells, |g| {
                (g.key[..2].to_vec(), g.key.get(2).cloned().flatten())
            }),
            apps: apps.into_iter().map(|(_, l)| l).collect(),
            timeline: top_models
                .iter()
                .map(|g| {
                    let label = model_labels.label(data, g);
                    ModelLane {
                        key: label.key,
                        label: label.label,
                        provider: label.provider,
                        tokens: g.series.iter().map(|p| measures(&p.sums).tokens).collect(),
                    }
                })
                .collect(),
            dates: dates.clone(),
            rate_definition: MetricId::CostPerMillionOutputTokens
                .definition()
                .description
                .to_owned(),
        })
    })
}
