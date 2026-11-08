//! Veyra analytics.
//!
//! The data layer returns only **additive base sums** (ADR-008); this crate derives every
//! ratio, average, saving, share and comparison from them with one metric registry, so
//! each formula exists exactly once and aggregates are never averages of averages.

pub mod activity;
pub mod composition;
pub mod dimension;
pub mod engine;
pub mod metrics;
pub mod port;
pub mod query;
pub mod sessions;
pub mod sums;
pub mod top_n;
