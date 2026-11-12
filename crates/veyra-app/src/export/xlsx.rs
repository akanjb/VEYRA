//! XLSX reports: one worksheet with a bold, frozen header row. Counts and amounts are
//! numbers, amounts in dollars with nine decimals shown. Text is stored as text, which a
//! spreadsheet never runs as a formula.

use rust_xlsxwriter::{ColNum, Format, RowNum, Workbook, XlsxError};

use super::{Cell, Table};

/// The table as an XLSX workbook.
pub(super) fn write(table: &Table) -> Result<Vec<u8>, String> {
    workbook(table).map_err(|e| format!("cannot build the XLSX report: {e}"))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "spreadsheet numbers are doubles; counts stay exact up to 2^53"
)]
fn workbook(table: &Table) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Report")?;
    let bold = Format::new().set_bold();
    let dollars = Format::new().set_num_format("0.000000000");
    for (col, column) in table.columns.iter().enumerate() {
        sheet.write_string_with_format(0, index(col)?, column.label, &bold)?;
    }
    for (row, cells) in table.rows.iter().enumerate() {
        let row = index::<RowNum>(row + 1)?;
        for (col, cell) in cells.iter().enumerate() {
            let col = index::<ColNum>(col)?;
            match cell {
                Cell::Text(text) => {
                    sheet.write_string(row, col, text.as_str())?;
                }
                Cell::Count(n) => {
                    sheet.write_number(row, col, *n as f64)?;
                }
                Cell::Money(amount) => {
                    let value = amount.0 as f64 / 1e9;
                    sheet.write_number_with_format(row, col, value, &dollars)?;
                }
                Cell::Empty => {}
            }
        }
    }
    sheet.set_freeze_panes(1, 0)?;
    sheet.autofit();
    workbook.save_to_buffer()
}

/// A row or column position, refused when the sheet cannot hold it.
fn index<T: TryFrom<usize>>(position: usize) -> Result<T, XlsxError> {
    T::try_from(position).map_err(|_| XlsxError::RowColumnLimitError)
}

#[cfg(test)]
mod tests {
    use veyra_domain::money::NanoUsd;

    use super::write;
    use crate::export::{Cell, Column, Table};

    #[test]
    fn a_report_is_a_zip_workbook() {
        let table = Table {
            columns: vec![Column {
                key: "cost",
                label: "Cost (USD)",
            }],
            rows: vec![vec![Cell::Money(NanoUsd(1_500_000_000))], vec![Cell::Empty]],
        };
        let bytes = write(&table).unwrap_or_default();
        assert!(bytes.starts_with(b"PK"), "XLSX files are ZIP archives");
    }
}
