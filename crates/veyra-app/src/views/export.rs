//! Reports for export (ADR-032). Every row comes from the analytics engine over the whole
//! scope, with no top-N cut, so a report's rows add up to the totals the pages show for
//! the same filters. Cost kinds stay in separate rows and are never added (ADR-004).

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use veyra_analytics::billing::daily_billing;
use veyra_analytics::dimension::Dimension;
use veyra_analytics::engine::GroupResult;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::TimeGrain;
use veyra_analytics::sums::BaseSums;
use veyra_domain::billing::EntryKind;

use super::kpis::token_accuracy;
use super::rankings::{Labeller, groups_of};
use super::{evaluated, internal, local_date, measures};
use crate::DataServices;
use crate::dto::{AccuracyClass, ExportPreset, ExportRedaction, ExportRequest};
use crate::error::ApiError;
use crate::export::{Cell, Column, ReportInfo, Table};
use crate::scope::{self, Resolved};

/// The token and request measures of every row, from the metric registry.
const MEASURES: [MetricId; 7] = [
    MetricId::InputTokens,
    MetricId::CacheReadTokens,
    MetricId::CacheWriteTokens,
    MetricId::OutputTokens,
    MetricId::ReasoningTokens,
    MetricId::TotalTokens,
    MetricId::Requests,
];
const COST: Column = Column {
    key: "api_equivalent_cost_nano_usd",
    label: "Cost (USD)",
};
const COST_KIND: Column = Column {
    key: "cost_kind",
    label: "Cost kind",
};
const ACCURACY: Column = Column {
    key: "token_accuracy",
    label: "Token accuracy",
};

/// The cost kinds a report names (ADR-004).
const API_EQUIVALENT: &str = "API-equivalent (estimate)";
const SUBSCRIPTION: &str = "Subscription spend";
const CHARGE: &str = "Actual charge";

/// The setting that holds this device's secret for hashing project names.
const HASH_SALT_KEY: &str = "export.project_hash_salt";

/// A report and what it covers.
pub(crate) struct Report {
    pub table: Table,
    pub info: ReportInfo,
}

/// The preset's name in file names and the audit log.
pub(crate) const fn preset_name(preset: ExportPreset) -> &'static str {
    match preset {
        ExportPreset::UsageSummary => "usage-summary",
        ExportPreset::CostSummary => "cost-summary",
        ExportPreset::ByProject => "per-project",
        ExportPreset::ByModel => "per-model",
        ExportPreset::Daily => "daily",
        ExportPreset::Weekly => "weekly",
        ExportPreset::Monthly => "monthly",
    }
}

/// Builds the report a request asks for.
pub(crate) fn report(data: &DataServices, request: ExportRequest) -> Result<Report, ApiError> {
    let scope = scope::resolve(&request.scope)?;
    let (keys, rows) = match request.preset {
        ExportPreset::Daily => by_period(data, &scope, TimeGrain::Day)?,
        ExportPreset::Weekly => by_period(data, &scope, TimeGrain::Week)?,
        ExportPreset::Monthly => by_period(data, &scope, TimeGrain::Month)?,
        ExportPreset::ByProject => by_project(data, &scope, request.redaction)?,
        ExportPreset::ByModel => by_model(data, &scope)?,
        ExportPreset::UsageSummary => by_application(data, &scope)?,
        ExportPreset::CostSummary => cost_summary(data, &scope)?,
    };
    let mut columns = keys;
    columns.extend(MEASURES.iter().map(|metric| {
        let definition = metric.definition();
        Column {
            key: definition.key,
            label: definition.label,
        }
    }));
    columns.extend([COST, COST_KIND, ACCURACY]);
    Ok(Report {
        table: Table { columns, rows },
        info: ReportInfo {
            report: preset_name(request.preset),
            start: request.scope.start,
            end: request.scope.end,
            time_zone: request.scope.time_zone,
            redaction: request.redaction,
        },
    })
}

type Rows = (Vec<Column>, Vec<Vec<Cell>>);

/// The key cells of a row followed by its measures, API-equivalent cost and accuracy.
fn measured(mut cells: Vec<Cell>, sums: &BaseSums) -> Vec<Cell> {
    cells.extend(MEASURES.iter().map(|metric| match metric.compute(sums) {
        Some(MetricValue::Count(n)) => Cell::Count(n),
        _ => Cell::Empty,
    }));
    cells.push(Cell::Money(sums.cost.total));
    cells.push(Cell::Text(API_EQUIVALENT.to_owned()));
    cells.push(token_accuracy(sums).map_or(Cell::Empty, |class| {
        Cell::Text(accuracy_label(class).to_owned())
    }));
    cells
}

const fn accuracy_label(class: AccuracyClass) -> &'static str {
    match class {
        AccuracyClass::Exact => "Exact",
        AccuracyClass::ProviderReported => "Provider-reported",
        AccuracyClass::Partial => "Partial",
        AccuracyClass::Estimated => "Estimated",
        AccuracyClass::Unsupported => "Unsupported",
    }
}

fn key(key: &'static str, label: &'static str) -> Column {
    Column { key, label }
}

