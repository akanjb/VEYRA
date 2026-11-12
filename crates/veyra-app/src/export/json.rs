//! JSON reports: what the report covers, then one object per row keyed by column key.
//! Amounts are integers in billionths of a US dollar, so they stay exact.

use serde_json::{Map, Value, json};

use super::{Cell, ReportInfo, Table};

/// The table as a JSON document.
pub(super) fn write(table: &Table, info: &ReportInfo) -> Vec<u8> {
    let rows: Vec<Value> = table
        .rows
        .iter()
        .map(|row| {
            let object: Map<String, Value> = table
                .columns
                .iter()
                .zip(row)
                .map(|(column, cell)| (column.key.to_owned(), value(cell)))
                .collect();
            Value::Object(object)
        })
        .collect();
    let document = json!({
        "report": info.report,
        "start": info.start,
        "end": info.end,
        "timeZone": info.time_zone,
        "amounts": "billionths of a US dollar",
        "redaction": {
            "hashProjectNames": info.redaction.hash_project_names,
            "dropPaths": info.redaction.drop_paths,
        },
        "rows": rows,
    });
    let mut bytes = serde_json::to_vec_pretty(&document).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn value(cell: &Cell) -> Value {
    match cell {
        Cell::Text(text) => Value::String(text.clone()),
        Cell::Count(n) => Value::from(*n),
        Cell::Money(amount) => Value::from(amount.0),
        Cell::Empty => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use veyra_domain::money::NanoUsd;

    use super::write;
    use crate::dto::ExportRedaction;
    use crate::export::{Cell, Column, ReportInfo, Table};

    #[test]
    fn rows_are_objects_with_exact_amounts() {
        let table = Table {
            columns: vec![
                Column {
                    key: "model",
                    label: "Model",
                },
                Column {
                    key: "costNanoUsd",
                    label: "Cost (USD)",
                },
                Column {
                    key: "requests",
                    label: "Requests",
                },
            ],
            rows: vec![vec![
                Cell::Text("m".to_owned()),
                Cell::Money(NanoUsd(12_345_678_901)),
                Cell::Empty,
            ]],
        };
        let info = ReportInfo {
            report: "per-model",
            start: "2026-11-01".to_owned(),
            end: "2026-11-30".to_owned(),
            time_zone: "UTC".to_owned(),
            redaction: ExportRedaction::default(),
        };
        let parsed: serde_json::Value =
            serde_json::from_slice(&write(&table, &info)).unwrap_or_default();
        assert_eq!(parsed["report"], "per-model");
        assert_eq!(
            parsed["rows"][0],
            serde_json::json!({ "model": "m", "costNanoUsd": 12_345_678_901_i64, "requests": null })
        );
    }
}
