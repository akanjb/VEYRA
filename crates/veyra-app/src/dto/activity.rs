use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{CalendarDay, HourCell, Kpi, Labelled, Scope};

/// Request for [`crate::Api::get_activity`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRequest {
    pub scope: Scope,
}

/// Everything the Activity page shows for a scope. Times are milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub kpis: ActivityKpis,
    /// The 365 local days ending on the scope's last day (filters apply).
    pub calendar: Vec<CalendarDay>,
    /// 7 × 24 cells in the scope's time zone.
    pub hours: Vec<HourCell>,
    /// One point per local day of the period.
    pub daily: Vec<ActivityDay>,
    /// Sessions per length range, by active and by wall time.
    pub session_lengths: Vec<LengthBin>,
    /// The sessions with the longest wall time.
    pub longest: Vec<LongSession>,
    /// The idle threshold that caps gaps in active time.
    #[ts(type = "number")]
    pub idle_threshold_ms: i64,
    pub active_definition: String,
    pub wall_definition: String,
}

/// The Activity headline tiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityKpis {
    pub active_ms: Kpi,
    pub wall_ms: Kpi,
    pub sessions: Kpi,
    /// Local days with at least one request.
    pub active_days: Kpi,
}

/// One local day's activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDay {
    pub date: String,
    #[ts(type = "number")]
    pub sessions: u64,
    #[ts(type = "number")]
    pub active_ms: i64,
    #[ts(type = "number")]
    pub wall_ms: i64,
}

/// Sessions whose length is in `[lower_ms, upper_ms)`; `upper_ms` is `null` for the last,
/// open-ended range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LengthBin {
    #[ts(type = "number")]
    pub lower_ms: i64,
    #[ts(type = "number | null")]
    pub upper_ms: Option<i64>,
    /// Sessions in this range by active time.
    #[ts(type = "number")]
    pub by_active: u64,
    /// Sessions in this range by wall time.
    #[ts(type = "number")]
    pub by_wall: u64,
}

/// A long-running session, over the events in the period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LongSession {
    #[ts(type = "number")]
    pub id: i64,
    pub app: String,
    pub project: Option<String>,
    pub models: Vec<Labelled>,
    #[ts(type = "number")]
    pub started_at: i64,
    #[ts(type = "number")]
    pub ended_at: i64,
    #[ts(type = "number")]
    pub active_ms: i64,
    #[ts(type = "number")]
    pub wall_ms: i64,
}
