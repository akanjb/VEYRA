//! Data transfer objects: the request and response types of every command.
//!
//! These types are the contract with the dashboard. TypeScript definitions are
//! generated from them (`pnpm contracts:generate`); never edit the generated files.

mod app_info;

pub use app_info::{AppInfo, AppInfoRequest, BuildProfile};
