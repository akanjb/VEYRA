//! The local OpenTelemetry receiver's switch and status (ADR-038).

use crate::DataServices;
use crate::dto::TelemetryReceiverInfo;
use crate::error::ApiError;
use crate::views::internal;

/// Settings key of the user's choice; the value is JSON `true` or `false`.
const ENABLED_KEY: &str = "telemetry_receiver_enabled";

/// Whether the user turned the receiver on. Off when never chosen.
fn enabled(data: &DataServices) -> Result<bool, ApiError> {
    Ok(data
        .settings
        .setting(ENABLED_KEY)
        .map_err(|e| internal(&e))?
        .is_some_and(|value| value == "true"))
}

/// The user's choice and what the receiver is doing.
pub fn info(data: &DataServices) -> Result<TelemetryReceiverInfo, ApiError> {
    let status = data.telemetry.status();
    Ok(TelemetryReceiverInfo {
        enabled: enabled(data)?,
        listening: status.listening,
        endpoint: status.endpoint,
        error: status.error,
        exports: status.exports,
        records: status.records,
        last_received_at: status.last_received_at.map(|at| at.0),
        last_refusal: status.refusal,
    })
}

/// Keeps the user's choice and applies it at once.
pub fn set(data: &DataServices, on: bool) -> Result<TelemetryReceiverInfo, ApiError> {
    data.settings
        .set_setting(ENABLED_KEY, if on { "true" } else { "false" })
        .map_err(|e| internal(&e))?;
    data.telemetry.set_enabled(on);
    info(data)
}

/// Starts the receiver at launch when the user had turned it on.
pub fn restore(data: &DataServices) {
    match enabled(data) {
        Ok(on) => data.telemetry.set_enabled(on),
        Err(error) => eprintln!("veyra: cannot read the receiver setting: {error}"),
    }
}
