//! The Storage page: the database's size, what it holds and its backups (ADR-033).

use jiff::tz::TimeZone;
use veyra_analytics::dimension::Filters;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_domain::storage::{Backup, BackupContents, DeletionScope, Footprint};
use veyra_domain::time::UtcMillis;

use super::{evaluated, internal, local_time};
use crate::DataServices;
use crate::dto::{
    BackupInfo, DeleteDataRequest, DeletePreview, MonthEvents, StorageOverview, TableBytes,
};
use crate::error::ApiError;
use crate::scope::date;

pub(crate) fn backup_info(backup: Backup) -> BackupInfo {
    BackupInfo {
        name: backup.name,
        created_at: backup.created_at.0,
        bytes: backup.bytes,
    }
}

/// Events per local month from the first event's month to the last's.
fn months(
    data: &DataServices,
    footprint: &Footprint,
    time_zone: &TimeZone,
) -> Result<Vec<MonthEvents>, ApiError> {
    let (Some(first), Some(last)) = (footprint.first_event_at, footprint.last_event_at) else {
        return Ok(Vec::new());
    };
    let first_day = local_time(first, time_zone)?.date().first_of_month();
    let after_last = local_time(last, time_zone)?
        .date()
        .tomorrow()
        .map_err(|e| internal(&e))?;
    let days = first_day
        .until(after_last)
        .map_err(|e| internal(&e))?
        .get_days();
    let period = Period::local_days(first_day, days, time_zone).map_err(|e| internal(&e))?;
    let result = evaluated(
        data,
        &QuerySpec {
            period,
            comparison: None,
            filters: Filters::new(),
            group_by: vec![],
            grain: TimeGrain::Month,
            time_zone: time_zone.clone(),
        },
    )?;
    result
        .series
        .iter()
        .map(|point| {
            let month = local_time(point.start, time_zone)?.date();
            Ok(MonthEvents {
                month: format!("{:04}-{:02}", month.year(), month.month()),
                events: point.sums.events,
            })
        })
        .collect()
}

/// The Storage page in `time_zone`.
pub(crate) fn overview(
    data: &DataServices,
    time_zone: &TimeZone,
) -> Result<StorageOverview, ApiError> {
    let footprint = data.storage.footprint().map_err(|e| internal(&e))?;
    let backups = data.storage.backups().map_err(|e| internal(&e))?;
    Ok(StorageOverview {
        database_bytes: footprint.database_bytes,
        months: months(data, &footprint, time_zone)?,
        tables: footprint
            .tables
            .iter()
            .map(|t| TableBytes {
                table: t.table.clone(),
                bytes: t.bytes,
            })
            .collect(),
        events: footprint.events,
        first_event_at: footprint.first_event_at.map(|at: UtcMillis| at.0),
        last_event_at: footprint.last_event_at.map(|at| at.0),
        backups: backups.into_iter().map(backup_info).collect(),
        kept_backups: data.storage.kept_backups(),
    })
}

/// The stored events a deletion request covers: its local days as UTC bounds on bucket
/// edges, and its connector.
///
/// # Errors
/// [`ApiError::Validation`] for an unknown time zone or connector, a malformed day, or a
/// last day before the first.
pub(crate) fn deletion_scope(
    data: &DataServices,
    request: DeleteDataRequest,
) -> Result<DeletionScope, ApiError> {
    let invalid = |message: String| ApiError::Validation { message };
    let zone = TimeZone::get(&request.time_zone)
        .map_err(|_| invalid(format!("unknown time zone `{}`", request.time_zone)))?;
    let start = request
        .start
        .as_deref()
        .map(|d| date(d, "start"))
        .transpose()?;
    let end = request.end.as_deref().map(|d| date(d, "end")).transpose()?;
    if let (Some(start), Some(end)) = (start, end)
        && end < start
    {
        return Err(invalid("the last day is before the first".to_owned()));
    }
    let day = |date| Period::local_days(date, 1, &zone).map_err(|e| invalid(e.to_string()));
    let known = |connector: &str| {
        data.connectors.iter().any(|c| c.id.as_str() == connector)
            || data
                .capture_routes
                .iter()
                .any(|r| r.descriptor().id.as_str() == connector)
            || data
                .importer
                .apps()
                .iter()
                .any(|a| a.id.as_str() == connector)
    };
    if let Some(connector) = &request.connector_id
        && !known(connector)
    {
        return Err(invalid(format!("unknown connector `{connector}`")));
    }
    Ok(DeletionScope {
        from: start.map(day).transpose()?.map(|p| p.from()),
        until: end.map(day).transpose()?.map(|p| p.until()),
        connector: request.connector_id,
    })
}

/// What a deletion covers, for the dashboard.
pub(crate) fn delete_preview(held: &BackupContents) -> DeletePreview {
    DeletePreview {
        events: held.events,
        first_event_at: held.first_event_at.map(|at| at.0),
        last_event_at: held.last_event_at.map(|at| at.0),
    }
}
