//! Reports written to files (ADR-032): a format-independent table, and the writers that
//! turn it into CSV, JSON or XLSX. The rows come from the analytics engine (`views::export`), so
//! a report adds up to what the pages show for the same scope.

mod csv;
mod json;
mod xlsx;

use veyra_domain::money::NanoUsd;

use crate::dto::{ExportFormat, ExportRedaction};

/// One column: a stable key (JSON) and a heading (CSV and the preview).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Column {
    pub key: &'static str,
    pub label: &'static str,
}

/// One value of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Cell {
    Text(String),
    Count(u64),
    /// US dollars, kept in billionths so the file is exact.
    Money(NanoUsd),
    /// No value: the measure does not apply to the row, or the source did not report it.
    Empty,
}

impl Cell {
    /// The value as it appears in CSV and in the preview.
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Count(n) => n.to_string(),
            Self::Money(amount) => dollars(*amount),
            Self::Empty => String::new(),
        }
    }
}

/// A report: columns and rows of cells aligned with them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
}

/// What a report covers, written at the top of a JSON file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReportInfo {
    /// The preset's name, as in the file name (for example `per-project`).
    pub report: &'static str,
    pub start: String,
    pub end: String,
    pub time_zone: String,
    pub redaction: ExportRedaction,
}

/// The bytes of `table` in `format`.
///
/// # Errors
/// A description of the failure when the workbook cannot be built (XLSX only).
pub(crate) fn encode(
    table: &Table,
    info: &ReportInfo,
    format: ExportFormat,
) -> Result<Vec<u8>, String> {
    match format {
        ExportFormat::Csv => Ok(csv::write(table, false)),
        ExportFormat::CsvExcel => Ok(csv::write(table, true)),
        ExportFormat::Json => Ok(json::write(table, info)),
        ExportFormat::Xlsx => xlsx::write(table),
    }
}

/// The file extension of `format`.
pub(crate) const fn extension(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv | ExportFormat::CsvExcel => "csv",
        ExportFormat::Json => "json",
        ExportFormat::Xlsx => "xlsx",
    }
}

/// The file name the save dialog suggests: `veyra-<report>-<start>-to-<end>.<ext>`.
pub(crate) fn file_name(info: &ReportInfo, format: ExportFormat) -> String {
    format!(
        "veyra-{}-{}-to-{}.{}",
        info.report,
        info.start,
        info.end,
        extension(format)
    )
}

/// The audit log's description of an export: the report, format, size and redaction,
/// never the file's name or location.
pub(crate) fn audit_detail(info: &ReportInfo, format: ExportFormat, rows: u32) -> String {
    let format = match format {
        ExportFormat::Csv => "CSV",
        ExportFormat::CsvExcel => "CSV for Excel",
        ExportFormat::Json => "JSON",
        ExportFormat::Xlsx => "XLSX",
    };
    let noun = if rows == 1 { "row" } else { "rows" };
    let mut detail = format!("{} report, {format}, {rows} {noun}", info.report);
    if info.redaction.hash_project_names {
        detail.push_str(", project names hashed");
    }
    if info.redaction.drop_paths {
        detail.push_str(", paths left out");
    }
    detail
}

/// `-12.345678900`: whole dollars and nine decimals, exact.
fn dollars(amount: NanoUsd) -> String {
    let nano = amount.0;
    let sign = if nano < 0 { "-" } else { "" };
    let magnitude = nano.unsigned_abs();
    format!(
        "{sign}{}.{:09}",
        magnitude / 1_000_000_000,
        magnitude % 1_000_000_000
    )
}

/// Asks the user where to save a file and writes it there (implemented by the shell, with
/// the operating system's save dialog, so the dashboard never names a path).
pub trait FileSaver: Send + Sync {
    /// Shows the dialog with `suggested_name` and saves `bytes` to the chosen file.
    /// Returns `false` when the user cancels.
    ///
    /// # Errors
    /// Returns a description of the failure when the file cannot be written.
    fn save(&self, suggested_name: &str, extension: &str, bytes: &[u8]) -> Result<bool, String>;
}

#[cfg(test)]
mod tests {
    use veyra_domain::money::NanoUsd;

    use super::dollars;

    #[test]
    fn dollars_keep_every_billionth() {
        assert_eq!(dollars(NanoUsd(0)), "0.000000000");
        assert_eq!(dollars(NanoUsd(12_345_678_901)), "12.345678901");
        assert_eq!(dollars(NanoUsd(-5)), "-0.000000005");
        assert_eq!(dollars(NanoUsd(i64::MIN)), "-9223372036.854775808");
    }
}
