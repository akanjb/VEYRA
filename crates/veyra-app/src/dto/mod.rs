//! Data transfer objects: the request and response types of every command.
//!
//! These types are the contract with the dashboard. TypeScript definitions are
//! generated from them (`pnpm contracts:generate`); never edit the generated files.

mod activity;
mod analytics;
mod app_info;
mod cache;
mod models;
mod sessions;
mod sources;
mod usage;

pub use activity::{Activity, ActivityDay, ActivityKpis, ActivityRequest, LengthBin, LongSession};
pub use analytics::{
    AccuracyClass, CalendarDay, Comparison, Composition, DailyPoint, HourCell, Kpi, Labelled,
    Measures, Overview, OverviewKpis, OverviewRequest, RankRow, Scope, ScopeFilters,
};
pub use app_info::{AppInfo, AppInfoRequest, BuildProfile};
pub use cache::{Cache, CacheKpis, CachePoint, CacheRequest, CacheSession, RatioRow, TtlRow};
pub use models::{Matrix, ModelLane, ModelRow, Models, ModelsRequest, ShareDay};
pub use sessions::{
    SessionDetail, SessionList, SessionRequest, SessionRow, SessionSort, SessionsRequest,
    TimelineEvent, TokenBreakdown, ToolCallSummary,
};
pub use sources::{
    Capability, DataChanged, IssueCount, ListSourcesRequest, RescanAccepted, RescanRequest,
    SetSourceEnabledRequest, SourceList, SourceSummary, SupportLevel,
};
pub use usage::{
    Breakdown, BreakdownDimension, SizeBinCount, SizePoint, TokenMixPoint, Usage, UsageKpis,
    UsageRequest,
};
