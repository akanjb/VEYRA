//! Contract tests of report exports (ADR-032), on the ingested Claude Code fixtures:
//! every report adds up to the figures the pages show, redaction leaves no project name
//! or path in the file, cost kinds stay in separate rows, and each saved report is
//! audited without personal data.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use veyra_app::Api;
use veyra_app::dto::{
    BillingEntryInput, BillingKind, BillingRecurrence, ExportFormat, ExportPreset, ExportRedaction,
    ExportRequest, Overview, OverviewRequest, SaveBillingEntryRequest,
};
use veyra_domain::audit::{AuditAction, AuditActor, AuditLog};

use support::{SavedFiles, api, api_with_files, ingested, scope};

const PRESETS: [ExportPreset; 7] = [
    ExportPreset::UsageSummary,
    ExportPreset::CostSummary,
    ExportPreset::ByProject,
    ExportPreset::ByModel,
    ExportPreset::Daily,
    ExportPreset::Weekly,
    ExportPreset::Monthly,
];

fn request(
    preset: ExportPreset,
    format: ExportFormat,
    redaction: ExportRedaction,
) -> ExportRequest {
    ExportRequest {
        scope: scope(),
        preset,
        format,
        redaction,
    }
}

fn overview(api: &Api) -> Overview {
    api.get_overview(OverviewRequest { scope: scope() })
        .unwrap()
}

/// The JSON report of `preset`.
fn json(
    pair: &(Api, SavedFiles),
    preset: ExportPreset,
    redaction: ExportRedaction,
) -> serde_json::Value {
    let (api, files) = pair;
    api.save_export(request(preset, ExportFormat::Json, redaction))
        .unwrap();
    let saved = files.lock().unwrap().pop().unwrap();
    serde_json::from_slice(&saved.1).unwrap()
}

#[test]
fn every_report_adds_up_to_the_overview() {
    let data = ingested();
    let (api, files) = api_with_files(&data);
    let page = overview(&api);
    let tokens = page.kpis.tokens.value.unwrap();
    let cost = page.kpis.cost_nano_usd.value.unwrap();
    for preset in PRESETS {
        api.save_export(request(
            preset,
            ExportFormat::Json,
            ExportRedaction::default(),
        ))
        .unwrap();
        let (_, bytes) = files.lock().unwrap().pop().unwrap();
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let rows = report["rows"].as_array().unwrap();
        let usage = rows
            .iter()
            .filter(|row| row["cost_kind"] == "API-equivalent (estimate)");
        let (sum_tokens, sum_cost) = usage.fold((0_u64, 0_i64), |(t, c), row| {
            (
                t + row["total_tokens"].as_u64().unwrap_or(0),
                c + row["api_equivalent_cost_nano_usd"].as_i64().unwrap(),
            )
        });
        #[expect(clippy::cast_precision_loss, reason = "small test totals")]
        let (sum_tokens, sum_cost) = (sum_tokens as f64, sum_cost as f64);
        assert_eq!(sum_tokens, tokens, "{preset:?} tokens");
        assert_eq!(sum_cost, cost, "{preset:?} cost");
    }
}

#[test]
fn hashed_reports_name_no_project_and_no_path() {
    let data = ingested();
    let pair = api_with_files(&data);
    let hashed = ExportRedaction {
        hash_project_names: true,
        drop_paths: false,
    };
    let first = json(&pair, ExportPreset::ByProject, hashed);
    let again = json(&pair, ExportPreset::ByProject, hashed);
    let text = first.to_string();
    assert!(!text.contains("demo"), "no name or path: {text}");
    assert!(first["rows"][0].get("path").is_none(), "no path column");
    let name = first["rows"][0]["project"].as_str().unwrap();
    assert!(name.starts_with("project-") && name.len() == "project-".len() + 12);
    assert_eq!(again["rows"][0]["project"], name, "stable on this device");

    let plain = json(&pair, ExportPreset::ByProject, ExportRedaction::default());
    assert!(
        plain["rows"][0]["path"]
            .as_str()
            .unwrap()
            .contains("work/demo")
    );
    let no_paths = json(
        &pair,
        ExportPreset::ByProject,
        ExportRedaction {
            hash_project_names: false,
            drop_paths: true,
        },
    );
    assert!(no_paths["rows"][0].get("path").is_none());
}

