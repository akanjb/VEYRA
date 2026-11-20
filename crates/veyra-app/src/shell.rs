//! What the application asks of the desktop shell besides data.

/// Controls the running application (implemented by the shell).
pub trait AppControl: Send + Sync {
    /// Restarts Veyra, for example to apply a restored backup before anything opens the
    /// database.
    fn restart(&self);
}

/// Asks the user to choose a file to read (implemented by the shell, with the operating
/// system's open dialog, so the dashboard never names a path).
pub trait FilePicker: Send + Sync {
    /// Shows the dialog, offering files with one of `extensions` under `filter_name`,
    /// and opens the chosen file. Returns `None` when the user cancels.
    ///
    /// # Errors
    /// Returns a description of the failure when the chosen file cannot be opened.
    fn pick(
        &self,
        filter_name: &str,
        extensions: &[&str],
    ) -> Result<Option<Box<dyn veyra_domain::import::ReadSeek>>, String>;
}
