//! Storing usage pushed to Veyra rather than read from files: the OpenTelemetry
//! receiver (ADR-038) and the capture proxy (ADR-039). Pushed usage takes the same path
//! as usage read from files, so the two cannot drift apart.

use veyra_domain::pricing::price_event;
use veyra_domain::source::{ConnectorDescriptor, RawUsage};
use veyra_domain::time::UtcMillis;
use veyra_registry::Registry;
use veyra_store::{PricedEvent, Store, StoreError};

use crate::ScanReport;
use crate::http::{Request, Status};
use crate::normalize::normalize;

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Normalises, prices and writes `usage`, each with the application it belongs to.
/// Returns how many records were written and what changed; records that cannot be
/// normalised are skipped.
pub fn store_usage<'a>(
    store: &Store,
    registry: &Registry,
    usage: impl IntoIterator<Item = (RawUsage, &'a ConnectorDescriptor)>,
) -> Result<(u64, ScanReport), StoreError> {
    let events: Vec<PricedEvent> = usage
        .into_iter()
        .filter_map(|(raw, descriptor)| {
            let event = normalize(&raw, descriptor, registry).ok()?;
            let cost = price_event(&event, registry.catalog());
            Some(PricedEvent {
                event,
                cost,
                precedence: raw.precedence,
                tool_calls: raw.tool_calls,
            })
        })
        .collect();
    if events.is_empty() {
        return Ok((0, ScanReport::default()));
    }
    let summary = store.write_events(&events, UtcMillis::now())?;
    let report = ScanReport {
        inserted: count(summary.inserted),
        updated: count(summary.updated),
        unchanged: count(summary.unchanged),
        superseded: count(summary.superseded),
        ..ScanReport::default()
    };
    Ok((count(events.len()), report))
}

/// Refuses requests from web pages (they carry an `Origin` header) and requests naming
/// a host other than this computer on `port`, which blocks DNS rebinding.
pub fn local_only(request: &Request, port: u16) -> Result<(), Status> {
    if request.header("origin").is_some() {
        return Err(Status::new(403, "requests from web pages are refused"));
    }
    let host = request.header("host").unwrap_or_default();
    let local = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if local.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        Ok(())
    } else {
        Err(Status::new(403, "the host must be 127.0.0.1 or localhost"))
    }
}
