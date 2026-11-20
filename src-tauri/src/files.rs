//! Saves reports where the user chooses and opens the data exports the user picks,
//! through the operating system's dialogs (ADR-032, ADR-040). The dashboard never names
//! a path: only the dialog's choice is written to or read.

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use veyra_app::{FilePicker, FileSaver};
use veyra_domain::import::ReadSeek;

/// The label of the dashboard window, which the dialog belongs to.
const MAIN_WINDOW: &str = "main";

/// A [`FileSaver`] and [`FilePicker`] backed by the native dialogs.
pub struct Dialogs<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> FileSaver for Dialogs<R> {
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

impl<R: Runtime> FilePicker for Dialogs<R> {
    fn pick(
        &self,
        filter_name: &str,
        extensions: &[&str],
    ) -> Result<Option<Box<dyn ReadSeek>>, String> {
        let mut dialog = self.app.dialog().file().add_filter(filter_name, extensions);
        if let Some(window) = self.app.get_webview_window(MAIN_WINDOW) {
            dialog = dialog.set_parent(&window);
        }
        // Runs on a command thread, never the UI thread (see `commands.rs`).
        let Some(chosen) = dialog.blocking_pick_file() else {
            return Ok(None);
        };
        let path = chosen
            .into_path()
            .map_err(|e| format!("the chosen file is not a file path: {e}"))?;
        // Opened read-only; the error leaves out the path, which can name the user.
        let file = std::fs::File::open(&path).map_err(|e| format!("cannot open the file: {e}"))?;
        Ok(Some(Box::new(file)))
    }
}
