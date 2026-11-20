//! Page budgets: the command behind every dashboard page, called through the application
//! facade exactly as the desktop shell calls it, over the last 30 days with comparison.

use std::path::PathBuf;
use std::sync::Arc;

use veyra_app::dto::{
    ActivityRequest, BuildProfile, CacheRequest, Comparison, CostAssumptionsRequest, CostRequest,
    ModelsRequest, OverviewRequest, ProjectsRequest, Scope, ScopeFilters, SessionSort,
    SessionsRequest, ToolsRequest, UsageRequest, WhatIfRequest,
};
use veyra_app::{
    Api, ApiError, AppControl, AppMetadata, DataServices, DisplayNames, FilePicker, FileSaver,
};
use veyra_domain::capture::{CaptureProxy, ProxyStatus};
use veyra_domain::import::{ArchiveImport, ImportError, ImportSummary, ReadSeek};
use veyra_domain::pricing::PriceCatalog;
use veyra_domain::source::ConnectorDescriptor;
use veyra_domain::source::IngestControl;
use veyra_domain::telemetry::{ReceiverStatus, TelemetryReceiver};
use veyra_store::{Storage, Store};

use crate::timing::{Budget, timed};

/// The dataset's last 30 local days in [`TIME_ZONE`]; it ends at 2026-11-08T00:00Z.
pub const FIRST_DAY: &str = "2026-10-08";
pub const LAST_DAY: &str = "2026-11-06";
pub const TIME_ZONE: &str = "Asia/Kolkata";

/// Ingestion is not running while pages are timed.
struct NoIngest;

impl IngestControl for NoIngest {
    fn request_scan(&self) {}
    fn sources_changed(&self) {}
}

/// No receiver runs while pages are timed.
struct NoReceiver;

impl TelemetryReceiver for NoReceiver {
    fn set_enabled(&self, _: bool) {}
    fn status(&self) -> ReceiverStatus {
        ReceiverStatus::default()
    }
}

/// No proxy runs while pages are timed.
struct NoProxy;

impl CaptureProxy for NoProxy {
    fn set_enabled(&self, _: bool) {}
    fn status(&self) -> ProxyStatus {
        ProxyStatus::default()
    }
}

/// Nothing restarts while pages are timed.
struct NoRestart;

impl AppControl for NoRestart {
    fn restart(&self) {}
}

/// Reports are not saved while pages are timed.
struct NoFiles;

impl FileSaver for NoFiles {
    fn save(&self, _: &str, _: &str, _: &[u8]) -> Result<bool, String> {
        Ok(false)
    }
}

impl FilePicker for NoFiles {
    fn pick(&self, _: &str, _: &[&str]) -> Result<Option<Box<dyn ReadSeek>>, String> {
        Ok(None)
    }
}

/// Nothing is imported while pages are timed.
struct NoImports;

impl ArchiveImport for NoImports {
    fn apps(&self) -> Vec<ConnectorDescriptor> {
        Vec::new()
    }
    fn import(&self, _: &mut dyn ReadSeek) -> Result<ImportSummary, ImportError> {
        Err(ImportError::NotRecognized)
    }
}

/// An API over the store, wired like the desktop shell.
fn api(store: &Arc<Store>, catalog: PriceCatalog, names: DisplayNames) -> Api {
    let api = Api::new(AppMetadata {
        product_name: "Veyra".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        build_profile: BuildProfile::Release,
    });
    api.attach(Ok(DataServices {
        sources: Arc::clone(store) as _,
        projects: Arc::clone(store) as _,
        billing: Arc::clone(store) as _,
        settings: Arc::clone(store) as _,
        audit: Arc::clone(store) as _,
        // Pages never back up; the paths are never used.
        storage: Arc::new(Storage::new(
            Arc::clone(store),
            PathBuf::from("veyra.db"),
            std::env::temp_dir().join("veyra-perf-backups"),
        )),
        analytics: Arc::clone(store) as _,
        catalog: Arc::new(catalog),
        connectors: Vec::new(),
        names,
        ingest: Box::new(NoIngest),
        telemetry: Box::new(NoReceiver),
        proxy: Box::new(NoProxy),
        capture_routes: Vec::new(),
        importer: Box::new(NoImports),
        files: Box::new(NoFiles),
        picker: Box::new(NoFiles),
        control: Box::new(NoRestart),
    }));
    api
}

fn scope() -> Scope {
    Scope {
        time_zone: TIME_ZONE.to_owned(),
        start: FIRST_DAY.to_owned(),
        end: LAST_DAY.to_owned(),
        comparison: Comparison::Previous,
        filters: ScopeFilters::default(),
    }
}

type PageCall = fn(&Api) -> Result<(), ApiError>;

/// Each page and the command it loads.
const PAGES: [(&str, PageCall); 11] = [
    ("Overview", |api| {
        api.get_overview(OverviewRequest { scope: scope() })
            .map(drop)
    }),
    ("Usage", |api| {
        api.get_usage(UsageRequest { scope: scope() }).map(drop)
    }),
    ("Cost", |api| {
        api.get_cost(CostRequest { scope: scope() }).map(drop)
    }),
    ("Cost assumptions", |api| {
        api.get_cost_assumptions(CostAssumptionsRequest { scope: scope() })
            .map(drop)
    }),
    ("What if (Claude Haiku 5.5)", |api| {
        api.get_what_if(WhatIfRequest {
            scope: scope(),
            model: Some("claude-haiku-5-5".to_owned()),
        })
        .map(drop)
    }),
    ("Cache", |api| {
        api.get_cache(CacheRequest { scope: scope() }).map(drop)
    }),
    ("Models & Providers", |api| {
        api.get_models(ModelsRequest { scope: scope() }).map(drop)
    }),
    ("Activity", |api| {
        api.get_activity(ActivityRequest { scope: scope() })
            .map(drop)
    }),
    ("Tools & Agents", |api| {
        api.get_tools(ToolsRequest { scope: scope() }).map(drop)
    }),
    ("Projects", |api| {
        api.list_projects(ProjectsRequest { scope: scope() })
            .map(drop)
    }),
    ("Sessions (first page)", |api| {
        api.list_sessions(SessionsRequest {
            scope: scope(),
            sort: SessionSort::LastActive,
            page: 0,
            page_size: 50,
        })
        .map(drop)
    }),
];

/// Times every page. Returns whether each p95 is within `budget`.
pub fn page_commands(
    store: &Arc<Store>,
    catalog: PriceCatalog,
    names: DisplayNames,
    budget: Budget,
) -> Result<bool, Box<dyn std::error::Error>> {
    let api = api(store, catalog, names);
    budget.heading("Dashboard pages (last 30 days with comparison)");
    let mut passed = true;
    for (name, call) in PAGES {
        passed &= budget.check(name, timed(|| call(&api))?);
    }
    Ok(passed)
}
