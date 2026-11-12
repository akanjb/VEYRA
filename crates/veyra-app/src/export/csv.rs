//! RFC 4180 CSV: comma-separated, CRLF line ends, fields quoted when they hold a comma, a
//! quote or a line break, and quotes doubled.

use super::{Cell, Table};

/// UTF-8 byte-order mark, which Excel needs to read the file as UTF-8.
const BOM: &str = "\u{feff}";

/// The table as CSV, with a header row of column labels.
pub(super) fn write(table: &Table, bom: bool) -> Vec<u8> {
    let mut out = String::new();
    if bom {
        out.push_str(BOM);
    }
    let header: Vec<String> = table
        .columns
        .iter()
        .map(|column| field(column.label))
        .collect();
    push_line(&mut out, &header);
    for row in &table.rows {
        let fields: Vec<String> = row.iter().map(cell).collect();
        push_line(&mut out, &fields);
    }
    out.into_bytes()
}

fn push_line(out: &mut String, fields: &[String]) {
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn cell(value: &Cell) -> String {
    match value {
        Cell::Text(text) => field(&neutralised(text)),
        other => field(&other.text()),
    }
}

/// Text a spreadsheet would run as a formula, prefixed with an apostrophe so it stays
/// text. Project names come from folder names, which anyone can choose.
fn neutralised(text: &str) -> String {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    }
}

fn field(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use veyra_domain::money::NanoUsd;

    use super::write;
    use crate::export::{Cell, Column, Table};

    fn table(rows: Vec<Vec<Cell>>) -> Table {
        Table {
            columns: vec![
                Column {
                    key: "name",
                    label: "Name",
                },
                Column {
                    key: "cost",
                    label: "Cost, USD",
                },
            ],
            rows,
        }
    }

    fn text(bytes: &[u8]) -> &str {
        std::str::from_utf8(bytes).unwrap_or_default()
    }

    #[test]
    fn fields_are_quoted_only_when_needed() {
        let bytes = write(
            &table(vec![
                vec![Cell::Text("plain".to_owned()), Cell::Money(NanoUsd(1))],
                vec![Cell::Text("say \"hi\", twice".to_owned()), Cell::Empty],
                vec![Cell::Text("two\nlines".to_owned()), Cell::Count(7)],
            ]),
            false,
        );
        assert_eq!(
            text(&bytes),
            "Name,\"Cost, USD\"\r\nplain,0.000000001\r\n\"say \"\"hi\"\", twice\",\r\n\
             \"two\nlines\",7\r\n"
        );
    }

    #[test]
    fn formulas_in_text_stay_text() {
        let bytes = write(
            &table(vec![vec![
                Cell::Text("=HYPERLINK(\"x\")".to_owned()),
                Cell::Money(NanoUsd(-1)),
            ]]),
            false,
        );
        // Text is neutralised; a negative amount is a number, not a formula.
        assert_eq!(
            text(&bytes),
            "Name,\"Cost, USD\"\r\n\"'=HYPERLINK(\"\"x\"\")\",-0.000000001\r\n"
        );
    }

    #[test]
    fn excel_files_start_with_a_byte_order_mark() {
        let bytes = write(&table(vec![]), true);
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    }
}
