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
mod export;
mod models;
mod pricing;
mod projects;
mod sessions;
mod sources;
mod storage;
mod tools;
mod usage;
mod what_if;

pub use activity::{Activity, ActivityDay, ActivityKpis, ActivityRequest, LengthBin, LongSession};
pub use analytics::{
    AccuracyClass, CalendarDay, Comparison, Composition, DailyPoint, Highlight, HourCell, Kpi,
    Labelled, Measures, Overview, OverviewKpis, OverviewRequest, RankRow, Scope, ScopeFilters,
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
pub use export::{
    ExportFormat, ExportPreset, ExportPreview, ExportRedaction, ExportRequest, ExportSaved,
};
pub use models::{Matrix, ModelLane, ModelRow, Models, ModelsRequest, ShareDay};
pub use pricing::{
    AssumptionKind, AssumptionUse, CostAssumptions, CostAssumptionsRequest, ModifierKind,
    PriceModifier, PriceTier, PricingStatus, PricingTally, Rates, RuleUse, StatusUse,
    UnpricedModel,
};
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
pub use storage::{
    BackupInfo, CreateBackupRequest, MonthEvents, RestorePreview, RestoreRequest, RestoreScheduled,
    StorageOverview, StorageRequest, TableBytes,
};
pub use tools::{
    McpServerRow, ToolCallCounts, ToolKind, ToolKindRow, ToolRow, Tools, ToolsDay, ToolsKpis,
    ToolsRequest,
};
pub use usage::{
    Breakdown, BreakdownDimension, SizeBinCount, SizePoint, TokenMixPoint, Usage, UsageKpis,
    UsageRequest,
};
pub use what_if::{WhatIf, WhatIfChoice, WhatIfComparison, WhatIfRequest, WhatIfRow};
