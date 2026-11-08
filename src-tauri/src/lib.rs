//! Veyra desktop shell and composition root.
//!
//! Wires infrastructure into the application layer and exposes the command catalogue
//! over Tauri IPC. Contains no business logic.

mod commands;
mod data;

use tauri::{Context, PackageInfo, Runtime};
use veyra_app::dto::BuildProfile;
use veyra_app::{Api, AppMetadata};

/// Registers Veyra's application state and command handlers on a Tauri builder.
///
/// Generic over the runtime so tests can use Tauri's mock runtime. Data services are
/// started by [`run`], so tests never touch the user's database.
pub fn configure<R: Runtime>(
    builder: tauri::Builder<R>,
    context: &Context<R>,
) -> tauri::Builder<R> {
    builder
        .manage(Api::new(app_metadata(context.package_info())))
        .invoke_handler(commands::handler())
}

/// Builds and runs the desktop application until it exits.
///
/// # Errors
/// Returns an error if the Tauri runtime fails to start.
pub fn run() -> Result<(), tauri::Error> {
    let context = tauri::generate_context!();
    configure(tauri::Builder::default(), &context)
        .setup(|app| {
            data::start(app.handle());
            Ok(())
        })
        .run(context)
}

fn app_metadata(info: &PackageInfo) -> AppMetadata {
    AppMetadata {
        product_name: info.name.clone(),
        version: info.version.to_string(),
        build_profile: if cfg!(debug_assertions) {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        },
    }
}
