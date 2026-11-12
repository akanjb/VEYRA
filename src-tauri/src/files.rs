//! Saves reports where the user chooses, through the operating system's save dialog
//! (ADR-032). The dashboard never names a path: only the dialog's choice is written to.

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use veyra_app::FileSaver;

/// The label of the dashboard window, which the dialog belongs to.
const MAIN_WINDOW: &str = "main";

/// A [`FileSaver`] backed by the native save dialog.
pub struct DialogSaver<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> FileSaver for DialogSaver<R> {
    fn save(&self, suggested_name: &str, extension: &str, bytes: &[u8]) -> Result<bool, String> {
        let mut dialog = self
            .app
            .dialog()
            .file()
            .set_file_name(suggested_name)
            .add_filter(extension.to_uppercase(), &[extension]);
        if let Some(window) = self.app.get_webview_window(MAIN_WINDOW) {
            dialog = dialog.set_parent(&window);
        }
        // Runs on a command thread, never the UI thread (see `commands.rs`).
        let Some(chosen) = dialog.blocking_save_file() else {
            return Ok(false);
        };
        let path = chosen
            .into_path()
            .map_err(|e| format!("the chosen location is not a file path: {e}"))?;
        // The error leaves out the path, which can name the user.
        std::fs::write(&path, bytes).map_err(|e| format!("cannot write the report: {e}"))?;
        Ok(true)
    }
}
