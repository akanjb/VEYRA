//! The application facade: one method per use case, named after its command.

use std::sync::{Arc, OnceLock};

use veyra_analytics::port::{
    ActivityReader, BaseSumsReader, LabelReader, PricingReader, SizeReader,
};
use veyra_analytics::sessions::SessionReader;
use veyra_domain::billing::BillingRepository;
use veyra_domain::pricing::PriceCatalog;
use veyra_domain::project::ProjectRepository;
use veyra_domain::source::{ConnectorDescriptor, IngestControl, SourceRepository};
use veyra_domain::time::UtcMillis;

use crate::dto::{
    Activity, ActivityRequest, AppInfo, AppInfoRequest, BillingChanged, BillingEntryInfo,
    BillingRequest, BillingSettings, BudgetInfo, BuildProfile, Cache, CacheRequest, Cost,
    CostAssumptions, CostAssumptionsRequest, CostRequest, DeleteBillingEntryRequest,
    DeleteBudgetRequest, ListSourcesRequest, MergeProjectRequest, Models, ModelsRequest, Overview,
    OverviewRequest, ProjectDetail, ProjectInfo, ProjectList, ProjectRequest, ProjectsChanged,
    ProjectsRequest, RenameProjectRequest, RescanAccepted, RescanRequest, SaveBillingEntryRequest,
    SaveBudgetRequest, SessionDetail, SessionList, SessionRequest, SessionsRequest,
    SetSourceEnabledRequest, SourceList, SourceSummary, UnmergeProjectRequest, Usage, UsageRequest,
};
use crate::error::ApiError;
use crate::names::DisplayNames;
use crate::{scope, views};

/// Static facts about the build, supplied by the composition root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMetadata {
    pub product_name: String,
    pub version: String,
    pub build_profile: BuildProfile,
}

/// Every analytics read port, as one object (implemented by the store).
pub trait AnalyticsReader:
    BaseSumsReader
    + ActivityReader
    + SessionReader
    + LabelReader
    + SizeReader
    + PricingReader
    + Send
    + Sync
{
}

impl<T> AnalyticsReader for T where
    T: BaseSumsReader
        + ActivityReader
        + SessionReader
        + LabelReader
        + SizeReader
        + PricingReader
        + Send
        + Sync
{
}

/// The database and ingestion ports, attached once they have started.
pub struct DataServices {
    pub sources: Arc<dyn SourceRepository>,
    pub projects: Arc<dyn ProjectRepository>,
    pub billing: Arc<dyn BillingRepository>,
    pub analytics: Arc<dyn AnalyticsReader>,
    /// The price catalogue events are priced with.
    pub catalog: Arc<PriceCatalog>,
    /// Descriptors of every connector in this build.
    pub connectors: Vec<ConnectorDescriptor>,
    /// Display names of providers, models and applications.
    pub names: DisplayNames,
    pub ingest: Box<dyn IngestControl>,
}

impl std::fmt::Debug for DataServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataServices")
            .field("connectors", &self.connectors.len())
            .finish_non_exhaustive()
    }
}

/// Entry point for every use case exposed to the presentation layer.
///
/// Method names must match the command catalogue in `commands.rs`.
#[derive(Debug)]
pub struct Api {
    metadata: AppMetadata,
    /// Set once at startup: the services, or why they could not start.
    data: OnceLock<Result<DataServices, ApiError>>,
}

impl Api {
    #[must_use]
    pub fn new(metadata: AppMetadata) -> Self {
        Self {
            metadata,
            data: OnceLock::new(),
        }
    }

    /// Attaches the data services, or records why they failed to start. Only the first
    /// call has an effect; returns whether it was this one.
    pub fn attach(&self, services: Result<DataServices, String>) -> bool {
        self.data
            .set(services.map_err(|reason| ApiError::internal(&reason)))
            .is_ok()
    }

    fn data(&self) -> Result<&DataServices, ApiError> {
        match self.data.get() {
            Some(Ok(services)) => Ok(services),
            Some(Err(error)) => Err(error.clone()),
            None => Err(ApiError::Busy {
                message: "the database is still starting".to_owned(),
            }),
        }
    }

