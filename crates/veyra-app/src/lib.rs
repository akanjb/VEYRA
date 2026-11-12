//! Veyra application layer.
//!
//! Owns the use cases exposed to the presentation layer, the DTOs that form the
//! Rust ↔ TypeScript contract, and the command catalogue that both the Tauri shell
//! and the contracts generator derive from (single source of truth).

mod api;
mod commands;
pub mod dto;
mod error;
pub mod events;
mod export;
mod names;
mod scope;
mod shell;
mod views;

pub use api::{AnalyticsReader, Api, AppMetadata, DataServices};
pub use commands::COMMAND_NAMES;
pub use error::ApiError;
pub use export::FileSaver;
pub use names::DisplayNames;
pub use shell::AppControl;
