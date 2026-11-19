//! Stored settings: JSON values under stable keys.

use crate::source::PortError;

/// Where settings are kept.
pub trait SettingsRepository: Send + Sync {
    /// The stored JSON value of `key`, if any.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn setting(&self, key: &str) -> Result<Option<String>, PortError>;

    /// The stored JSON value of `key`, storing `initial` first when there is none. Two
    /// callers racing to set it both get the value stored first.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn setting_or_init(&self, key: &str, initial: &str) -> Result<String, PortError>;

    /// Stores `value` (JSON) under `key`, replacing any earlier value.
    ///
    /// # Errors
    /// Returns [`PortError`] when persistence fails.
    fn set_setting(&self, key: &str, value: &str) -> Result<(), PortError>;
}
