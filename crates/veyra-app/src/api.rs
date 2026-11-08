//! The application facade: one method per use case, named after its command.

use std::sync::{Arc, OnceLock};

use veyra_domain::source::{ConnectorDescriptor, IngestControl, SourceRecord, SourceRepository};
use veyra_domain::time::UtcMillis;

use crate::dto::{
    AppInfo, AppInfoRequest, BuildProfile, ListSourcesRequest, RescanAccepted, RescanRequest,
    SetSourceEnabledRequest, SourceList, SourceSummary,
};
use crate::error::ApiError;

/// Static facts about the build, supplied by the composition root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMetadata {
    pub product_name: String,
    pub version: String,
    pub build_profile: BuildProfile,
}

/// The database and ingestion ports, attached once they have started.
pub struct DataServices {
    pub sources: Arc<dyn SourceRepository>,
    /// Descriptors of every connector in this build.
    pub connectors: Vec<ConnectorDescriptor>,
    pub ingest: Box<dyn IngestControl>,
}

fn now() -> UtcMillis {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    UtcMillis(millis)
}

impl std::fmt::Debug for DataServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataServices")
            .field("connectors", &self.connectors.len())
            .finish_non_exhaustive()
    }
}

impl DataServices {
    fn summary(&self, record: &SourceRecord) -> SourceSummary {
        let descriptor = self
            .connectors
            .iter()
            .find(|d| d.id.as_str() == record.connector_id);
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
        }
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

    /// Lists every usage source found on this device, enabled or not.
    ///
    /// # Errors
    /// [`ApiError::Busy`] while starting; [`ApiError::Internal`] on database failure.
    pub fn list_sources(&self, _request: ListSourcesRequest) -> Result<SourceList, ApiError> {
        let data = self.data()?;
        let records = data.sources.sources().map_err(|e| ApiError::internal(&e))?;
        Ok(SourceList {
            sources: records.iter().map(|r| data.summary(r)).collect(),
        })
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
            .set_source_enabled(request.source_id, request.enabled, now())
            .map_err(|e| ApiError::internal(&e))?
            .ok_or_else(|| ApiError::NotFound {
                message: format!("source {} does not exist", request.source_id),
            })?;
        data.ingest.sources_changed();
        Ok(data.summary(&record))
    }

    /// Queues a scan of every enabled source; changes arrive as `data-changed` events.
    ///
    /// # Errors
    /// [`ApiError::Busy`] while starting.
    pub fn request_rescan(&self, _request: RescanRequest) -> Result<RescanAccepted, ApiError> {
        self.data()?.ingest.request_scan();
        Ok(RescanAccepted {})
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use veyra_domain::source::IngestControl;
    use veyra_domain::time::UtcMillis;
    use veyra_store::Store;

    use super::{Api, AppMetadata, DataServices};
    use crate::ApiError;
    use crate::dto::{
        AppInfoRequest, BuildProfile, ListSourcesRequest, RescanRequest, SetSourceEnabledRequest,
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
                connectors: veyra_connectors::all()
                    .iter()
                    .map(|c| c.descriptor().clone())
                    .collect(),
                ingest: Box::new(FakeIngest(Arc::clone(&calls))),
            }))
        );
        (api, store, calls, dir)
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

        let listed = api.list_sources(ListSourcesRequest {}).unwrap().sources;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_name, "Claude Code");
        assert!(!listed[0].enabled);
        assert!(!listed[0].documented_format);
        assert_eq!(listed[0].consented_at, None);

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
}
