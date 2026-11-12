//! The Projects list, the project detail, and renaming and merging projects (ADR-027).
//! Every number comes from the analytics engine (ADR-008).

use std::collections::BTreeMap;
use std::thread;

use veyra_analytics::activity::{DEFAULT_IDLE_THRESHOLD_MS, Span, activity};
use veyra_analytics::dimension::Dimension;
use veyra_analytics::engine::GroupResult;
use veyra_analytics::metrics::MetricId;
use veyra_analytics::query::TimeGrain;
use veyra_domain::project::{MAX_PROJECT_NAME_CHARS, ProjectChangeError, ProjectRecord};
use veyra_domain::source::PortError;

use super::kpis::metric_kpi;
use super::overview::{composition, daily, kpis, top};
use super::{evaluated, internal, joined, local_date, measures, tokens_by};
use crate::DataServices;
use crate::dto::{
    AccuracyClass, Labelled, ProjectAlias, ProjectDetail, ProjectInfo, ProjectList, ProjectRow,
    SessionSort,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Models and providers ranked in a project detail before "Other".
const TOP_IN_PROJECT: usize = 8;
/// Most expensive sessions listed in a project detail.
const SESSIONS_SHOWN: u32 = 10;

/// A refused project change as the error the dashboard shows.
pub(crate) fn project_error(error: &PortError) -> ApiError {
    match error.downcast_ref::<ProjectChangeError>() {
        Some(ProjectChangeError::NotFound(_)) => ApiError::NotFound {
            message: error.to_string(),
        },
        Some(_) => ApiError::Validation {
            message: error.to_string(),
        },
        None => internal(error),
    }
}

/// Every project, by id.
fn records(data: &DataServices) -> Result<BTreeMap<i64, ProjectRecord>, ApiError> {
    Ok(data
        .projects
        .list_projects()
        .map_err(|e| project_error(&e))?
        .into_iter()
        .map(|p| (p.id, p))
        .collect())
}

fn info(record: &ProjectRecord, all: &BTreeMap<i64, ProjectRecord>) -> ProjectInfo {
    ProjectInfo {
        id: record.id,
        path: record.path.clone(),
        name: record.display_name.clone(),
        label: record
            .display_name
            .clone()
            .unwrap_or_else(|| record.path.clone()),
        aliases: all
            .values()
            .filter(|p| p.merged_into == Some(record.id))
            .map(|p| ProjectAlias {
                id: p.id,
                path: p.path.clone(),
            })
            .collect(),
        merged_into: record.merged_into,
    }
}

/// The model with the most tokens in each project.
fn top_models(
    data: &DataServices,
    scope: &Resolved,
) -> Result<BTreeMap<String, Labelled>, ApiError> {
    let groups = tokens_by(
        data,
        scope,
        vec![Dimension::Project, Dimension::Provider, Dimension::Model],
    )?;
    let mut best: BTreeMap<String, (u64, Labelled)> = BTreeMap::new();
    for group in &groups {
        let (Some(project), Some(model)) = (
            group.key.first().cloned().flatten(),
            group.key.get(2).cloned().flatten(),
        ) else {
            continue;
        };
        let tokens = group.tokens;
        if best.get(&project).is_none_or(|(most, _)| tokens > *most) {
            best.insert(project, (tokens, data.names.label_model(model)));
        }
    }
    Ok(best.into_iter().map(|(k, (_, model))| (k, model)).collect())
}

/// Sessions, active time and the last request of each project.
fn project_activity(
    data: &DataServices,
    scope: &Resolved,
) -> Result<BTreeMap<i64, (u64, i64, i64)>, ApiError> {
    let spans = data
        .analytics
        .project_spans(&scope.range(scope.period))
        .map_err(|e| internal(&e))?;
    let mut per_project: BTreeMap<i64, Vec<Span>> = BTreeMap::new();
    for (project, span) in spans {
        per_project.entry(project).or_default().push(span);
    }
    Ok(per_project
        .into_iter()
        .map(|(project, spans)| {
            let last = spans.iter().map(|span| span.last).max().unwrap_or(0);
            let result = activity(&spans, DEFAULT_IDLE_THRESHOLD_MS);
            (project, (result.sessions, result.active_ms, last))
        })
        .collect())
}

fn row(
    record: &ProjectRecord,
    all: &BTreeMap<i64, ProjectRecord>,
    group: Option<&GroupResult>,
    activity: Option<&(u64, i64, i64)>,
    top_model: Option<&Labelled>,
    days: usize,
) -> ProjectRow {
    ProjectRow {
        project: info(record, all),
        measures: group.map(|g| measures(&g.totals)).unwrap_or_default(),
        sessions: activity.map_or(0, |a| a.0),
        active_ms: activity.map_or(0, |a| a.1),
        top_model: top_model.cloned(),
        last_active_at: activity.map(|a| a.2),
        daily_tokens: group.map_or_else(
            || vec![0; days],
            |g| g.series.iter().map(|p| measures(&p.sums).tokens).collect(),
        ),
    }
}

/// Every project that is not merged into another, with its usage in the scope.
pub(crate) fn list(data: &DataServices, scope: &Resolved) -> Result<ProjectList, ApiError> {
    let all = records(data)?;
    let (result, activity, models) = thread::scope(|s| {
        let activity = s.spawn(|| project_activity(data, scope));
        let models = s.spawn(|| top_models(data, scope));
        let result = evaluated(
            data,
            &scope.spec(vec![Dimension::Project], TimeGrain::Day, false),
        );
        (result, joined(activity), joined(models))
    });
    let (result, activity, models) = (result?, activity?, models?);
    let dates = result
        .series
        .iter()
        .map(|p| local_date(p.start, &scope.time_zone))
        .collect::<Result<Vec<_>, _>>()?;
    let groups: BTreeMap<String, &GroupResult> = result
        .groups
        .iter()
        .filter_map(|g| g.key.first().cloned().flatten().map(|k| (k, g)))
        .collect();
    let mut rows: Vec<ProjectRow> = all
        .values()
        .filter(|p| p.merged_into.is_none())
        .map(|p| {
            let key = p.id.to_string();
            row(
                p,
                &all,
                groups.get(&key).copied(),
                activity.get(&p.id),
                models.get(&key),
                dates.len(),
            )
        })
        .collect();
    rows.sort_by(|a, b| {
        b.measures
            .tokens
            .cmp(&a.measures.tokens)
            .then_with(|| a.project.path.cmp(&b.project.path))
    });
    Ok(ProjectList { dates, rows })
}

/// One project over the scope: the Overview's figures restricted to it.
pub(crate) fn detail(
    data: &DataServices,
    scope: &Resolved,
    id: i64,
) -> Result<ProjectDetail, ApiError> {
    let all = records(data)?;
    let record = all.get(&id).ok_or_else(|| ApiError::NotFound {
        message: format!("project {id} does not exist"),
    })?;
    let project_scope = scope.for_project(id);
    let totals = evaluated(data, &project_scope.spec(vec![], TimeGrain::Total, true))?;
    let current = &totals.totals;
    let previous = totals.previous_totals.as_ref();
    let tokens = measures(current).tokens;
    let (apps, daily_points) = daily(data, &project_scope)?;
    let sessions =
        super::sessions::list(data, &project_scope, SessionSort::Cost, 0, SESSIONS_SHOWN)?;
    Ok(ProjectDetail {
        project: info(record, &all),
        kpis: kpis(data, &project_scope, current, previous)?,
        net_cache_benefit_nano_usd: metric_kpi(
            MetricId::NetCacheBenefit,
            current,
            previous,
            Some(AccuracyClass::Estimated),
        ),
        composition: composition(current),
        apps,
        daily: daily_points,
        models: top(
            data,
            &project_scope,
            Dimension::Model,
            TOP_IN_PROJECT,
            tokens,
        )?,
        providers: top(
            data,
            &project_scope,
            Dimension::Provider,
            TOP_IN_PROJECT,
            tokens,
        )?,
        sessions: sessions.rows,
    })
}

/// Sets or clears a project's name.
pub(crate) fn rename(
    data: &DataServices,
    id: i64,
    name: Option<&str>,
) -> Result<ProjectInfo, ApiError> {
    if name.is_some_and(|n| n.trim().chars().count() > MAX_PROJECT_NAME_CHARS) {
        return Err(ApiError::Validation {
            message: format!("a project name can have at most {MAX_PROJECT_NAME_CHARS} characters"),
        });
    }
    let record = data
        .projects
        .rename_project(id, name)
        .map_err(|e| project_error(&e))?;
    Ok(info(&record, &records(data)?))
}
