//! Names of the events the core emits to the dashboard (see `for_each_event!`).

/// Stored usage changed after an ingest scan. Payload: [`crate::dto::DataChanged`].
pub const DATA_CHANGED: &str = "data-changed";
