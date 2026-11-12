//! The Storage page: the database's size, what it holds and its backups (ADR-033).

use jiff::tz::TimeZone;
use veyra_analytics::dimension::Filters;
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_domain::storage::{Backup, Footprint};
use veyra_domain::time::UtcMillis;

use super::{evaluated, internal, local_time};
use crate::DataServices;
use crate::dto::{BackupInfo, MonthEvents, StorageOverview, TableBytes};
use crate::error::ApiError;

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
