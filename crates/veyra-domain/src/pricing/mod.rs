//! Pricing: effective-dated price rules, the catalogue that resolves them, and the
//! engine that turns token counts into an API-equivalent cost (ADR-007).
//!
//! This is the only place in Veyra where cost formulas live.

mod catalog;
mod engine;
mod rule;

pub use catalog::{CatalogError, PriceCatalog};
pub use engine::{Assumption, Assumptions, CostBreakdown, CostStatus, EventCost, price_event};
pub use rule::{
    EffectiveRange, ModelMatch, Modifiers, PriceRule, PromptTier, RateCard, TierFactor,
};
