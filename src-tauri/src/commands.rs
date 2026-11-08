//! Tauri command handlers, generated from the application's command catalogue.
//!
//! Each handler only forwards to the `Api` method of the same name.

use tauri::ipc::Invoke;
use tauri::{Runtime, State};
use veyra_app::{Api, ApiError};

macro_rules! tauri_commands {
    ($($name:ident($request:ty) -> $response:ty;)*) => {
        $(
            // `async` runs the handler off the main (UI) thread.
            #[tauri::command(async)]
            fn $name(api: State<'_, Api>, request: $request) -> Result<$response, ApiError> {
                api.$name(request)
            }
        )*

        pub(crate) fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$($name),*]
        }
    };
}

veyra_app::for_each_command!(tauri_commands);
