//! Data transfer objects: the request and response types of every command.
//!
//! These types are the contract with the dashboard. TypeScript definitions are
//! generated from them (`pnpm contracts:generate`); never edit the generated files.

mod activity;
mod analytics;
mod app_info;
mod billing;
mod cache;
mod cost;
mod models;
mod projects;
mod sessions;
mod sources;
mod usage;

pub use activity::{Activity, ActivityDay, ActivityKpis, ActivityRequest, LengthBin, LongSession};
pub use analytics::{
    AccuracyClass, CalendarDay, Comparison, Composition, DailyPoint, HourCell, Kpi, Labelled,
    Measures, Overview, OverviewKpis, OverviewRequest, RankRow, Scope, ScopeFilters,
};
pub use app_info::{AppInfo, AppInfoRequest, BuildProfile};
pub use billing::{
    BillingChanged, BillingEntryInfo, BillingEntryInput, BillingKind, BillingRecurrence,
    BillingRequest, BillingSettings, BudgetInfo, BudgetInput, DeleteBillingEntryRequest,
    DeleteBudgetRequest, SaveBillingEntryRequest, SaveBudgetRequest,
};
pub use cache::{Cache, CacheKpis, CachePoint, CacheRequest, CacheSession, RatioRow, TtlRow};
pub use cost::{
    BudgetLevel, BudgetStatus, Cost, CostBreakdown, CostDay, CostDefinitions, CostKpis, CostMix,
    CostRequest, MonthOutlook, OutlookDay, RateRow, Waterfall, WaterfallStep,
};
pub use models::{Matrix, ModelLane, ModelRow, Models, ModelsRequest, ShareDay};
pub use projects::{
    MergeProjectRequest, ProjectAlias, ProjectDetail, ProjectInfo, ProjectList, ProjectRequest,
    ProjectRow, ProjectsChanged, ProjectsRequest, RenameProjectRequest, UnmergeProjectRequest,
};
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
