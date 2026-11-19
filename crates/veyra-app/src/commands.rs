//! The command catalogue: the only place where commands are declared.
//!
//! Each entry reads `command_name(RequestDto) -> ResponseDto;` and maps 1:1 to the
//! [`crate::Api`] method of the same name. Consumers expand the catalogue through a
//! callback macro, so the Tauri handlers, the Tauri permission allow-list and the
//! generated TypeScript command map can never drift apart.

/// Expands `$callback!` with every command in the catalogue.
///
/// The callback macro receives a list of `name(Request) -> Response;` entries.
#[macro_export]
macro_rules! for_each_command {
    ($callback:ident) => {
        $callback! {
            get_app_info($crate::dto::AppInfoRequest) -> $crate::dto::AppInfo;
            list_sources($crate::dto::ListSourcesRequest) -> $crate::dto::SourceList;
            set_source_enabled($crate::dto::SetSourceEnabledRequest) -> $crate::dto::SourceSummary;
            request_rescan($crate::dto::RescanRequest) -> $crate::dto::RescanAccepted;
            get_telemetry_receiver($crate::dto::TelemetryReceiverRequest) -> $crate::dto::TelemetryReceiverInfo;
            set_telemetry_receiver($crate::dto::SetTelemetryReceiverRequest) -> $crate::dto::TelemetryReceiverInfo;
            get_overview($crate::dto::OverviewRequest) -> $crate::dto::Overview;
            get_usage($crate::dto::UsageRequest) -> $crate::dto::Usage;
            get_cost($crate::dto::CostRequest) -> $crate::dto::Cost;
            get_cost_assumptions($crate::dto::CostAssumptionsRequest) -> $crate::dto::CostAssumptions;
            get_what_if($crate::dto::WhatIfRequest) -> $crate::dto::WhatIf;
            get_cache($crate::dto::CacheRequest) -> $crate::dto::Cache;
            get_models($crate::dto::ModelsRequest) -> $crate::dto::Models;
            get_activity($crate::dto::ActivityRequest) -> $crate::dto::Activity;
            get_tools($crate::dto::ToolsRequest) -> $crate::dto::Tools;
            list_projects($crate::dto::ProjectsRequest) -> $crate::dto::ProjectList;
            get_project($crate::dto::ProjectRequest) -> $crate::dto::ProjectDetail;
            rename_project($crate::dto::RenameProjectRequest) -> $crate::dto::ProjectInfo;
            merge_project($crate::dto::MergeProjectRequest) -> $crate::dto::ProjectsChanged;
            unmerge_project($crate::dto::UnmergeProjectRequest) -> $crate::dto::ProjectsChanged;
            get_billing($crate::dto::BillingRequest) -> $crate::dto::BillingSettings;
            save_billing_entry($crate::dto::SaveBillingEntryRequest) -> $crate::dto::BillingEntryInfo;
            delete_billing_entry($crate::dto::DeleteBillingEntryRequest) -> $crate::dto::BillingChanged;
            save_budget($crate::dto::SaveBudgetRequest) -> $crate::dto::BudgetInfo;
            delete_budget($crate::dto::DeleteBudgetRequest) -> $crate::dto::BillingChanged;
            preview_export($crate::dto::ExportRequest) -> $crate::dto::ExportPreview;
            save_export($crate::dto::ExportRequest) -> $crate::dto::ExportSaved;
            get_storage($crate::dto::StorageRequest) -> $crate::dto::StorageOverview;
            create_backup($crate::dto::CreateBackupRequest) -> $crate::dto::BackupInfo;
            preview_restore($crate::dto::RestoreRequest) -> $crate::dto::RestorePreview;
            restore_backup($crate::dto::RestoreRequest) -> $crate::dto::RestoreScheduled;
            preview_delete($crate::dto::DeleteDataRequest) -> $crate::dto::DeletePreview;
            delete_data($crate::dto::DeleteDataRequest) -> $crate::dto::DataDeleted;
            list_sessions($crate::dto::SessionsRequest) -> $crate::dto::SessionList;
            get_session($crate::dto::SessionRequest) -> $crate::dto::SessionDetail;
        }
    };
}

/// Expands `$callback!` with every event the core emits to the dashboard.
///
/// The callback macro receives a list of `NAME_CONSTANT => Payload;` entries.
#[macro_export]
macro_rules! for_each_event {
    ($callback:ident) => {
        $callback! {
            $crate::events::DATA_CHANGED => $crate::dto::DataChanged;
        }
    };
}

macro_rules! command_names {
    ($($name:ident($request:ty) -> $response:ty;)*) => {
        /// Names of every command, in catalogue order.
        pub const COMMAND_NAMES: &[&str] = &[$(stringify!($name)),*];
    };
}

for_each_command!(command_names);

#[cfg(test)]
mod tests {
    use super::COMMAND_NAMES;

    #[test]
    fn command_names_are_unique_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for name in COMMAND_NAMES {
            assert!(seen.insert(*name), "duplicate command `{name}`");
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "command `{name}` must be snake_case"
            );
        }
    }
}
