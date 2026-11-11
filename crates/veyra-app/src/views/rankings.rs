//! Rankings: the largest groups by tokens or cost and an exact "Other", labelled for
//! display.

use std::collections::BTreeMap;

use veyra_analytics::concentration::share;
use veyra_analytics::dimension::{Dimension, DimensionValue};
use veyra_analytics::engine::GroupResult;
use veyra_analytics::metrics::MetricId;
use veyra_analytics::query::TimeGrain;
use veyra_analytics::top_n::top_n;

use super::{evaluated, internal, measures};
use crate::DataServices;
use crate::dto::{Measures, RankRow};
use crate::error::ApiError;
use crate::scope::Resolved;

/// A group's filter key, display label and (for models) provider.
pub(crate) struct RowLabel {
    pub key: Option<String>,
    pub label: String,
    pub provider: Option<String>,
}

/// The measure a ranking orders groups by, and that its shares are of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RankBy {
    Tokens,
    /// API-equivalent cost.
    Cost,
}

impl RankBy {
    const fn metric(self) -> MetricId {
        match self {
            Self::Tokens => MetricId::TotalTokens,
            Self::Cost => MetricId::ApiEquivalentCost,
        }
    }

    /// The ranked amount of a group (negative cost counts as none).
    pub(crate) fn amount(self, measures: &Measures) -> u64 {
        match self {
            Self::Tokens => measures.tokens,
            Self::Cost => u64::try_from(measures.cost_nano_usd).unwrap_or(0),
        }
    }
}

/// A ranking: the top `n` groups by `by` and an exact "Other". Shares are of
/// `scope_total`, the scope's amount of the same measure.
pub(crate) fn ranking(
    groups: Vec<GroupResult>,
    n: usize,
    by: RankBy,
    scope_total: u64,
    label: impl Fn(&GroupResult) -> RowLabel,
) -> Vec<RankRow> {
    let ranked = top_n(groups, by.metric(), n);
    let mut rows: Vec<RankRow> = ranked
        .top
        .iter()
        .map(|group| {
            let RowLabel {
                key,
                label,
                provider,
            } = label(group);
            let measures = measures(&group.totals);
            RankRow {
                key,
                label,
                provider,
                measures,
                share: share(by.amount(&measures), scope_total),
                group_count: 1,
                is_other: false,
            }
        })
        .collect();
    if let Some(other) = ranked.other {
        let measures = measures(&other.totals);
        rows.push(RankRow {
            key: None,
            label: "Other".to_owned(),
            provider: None,
            measures,
            share: share(by.amount(&measures), scope_total),
            group_count: u64::try_from(other.group_count).unwrap_or(u64::MAX),
            is_other: true,
        });
    }
    rows
}

/// Project id → what the project is called (its name, or else its path).
pub(crate) fn project_labels(data: &DataServices) -> Result<BTreeMap<String, String>, ApiError> {
    Ok(data
        .analytics
        .projects()
        .map_err(|e| internal(&e))?
        .into_iter()
        .map(|p| (p.id.to_string(), p.label().to_owned()))
        .collect())
}

/// How groups of a dimension are keyed and labelled. Models are grouped with their
/// provider, so each model row carries its provider's identity colour.
pub(crate) struct Labeller {
    dimension: Dimension,
    projects: BTreeMap<String, String>,
}

impl Labeller {
    pub(crate) fn new(data: &DataServices, dimension: Dimension) -> Result<Self, ApiError> {
        let projects = if dimension == Dimension::Project {
            project_labels(data)?
        } else {
            BTreeMap::new()
        };
        Ok(Self {
            dimension,
            projects,
        })
    }

    /// The dimensions to group by.
    pub(crate) fn group_by(&self) -> Vec<Dimension> {
        match self.dimension {
            Dimension::Model => vec![Dimension::Provider, Dimension::Model],
            other => vec![other],
        }
    }

    pub(crate) fn label(&self, data: &DataServices, group: &GroupResult) -> RowLabel {
        self.label_key(data, &group.key)
    }

    /// Labels a group key (aligned with [`Self::group_by`]).
    pub(crate) fn label_key(&self, data: &DataServices, group: &[DimensionValue]) -> RowLabel {
        let names = &data.names;
        let key = group.first().cloned().flatten();
        let (key, label, provider) = match self.dimension {
            Dimension::Model => {
                let model = group.get(1).cloned().flatten();
                let label = model
                    .as_deref()
                    .map_or_else(|| "Unknown model".to_owned(), |id| names.model(id));
                (model, label, key)
            }
            Dimension::Provider => {
                let label = key.clone().map_or_else(
                    || "Unknown provider".to_owned(),
                    |k| names.label_provider(k).label,
                );
                (key, label, None)
            }
            Dimension::App => {
                let label = key.clone().map_or_else(
                    || "Unknown application".to_owned(),
                    |k| names.label_app(k).label,
                );
                (key, label, None)
            }
            Dimension::Interface => {
                let label = key.clone().map_or_else(
                    || "Unknown interface".to_owned(),
                    |k| names.label_interface(k).label,
                );
                (key, label, None)
            }
            Dimension::Gateway => {
                let label = key
                    .clone()
                    .map_or_else(|| "Direct".to_owned(), |k| names.label_provider(k).label);
                (key, label, None)
            }
            Dimension::Project => {
                let label = key
                    .as_ref()
                    .and_then(|id| self.projects.get(id).cloned())
                    .unwrap_or_else(|| "No project".to_owned());
                (key, label, None)
            }
            // Device keys arrive with the Organisation Hub; until then all usage is local.
            Dimension::Device => (key, "This device".to_owned(), None),
        };
        RowLabel {
            key,
            label,
            provider,
        }
    }
}

/// The ranking of one dimension over a scope's totals.
pub(crate) fn dimension_ranking(
    data: &DataServices,
    groups: Vec<GroupResult>,
    labeller: &Labeller,
    n: usize,
    by: RankBy,
    scope_total: u64,
) -> Vec<RankRow> {
    ranking(groups, n, by, scope_total, |group| {
        labeller.label(data, group)
    })
}

/// Groups of a scope by one dimension, totals only.
pub(crate) fn groups_of(
    data: &DataServices,
    scope: &Resolved,
    labeller: &Labeller,
) -> Result<Vec<GroupResult>, ApiError> {
    Ok(evaluated(
        data,
        &scope.spec(labeller.group_by(), TimeGrain::Total, false),
    )?
    .groups)
}
