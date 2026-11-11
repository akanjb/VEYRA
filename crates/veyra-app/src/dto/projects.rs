use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::analytics::{
    Composition, DailyPoint, Kpi, Labelled, Measures, OverviewKpis, RankRow, Scope,
};
use super::sessions::SessionRow;

/// Request for [`crate::Api::list_projects`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsRequest {
    pub scope: Scope,
}

/// Every project with its usage in a scope. Projects merged into another appear as
/// that project's aliases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectList {
    /// Local days of the period, for the sparklines.
    pub dates: Vec<String>,
    /// Projects with the most tokens first, then those without usage in the period.
    pub rows: Vec<ProjectRow>,
}

/// What identifies a project and what the user called it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    #[ts(type = "number")]
    pub id: i64,
    /// Normalised project root path.
    pub path: String,
    /// The name the user gave it.
    pub name: Option<String>,
    /// The name, or else the path.
    pub label: String,
    /// The projects merged into this one.
    pub aliases: Vec<ProjectAlias>,
    /// The project this one is merged into, if any.
    #[ts(type = "number | null")]
    pub merged_into: Option<i64>,
}

/// A project merged into another, which can be unmerged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAlias {
    #[ts(type = "number")]
    pub id: i64,
    pub path: String,
}

/// One project in the list, over the scope's period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub project: ProjectInfo,
    pub measures: Measures,
    #[ts(type = "number")]
    pub sessions: u64,
    /// Derived active time (gaps capped at the idle threshold).
    #[ts(type = "number")]
    pub active_ms: i64,
    /// The model with the most tokens.
    pub top_model: Option<Labelled>,
    /// The last request in the period (UTC epoch milliseconds).
    #[ts(type = "number | null")]
    pub last_active_at: Option<i64>,
    /// Tokens per local day, aligned with [`ProjectList::dates`].
    #[ts(type = "number[]")]
    pub daily_tokens: Vec<u64>,
}

/// Request for [`crate::Api::get_project`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRequest {
    pub scope: Scope,
    #[ts(type = "number")]
    pub id: i64,
}

/// One project over a scope: the Overview's figures for this project only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: ProjectInfo,
    pub kpis: OverviewKpis,
    /// API-equivalent net cache benefit in nano-USD.
    pub net_cache_benefit_nano_usd: Kpi,
    pub composition: Composition,
    /// Applications in the daily trend, largest first.
    pub apps: Vec<Labelled>,
    pub daily: Vec<DailyPoint>,
    pub models: Vec<RankRow>,
    pub providers: Vec<RankRow>,
    /// The most expensive sessions of the project in the period.
    pub sessions: Vec<SessionRow>,
}

/// Request for [`crate::Api::rename_project`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameProjectRequest {
    #[ts(type = "number")]
    pub id: i64,
    /// The new name; `null` or blank restores the path.
    pub name: Option<String>,
}

/// Request for [`crate::Api::merge_project`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeProjectRequest {
    /// The project merged away; its usage counts toward `into` from now on.
    #[ts(type = "number")]
    pub from: i64,
    #[ts(type = "number")]
    pub into: i64,
}

/// Request for [`crate::Api::unmerge_project`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnmergeProjectRequest {
    #[ts(type = "number")]
    pub id: i64,
}

/// A merge or unmerge was applied; every view reflects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsChanged {}
