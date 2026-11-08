//! Sources & Health: consent state, freshness, files, stored requests and issues.

use std::collections::HashMap;

use jiff::tz::TimeZone;
use veyra_analytics::dimension::{Dimension, Filters};
use veyra_analytics::engine::evaluate;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_domain::source::{ConnectorDescriptor, SourceHealth, SourceRecord};
use veyra_domain::time::UtcMillis;

use super::internal;
use crate::DataServices;
use crate::dto::{IssueCount, SourceList, SourceSummary};
use crate::error::ApiError;

/// How far back "recent issues" reach.
const ISSUE_WINDOW_MS: i64 = 7 * 86_400_000;
/// One rollup bucket.
const BUCKET_MS: i64 = 15 * 60_000;

/// The connector a source belongs to (absent for connectors not in this build).
fn descriptor<'a>(data: &'a DataServices, connector_id: &str) -> Option<&'a ConnectorDescriptor> {
    data.connectors
        .iter()
        .find(|d| d.id.as_str() == connector_id)
}

/// Builds one source's summary.
pub(crate) fn summary(
    data: &DataServices,
    record: &SourceRecord,
    health: Option<&SourceHealth>,
    events: u64,
) -> SourceSummary {
    let descriptor = descriptor(data, &record.connector_id);
    let health = health.cloned().unwrap_or_default();
    SourceSummary {
        id: record.id,
        connector_id: record.connector_id.clone(),
        display_name: descriptor.map_or_else(
            || record.connector_id.clone(),
            |d| d.display_name.to_owned(),
        ),
        root: record.root.clone(),
        enabled: record.enabled,
        documented_format: descriptor.is_some_and(|d| d.documented_format),
        consented_at: record.consented_at.map(|t| t.0),
        last_scan_at: record.last_scan_at.map(|t| t.0),
        last_success_at: record.last_success_at.map(|t| t.0),
        last_error: record.last_error.clone(),
        files_active: health.files_active,
        files_missing: health.files_missing,
        last_ingested_at: health.last_ingested_at.map(|t| t.0),
        events,
        recent_issues: health
            .recent_issues
            .into_iter()
            .map(|(kind, occurrences)| IssueCount { kind, occurrences })
            .collect(),
    }
}

/// Stored requests per application, all time.
fn events_per_app(data: &DataServices, now: UtcMillis) -> Result<HashMap<String, u64>, ApiError> {
    let until = UtcMillis(now.bucket_start().0 + BUCKET_MS);
    let period = Period::new(UtcMillis(0), until).map_err(|e| internal(&e))?;
    let result = evaluate(
        data.analytics.as_ref(),
        &QuerySpec {
            period,
            comparison: None,
            filters: Filters::new(),
            group_by: vec![Dimension::App],
            grain: TimeGrain::Total,
            time_zone: TimeZone::UTC,
        },
    )
    .map_err(|e| internal(&e))?;
    Ok(result
        .groups
        .into_iter()
        .filter_map(|g| {
            g.key
                .into_iter()
                .next()
                .flatten()
                .map(|app| (app, g.totals.events))
        })
        .collect())
}

/// Every source with its health.
pub(crate) fn list(data: &DataServices, now: UtcMillis) -> Result<SourceList, ApiError> {
    let records = data.sources.sources().map_err(|e| internal(&e))?;
    let health = data
        .sources
        .health(UtcMillis(now.0 - ISSUE_WINDOW_MS))
        .map_err(|e| internal(&e))?;
    let events = events_per_app(data, now)?;
    let sources = records
        .iter()
        .map(|record| {
            let app = descriptor(data, &record.connector_id).map(|d| d.app.as_str());
            summary(
                data,
                record,
                health.iter().find(|h| h.source_id == record.id),
                app.and_then(|a| events.get(a)).copied().unwrap_or(0),
            )
        })
        .collect();
    Ok(SourceList { sources })
}
