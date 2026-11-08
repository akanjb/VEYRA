//! Veyra domain model.
//!
//! Pure types and rules with no I/O: identities, usage events, token semantics,
//! accuracy classes, money and the pricing engine. Every other crate builds on these
//! definitions, so each business rule has exactly one home.

pub mod accuracy;
pub mod event;
pub mod identity;
pub mod money;
pub mod pricing;
pub mod time;
pub mod tokens;