/// Groups largest by tokens first, ties by key, so the order is stable.
fn by_tokens(mut groups: Vec<GroupResult>) -> Vec<GroupResult> {
    groups.sort_by(|a, b| {
        measures(&b.totals)
            .tokens
            .cmp(&measures(&a.totals).tokens)
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
}

/// One row per local day, week or month of the scope, empty periods included.
fn by_period(data: &DataServices, scope: &Resolved, grain: TimeGrain) -> Result<Rows, ApiError> {
    let series = evaluated(data, &scope.spec(vec![], grain, false))?.series;
    let rows = series
        .iter()
        .map(|point| {
            Ok(measured(
                vec![Cell::Text(local_date(point.start, &scope.time_zone)?)],
                &point.sums,
            ))
        })
        .collect::<Result<_, ApiError>>()?;
    Ok((vec![key("period_start", "Period start")], rows))
}

fn by_project(
    data: &DataServices,
    scope: &Resolved,
    redaction: ExportRedaction,
) -> Result<Rows, ApiError> {
    let records: BTreeMap<String, (String, Option<String>)> = data
        .projects
        .list_projects()
        .map_err(|e| internal(&e))?
        .into_iter()
        .map(|p| (p.id.to_string(), (p.path, p.display_name)))
        .collect();
    let hash = if redaction.hash_project_names {
        Some(ProjectHash::of(data)?)
    } else {
        None
    };
    // A path names its project, so hashed names never come with paths.
    let with_paths = !(redaction.drop_paths || redaction.hash_project_names);
    let labeller = Labeller::new(data, Dimension::Project)?;
    let rows = by_tokens(groups_of(data, scope, &labeller)?)
        .iter()
        .map(|group| {
            let record = group
                .key
                .first()
                .cloned()
                .flatten()
                .and_then(|id| records.get(&id));
            let name = match (record, &hash) {
                (None, _) => "No project".to_owned(),
                (Some((path, _)), Some(hash)) => hash.name(path),
                (Some((path, name)), None) => name.clone().unwrap_or_else(|| path.clone()),
            };
            let mut cells = vec![Cell::Text(name)];
            if with_paths {
                cells.push(record.map_or(Cell::Empty, |(path, _)| Cell::Text(path.clone())));
            }
            measured(cells, &group.totals)
        })
        .collect();
    let mut keys = vec![key("project", "Project")];
    if with_paths {
        keys.push(key("path", "Path"));
    }
    Ok((keys, rows))
}

fn by_model(data: &DataServices, scope: &Resolved) -> Result<Rows, ApiError> {
    let labeller = Labeller::new(data, Dimension::Model)?;
    let rows = by_tokens(groups_of(data, scope, &labeller)?)
        .iter()
        .map(|group| {
            let label = labeller.label(data, group);
            let provider = label.provider.map_or_else(
                || "Unknown provider".to_owned(),
                |p| data.names.label_provider(p).label,
            );
            measured(
                vec![Cell::Text(provider), Cell::Text(label.label)],
                &group.totals,
            )
        })
        .collect();
    Ok((
        vec![key("provider", "Provider"), key("model", "Model")],
        rows,
    ))
}

/// One row per group of a single dimension, labelled for display.
fn by_dimension(
    data: &DataServices,
    scope: &Resolved,
    dimension: Dimension,
) -> Result<Vec<Vec<Cell>>, ApiError> {
    let labeller = Labeller::new(data, dimension)?;
    Ok(by_tokens(groups_of(data, scope, &labeller)?)
        .iter()
        .map(|group| {
            measured(
                vec![Cell::Text(labeller.label(data, group).label)],
                &group.totals,
            )
        })
        .collect())
}

fn by_application(data: &DataServices, scope: &Resolved) -> Result<Rows, ApiError> {
    Ok((
        vec![key("application", "Application")],
        by_dimension(data, scope, Dimension::App)?,
    ))
}

/// API-equivalent cost per provider, then each recorded subscription and charge in the
/// period (US dollars only, as on the Cost page), each row in its own cost kind.
fn cost_summary(data: &DataServices, scope: &Resolved) -> Result<Rows, ApiError> {
    let mut rows = by_dimension(data, scope, Dimension::Provider)?;
    let days = usize::try_from(scope.days).unwrap_or(0);
    let entries = data.billing.billing_entries().map_err(|e| internal(&e))?;
    for stored in entries {
        let entry = stored.entry;
        let billing = daily_billing(std::slice::from_ref(&entry), scope.start, days);
        let (kind, amount) = match entry.kind {
            EntryKind::Subscription => (SUBSCRIPTION, billing.subscription_total().amount()),
            EntryKind::Charge => (CHARGE, billing.charge_total().amount()),
        };
        // Nothing in the period, or not in US dollars.
        if amount.0 == 0 {
            continue;
        }
        let provider = entry.provider.map_or_else(
            || "All providers".to_owned(),
            |p| data.names.label_provider(p).label,
        );
        let mut cells = vec![Cell::Text(format!("{provider}: {}", entry.label))];
        cells.extend(MEASURES.iter().map(|_| Cell::Empty));
        cells.extend([
            Cell::Money(amount),
            Cell::Text(kind.to_owned()),
            Cell::Empty,
        ]);
        rows.push(cells);
    }
    Ok((vec![key("item", "Provider or item")], rows))
}

/// Stable, secret-salted names for projects: the same project gets the same name in every
/// export from this device, and nobody can recover the folder from it.
struct ProjectHash {
    salt: String,
}

impl ProjectHash {
    fn of(data: &DataServices) -> Result<Self, ApiError> {
        let mut secret = [0_u8; 32];
        getrandom::fill(&mut secret).map_err(|e| internal(&e))?;
        let candidate = serde_json::Value::String(hex(&secret)).to_string();
        let stored = data
            .settings
            .setting_or_init(HASH_SALT_KEY, &candidate)
            .map_err(|e| internal(&e))?;
        let salt = serde_json::from_str::<String>(&stored).map_err(|e| internal(&e))?;
        Ok(Self { salt })
    }

    /// `project-` and the first 12 hex digits of SHA-256 over the salt and the path.
    fn name(&self, path: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.salt.as_bytes());
        hasher.update([0]);
        hasher.update(path.as_bytes());
        let digest = hasher.finalize();
        format!("project-{}", &hex(&digest)[..12])
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}
