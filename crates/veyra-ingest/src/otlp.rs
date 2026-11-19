//! Decoding of OTLP/HTTP JSON log exports (`ExportLogsServiceRequest`) into
//! [`TelemetryRecord`]s (ADR-038).
//!
//! Only scalar attributes are kept: arrays, maps and bytes are skipped, and record
//! bodies are never read. Integers may arrive as JSON strings (the protobuf JSON
//! mapping of 64-bit values) or as numbers; both are accepted.

use serde_json::Value;
use veyra_domain::telemetry::TelemetryRecord;
use veyra_domain::time::UtcMillis;

const NANOS_PER_MILLI: u64 = 1_000_000;

/// The log records of an export, each with its resource's service.
///
/// # Errors
/// Returns the JSON error when the body is not JSON. A body that is JSON but not an
/// export decodes to no records.
pub fn decode_logs(body: &[u8]) -> Result<Vec<TelemetryRecord>, serde_json::Error> {
    let export: Value = serde_json::from_slice(body)?;
    let mut records = Vec::new();
    for resource_logs in array(&export, "resourceLogs") {
        let resource = scalars(resource_logs.get("resource").unwrap_or(&Value::Null));
        let service = find(&resource, "service.name").unwrap_or_default();
        let service_version = find(&resource, "service.version");
        for scope_logs in array(resource_logs, "scopeLogs") {
            for log in array(scope_logs, "logRecords") {
                records.push(TelemetryRecord {
                    service: service.clone(),
                    service_version: service_version.clone(),
                    time: log.get("timeUnixNano").and_then(unix_nanos),
                    attributes: scalars(log).into_iter().collect(),
                });
            }
        }
    }
    Ok(records)
}

/// The array under `key`, or nothing.
fn array<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The scalar entries of the `attributes` list of `owner`, as text.
fn scalars(owner: &Value) -> Vec<(String, String)> {
    array(owner, "attributes")
        .filter_map(|attribute| {
            let key = attribute.get("key")?.as_str()?;
            let value = scalar(attribute.get("value")?)?;
            Some((key.to_owned(), value))
        })
        .collect()
}

fn find(attributes: &[(String, String)], key: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

/// An `AnyValue` holding a string, integer, double or boolean, as text.
fn scalar(value: &Value) -> Option<String> {
    if let Some(text) = value.get("stringValue").and_then(Value::as_str) {
        return Some(text.to_owned());
    }
    if let Some(int) = value.get("intValue") {
        return match int {
            Value::String(text) => text.parse::<i64>().ok().map(|n| n.to_string()),
            Value::Number(number) => number.as_i64().map(|n| n.to_string()),
            _ => None,
        };
    }
    if let Some(double) = value.get("doubleValue").and_then(Value::as_f64) {
        return Some(double.to_string());
    }
    value
        .get("boolValue")
        .and_then(Value::as_bool)
        .map(|b| b.to_string())
}

/// A `fixed64` nanosecond time, given as a JSON string or number; zero means unset.
fn unix_nanos(value: &Value) -> Option<UtcMillis> {
    let nanos = match value {
        Value::String(text) => text.parse::<u64>().ok()?,
        Value::Number(number) => number.as_u64()?,
        _ => return None,
    };
    let millis = i64::try_from(nanos / NANOS_PER_MILLI).ok()?;
    (millis > 0).then_some(UtcMillis(millis))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

    use super::decode_logs;
    use veyra_domain::time::UtcMillis;

    #[test]
    fn decodes_records_with_their_service_and_scalar_attributes() {
        let body = br#"{"resourceLogs":[{
            "resource":{"attributes":[
                {"key":"service.name","value":{"stringValue":"claude-code"}},
                {"key":"service.version","value":{"stringValue":"2.1.400"}}]},
            "scopeLogs":[{"logRecords":[{
                "timeUnixNano":"1762600000000000000",
                "body":{"stringValue":"claude_code.api_request"},
                "attributes":[
                    {"key":"input_tokens","value":{"intValue":"12"}},
                    {"key":"output_tokens","value":{"intValue":7}},
                    {"key":"cost_usd","value":{"doubleValue":0.5}},
                    {"key":"flag","value":{"boolValue":true}},
                    {"key":"list","value":{"arrayValue":{"values":[]}}}]}]}]}]}"#;
        let records = decode_logs(body).unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.service, "claude-code");
        assert_eq!(record.service_version.as_deref(), Some("2.1.400"));
        assert_eq!(record.time, Some(UtcMillis(1_762_600_000_000)));
        assert_eq!(record.count("input_tokens"), Some(12));
        assert_eq!(record.count("output_tokens"), Some(7));
        assert_eq!(record.text("cost_usd"), Some("0.5"));
        assert_eq!(record.text("flag"), Some("true"));
        assert_eq!(record.text("list"), None);
    }

    #[test]
    fn json_that_is_not_an_export_has_no_records() {
        assert_eq!(decode_logs(b"{}").unwrap(), []);
        assert_eq!(decode_logs(b"[1,2]").unwrap(), []);
        assert!(decode_logs(b"not json").is_err());
    }
}