#[test]
fn csv_previews_show_the_written_rows() {
    let data = ingested();
    let (api, files) = api_with_files(&data);
    let ask = request(
        ExportPreset::ByModel,
        ExportFormat::Csv,
        ExportRedaction::default(),
    );
    let preview = api.preview_export(ask.clone()).unwrap();
    assert_eq!(
        preview.file_name,
        "veyra-per-model-2026-11-08-to-2026-11-08.csv"
    );
    api.save_export(ask).unwrap();
    let (name, bytes) = files.lock().unwrap().pop().unwrap();
    assert_eq!(name, preview.file_name);
    let text = String::from_utf8(bytes).unwrap();
    let mut lines = text.split("\r\n");
    assert_eq!(lines.next().unwrap(), preview.columns.join(","));
    for row in &preview.rows {
        assert_eq!(lines.next().unwrap(), row.join(","));
    }
    assert_eq!(
        usize::try_from(preview.row_count).unwrap(),
        preview.rows.len()
    );
}

#[test]
fn recorded_billing_stays_in_its_own_cost_kind() {
    let data = ingested();
    let pair = api_with_files(&data);
    pair.0
        .save_billing_entry(SaveBillingEntryRequest {
            id: None,
            entry: BillingEntryInput {
                kind: BillingKind::Subscription,
                label: "Plan".to_owned(),
                provider: Some("anthropic".to_owned()),
                amount_nano: 30_000_000_000,
                currency: "USD".to_owned(),
                recurrence: BillingRecurrence::Monthly,
                starts_on: "2026-11-01".to_owned(),
                ends_on: None,
            },
        })
        .unwrap();
    let report = json(&pair, ExportPreset::CostSummary, ExportRedaction::default());
    let rows = report["rows"].as_array().unwrap();
    let plan = rows
        .iter()
        .find(|row| row["cost_kind"] == "Subscription spend")
        .unwrap();
    // One day of a $30 monthly plan in November.
    assert_eq!(plan["api_equivalent_cost_nano_usd"], 1_000_000_000);
    assert!(plan["total_tokens"].is_null());
    assert!(
        rows.iter()
            .any(|row| row["cost_kind"] == "API-equivalent (estimate)")
    );
}

#[test]
fn saved_reports_are_audited_without_personal_data() {
    let data = ingested();
    let (api, _files) = api_with_files(&data);
    api.preview_export(request(
        ExportPreset::ByProject,
        ExportFormat::Csv,
        ExportRedaction::default(),
    ))
    .unwrap();
    assert!(
        data.store.audit_entries(10).unwrap().is_empty(),
        "previews are not audited"
    );
    api.save_export(request(
        ExportPreset::ByProject,
        ExportFormat::CsvExcel,
        ExportRedaction {
            hash_project_names: true,
            drop_paths: true,
        },
    ))
    .unwrap();
    let entries = data.store.audit_entries(10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].actor, AuditActor::User);
    assert_eq!(entries[0].action, AuditAction::Export);
    assert_eq!(
        entries[0].detail,
        "per-project report, CSV for Excel, 1 row, project names hashed, paths left out"
    );
}

#[test]
fn an_invalid_scope_is_refused() {
    let data = ingested();
    let mut ask = request(
        ExportPreset::Daily,
        ExportFormat::Csv,
        ExportRedaction::default(),
    );
    ask.scope.time_zone = "Nowhere/Else".to_owned();
    assert!(matches!(
        api(&data).preview_export(ask),
        Err(veyra_app::ApiError::Validation { .. })
    ));
}

#[test]
fn workbooks_are_written_and_audited() {
    let data = ingested();
    let (api, files) = api_with_files(&data);
    let saved = api
        .save_export(request(
            ExportPreset::Monthly,
            ExportFormat::Xlsx,
            ExportRedaction::default(),
        ))
        .unwrap();
    assert!(saved.saved);
    let (name, bytes) = files.lock().unwrap().pop().unwrap();
    assert!(
        std::path::Path::new(&name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
    );
    assert!(bytes.starts_with(b"PK"), "XLSX files are ZIP archives");
    assert_eq!(
        data.store.audit_entries(1).unwrap()[0].detail,
        "monthly report, XLSX, 1 row"
    );
}