    /// Returns product name, version, build profile and platform.
    ///
    /// # Errors
    /// Never fails today; the `Result` keeps the signature uniform across commands.
    pub fn get_app_info(&self, _request: AppInfoRequest) -> Result<AppInfo, ApiError> {
        Ok(AppInfo {
            product_name: self.metadata.product_name.clone(),
            version: self.metadata.version.clone(),
            build_profile: self.metadata.build_profile,
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        })
    }

    /// Lists every usage source found on this device, enabled or not, with its health.
    ///
    /// # Errors
    /// [`ApiError::Busy`] while starting; [`ApiError::Internal`] on database failure.
    pub fn list_sources(&self, _request: ListSourcesRequest) -> Result<SourceList, ApiError> {
        views::sources::list(self.data()?, UtcMillis::now())
    }

    /// Enables (recording consent) or disables a source, then lets ingestion react: an
    /// enabled source is read at once; a disabled one is no longer read or watched.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown source; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn set_source_enabled(
        &self,
        request: SetSourceEnabledRequest,
    ) -> Result<SourceSummary, ApiError> {
        let data = self.data()?;
        let record = data
            .sources
            .set_source_enabled(request.source_id, request.enabled, UtcMillis::now())
            .map_err(|e| ApiError::internal(&e))?
            .ok_or_else(|| ApiError::NotFound {
                message: format!("source {} does not exist", request.source_id),
            })?;
        data.ingest.sources_changed();
        Ok(views::sources::list(data, UtcMillis::now())?
            .sources
            .into_iter()
            .find(|s| s.id == record.id)
            .unwrap_or_else(|| views::sources::summary(data, &record, None, 0)))
    }

    /// Queues a scan of every enabled source; changes arrive as `data-changed` events.
    ///
    /// # Errors
    /// [`ApiError::Busy`] while starting.
    pub fn request_rescan(&self, _request: RescanRequest) -> Result<RescanAccepted, ApiError> {
        self.data()?.ingest.request_scan();
        Ok(RescanAccepted {})
    }

    /// The Overview page for a scope: headline numbers with comparisons, accuracy, the
    /// daily trend by application, model and project rankings, calendar and hours.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_overview(&self, request: OverviewRequest) -> Result<Overview, ApiError> {
        let OverviewRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::overview::overview(self.data()?, &resolved)
    }

    /// The Usage page for a scope: token kinds over time, breakdowns with concentration,
    /// and request and session sizes.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_usage(&self, request: UsageRequest) -> Result<Usage, ApiError> {
        let UsageRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::usage::usage(self.data()?, &resolved)
    }

    /// The Cost page for a scope: API-equivalent cost with its trend, month outlook,
    /// budgets, breakdowns, composition, effective rates and period change, beside the
    /// subscription spend and charges the user recorded (never added together).
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_cost(&self, request: CostRequest) -> Result<Cost, ApiError> {
        let CostRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::cost::cost(self.data()?, &resolved, UtcMillis::now())
    }

    /// How the scope's API-equivalent cost was estimated: the price rules applied, the
    /// assumptions made and the models with no price.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_cost_assumptions(
        &self,
        request: CostAssumptionsRequest,
    ) -> Result<CostAssumptions, ApiError> {
        let CostAssumptionsRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::pricing::cost_assumptions(self.data()?, &resolved, UtcMillis::now())
    }

    /// Every billing entry and budget, for editing.
    ///
    /// # Errors
    /// [`ApiError::Busy`] while starting; [`ApiError::Internal`] on database failure.
    pub fn get_billing(&self, request: BillingRequest) -> Result<BillingSettings, ApiError> {
        let BillingRequest {} = request;
        views::billing::settings(self.data()?)
    }

    /// Records a subscription or charge, or replaces one.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid entry; [`ApiError::NotFound`] for an unknown
    /// id; [`ApiError::Busy`] while starting; [`ApiError::Internal`] on database failure.
    pub fn save_billing_entry(
        &self,
        request: SaveBillingEntryRequest,
    ) -> Result<BillingEntryInfo, ApiError> {
        let SaveBillingEntryRequest { id, entry } = request;
        views::billing::save_entry(self.data()?, id, entry)
    }

    /// Deletes a billing entry.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown id; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn delete_billing_entry(
        &self,
        request: DeleteBillingEntryRequest,
    ) -> Result<BillingChanged, ApiError> {
        let DeleteBillingEntryRequest { id } = request;
        self.data()?
            .billing
            .delete_billing_entry(id)
            .map_err(|e| views::billing::billing_error(&e))?;
        Ok(BillingChanged {})
    }

    /// Creates a monthly budget, or replaces one.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid budget; [`ApiError::NotFound`] for an
    /// unknown id or project; [`ApiError::Busy`] while starting; [`ApiError::Internal`] on
    /// database failure.
    pub fn save_budget(&self, request: SaveBudgetRequest) -> Result<BudgetInfo, ApiError> {
        let SaveBudgetRequest { id, budget } = request;
        views::billing::save_budget(self.data()?, id, budget)
    }

    /// Deletes a budget.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown id; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn delete_budget(&self, request: DeleteBudgetRequest) -> Result<BillingChanged, ApiError> {
        let DeleteBudgetRequest { id } = request;
        self.data()?
            .billing
            .delete_budget(id)
            .map_err(|e| views::billing::billing_error(&e))?;
        Ok(BillingChanged {})
    }

    /// The Cache page for a scope: net benefit, savings and overhead with their price
    /// coverage, reads and writes over time, hit ratios, writes per TTL and the sessions
    /// whose cache writes cost more than they saved.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_cache(&self, request: CacheRequest) -> Result<Cache, ApiError> {
        let CacheRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::cache::cache(self.data()?, &resolved)
    }

    /// The Models & Providers page for a scope: provider shares over time, the model
    /// leaderboard, provider and model use per application, and when each model was used.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_models(&self, request: ModelsRequest) -> Result<Models, ApiError> {
        let ModelsRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::models::models(self.data()?, &resolved)
    }

    /// The Activity page for a scope: active and wall time per day, the calendar, hours of
    /// the week, session lengths and the longest sessions.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_activity(&self, request: ActivityRequest) -> Result<Activity, ApiError> {
        let ActivityRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::activity::activity(self.data()?, &resolved)
    }

    /// Every project that is not merged into another, with its usage in the scope.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn list_projects(&self, request: ProjectsRequest) -> Result<ProjectList, ApiError> {
        let ProjectsRequest { scope: requested } = request;
        let resolved = scope::resolve(&requested)?;
        views::projects::list(self.data()?, &resolved)
    }

    /// One project over the scope: headline numbers, daily trend, model and provider mix
    /// and its most expensive sessions.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown project; [`ApiError::Validation`] for an
    /// invalid scope; [`ApiError::Busy`] while starting; [`ApiError::Internal`] on
    /// database failure.
    pub fn get_project(&self, request: ProjectRequest) -> Result<ProjectDetail, ApiError> {
        let ProjectRequest {
            scope: requested,
            id,
        } = request;
        let resolved = scope::resolve(&requested)?;
        views::projects::detail(self.data()?, &resolved, id)
    }

    /// Sets or clears a project's name.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown project; [`ApiError::Validation`] for a name
    /// that is too long; [`ApiError::Busy`] while starting; [`ApiError::Internal`] on
    /// database failure.
    pub fn rename_project(&self, request: RenameProjectRequest) -> Result<ProjectInfo, ApiError> {
        let RenameProjectRequest { id, name } = request;
        views::projects::rename(self.data()?, id, name.as_deref())
    }

    /// Merges a project into another: its usage, past and future, counts toward the
    /// other one, until the merge is undone.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown project; [`ApiError::Validation`] for a merge
    /// into itself; [`ApiError::Busy`] while starting; [`ApiError::Internal`] on database
    /// failure.
    pub fn merge_project(&self, request: MergeProjectRequest) -> Result<ProjectsChanged, ApiError> {
        let MergeProjectRequest { from, into } = request;
        self.data()?
            .projects
            .merge_project(from, into)
            .map_err(|e| views::projects::project_error(&e))?;
        Ok(ProjectsChanged {})
    }

    /// Undoes a merge: the project's usage counts as its own again.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown project; [`ApiError::Validation`] for a project
    /// that is not merged; [`ApiError::Busy`] while starting; [`ApiError::Internal`] on
    /// database failure.
    pub fn unmerge_project(
        &self,
        request: UnmergeProjectRequest,
    ) -> Result<ProjectsChanged, ApiError> {
        let UnmergeProjectRequest { id } = request;
        self.data()?
            .projects
            .unmerge_project(id)
            .map_err(|e| views::projects::project_error(&e))?;
        Ok(ProjectsChanged {})
    }

    /// One page of sessions with activity in the scope, in the requested order.
    ///
    /// # Errors
    /// [`ApiError::Validation`] for an invalid scope or page; [`ApiError::Busy`] while
    /// starting; [`ApiError::Internal`] on database failure.
    pub fn list_sessions(&self, request: SessionsRequest) -> Result<SessionList, ApiError> {
        let SessionsRequest {
            scope: requested,
            sort,
            page,
            page_size,
        } = request;
        let resolved = scope::resolve(&requested)?;
        views::sessions::list(self.data()?, &resolved, sort, page, page_size)
    }

    /// A whole session: totals, the request timeline with tool names, and its subagents.
    ///
    /// # Errors
    /// [`ApiError::NotFound`] for an unknown session; [`ApiError::Busy`] while starting;
    /// [`ApiError::Internal`] on database failure.
    pub fn get_session(&self, request: SessionRequest) -> Result<SessionDetail, ApiError> {
        views::sessions::detail(self.data()?, request.id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use veyra_domain::source::IngestControl;
    use veyra_domain::time::UtcMillis;
    use veyra_store::Store;

    use super::{Api, AppMetadata, DataServices, DisplayNames};
    use crate::ApiError;
    use crate::dto::{
        AppInfoRequest, BuildProfile, Comparison, ListSourcesRequest, OverviewRequest,
        RescanRequest, Scope, ScopeFilters, SessionRequest, SessionSort, SessionsRequest,
        SetSourceEnabledRequest,
    };

    fn api() -> Api {
        Api::new(AppMetadata {
            product_name: "Veyra".to_owned(),
            version: "1.2.3".to_owned(),
            build_profile: BuildProfile::Release,
        })
    }

    #[derive(Default)]
    struct Calls {
        scans: AtomicUsize,
        changes: AtomicUsize,
    }

    struct FakeIngest(Arc<Calls>);

    impl IngestControl for FakeIngest {
        fn request_scan(&self) {
            self.0.scans.fetch_add(1, Ordering::Relaxed);
        }
        fn sources_changed(&self) {
            self.0.changes.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// An API with a store in a fresh temporary directory (removed by the caller).
    fn attached(name: &str) -> (Api, Arc<Store>, Arc<Calls>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("veyra-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(Store::open(&dir.join("veyra.db")).unwrap());
        let calls = Arc::new(Calls::default());
        let api = api();
        assert!(
            api.attach(Ok(DataServices {
                sources: Arc::clone(&store) as _,
                projects: Arc::clone(&store) as _,
                billing: Arc::clone(&store) as _,
                analytics: Arc::clone(&store) as _,
                catalog: Arc::new(
                    veyra_registry::Registry::bundled()
                        .unwrap()
                        .catalog()
                        .clone()
                ),
                connectors: veyra_connectors::all()
                    .iter()
                    .map(|c| c.descriptor().clone())
                    .collect(),
                names: DisplayNames::default(),
                ingest: Box::new(FakeIngest(Arc::clone(&calls))),
            }))
        );
        (api, store, calls, dir)
    }

    fn day_scope(tz: &str) -> Scope {
        Scope {
            time_zone: tz.to_owned(),
            start: "2026-11-08".to_owned(),
            end: "2026-11-08".to_owned(),
            comparison: Comparison::Previous,
            filters: ScopeFilters::default(),
        }
    }

    #[test]
    fn app_info_reports_metadata_and_platform() {
        let info = api().get_app_info(AppInfoRequest {});
        assert_eq!(
            info.map(|i| (i.product_name, i.version, i.build_profile, i.os, i.arch)),
            Ok((
                "Veyra".to_owned(),
                "1.2.3".to_owned(),
                BuildProfile::Release,
                std::env::consts::OS.to_owned(),
                std::env::consts::ARCH.to_owned(),
            ))
        );
    }

    #[test]
    fn data_commands_report_busy_until_attached_and_the_failure_after() {
        let api = api();
        assert!(matches!(
            api.list_sources(ListSourcesRequest {}),
            Err(ApiError::Busy { .. })
        ));
        assert!(api.attach(Err("disk full".to_owned())));
        assert!(matches!(
            api.request_rescan(RescanRequest {}),
            Err(ApiError::Internal { .. })
        ));
        assert!(
            !api.attach(Err("again".to_owned())),
            "only the first attach counts"
        );
    }

    #[test]
    fn enabling_a_source_records_consent_and_notifies_ingestion() {
        let (api, store, calls, dir) = attached("enable");
        let source = store
            .register_source("claude-code", "/home/dev/.claude/projects", UtcMillis(5))
            .unwrap();

        let list = api.list_sources(ListSourcesRequest {}).unwrap();
        // Every other tool of this build is reported as not found.
        assert_eq!(
            list.not_found,
            ["Codex", "Gemini CLI", "OpenCode", "GitHub Copilot CLI"]
        );
        let listed = list.sources;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_name, "Claude Code");
        let labels: Vec<&str> = listed[0]
            .capabilities
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "Tokens",
                "Cache",
                "Reasoning",
                "Tools",
                "Sessions",
                "Projects"
            ]
        );
        assert!(
            listed[0]
                .capabilities
                .iter()
                .all(|c| (c.note.is_some()) == (c.support == crate::dto::SupportLevel::Partial)),
            "a partial capability always says why"
        );
        assert!(!listed[0].enabled);
        assert!(!listed[0].documented_format);
        assert_eq!(listed[0].consented_at, None);
        assert_eq!((listed[0].files_active, listed[0].events), (0, 0));

        let enabled = api
            .set_source_enabled(SetSourceEnabledRequest {
                source_id: source.id,
                enabled: true,
            })
            .unwrap();
        assert!(enabled.enabled);
        assert!(enabled.consented_at.is_some());
        assert_eq!(calls.changes.load(Ordering::Relaxed), 1);

        assert!(matches!(
            api.set_source_enabled(SetSourceEnabledRequest {
                source_id: 999,
                enabled: true,
            }),
            Err(ApiError::NotFound { .. })
        ));

        api.request_rescan(RescanRequest {}).unwrap();
        assert_eq!(calls.scans.load(Ordering::Relaxed), 1);
        drop((api, store));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn analytics_commands_validate_and_handle_an_empty_database() {
        let (api, store, _calls, dir) = attached("empty");
        let overview = api
            .get_overview(OverviewRequest {
                scope: day_scope("UTC"),
            })
            .unwrap();
        assert_eq!(overview.kpis.sessions.value, Some(0.0));
        assert_eq!(overview.daily.len(), 1);
        assert_eq!(overview.calendar.len(), 365);
        assert_eq!(overview.hours.len(), 7 * 24);
        assert_eq!(overview.models.len(), 0);

        assert!(matches!(
            api.get_overview(OverviewRequest {
                scope: day_scope("Nowhere/Invalid"),
            }),
            Err(ApiError::Validation { .. })
        ));
        assert!(matches!(
            api.list_sessions(SessionsRequest {
                scope: day_scope("UTC"),
                sort: SessionSort::LastActive,
                page: 0,
                page_size: 0,
            }),
            Err(ApiError::Validation { .. })
        ));
        assert!(matches!(
            api.get_session(SessionRequest { id: 42 }),
            Err(ApiError::NotFound { .. })
        ));
        drop((api, store));
        let _ = std::fs::remove_dir_all(dir);
    }
}
