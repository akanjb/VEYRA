//! What the application asks of the desktop shell besides data.

/// Controls the running application (implemented by the shell).
pub trait AppControl: Send + Sync {
    /// Restarts Veyra, for example to apply a restored backup before anything opens the
    /// database.
    fn restart(&self);
}
