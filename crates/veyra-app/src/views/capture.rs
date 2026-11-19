//! The local capture proxy's switch and status (ADR-039).

use crate::DataServices;
use crate::dto::{CaptureProvider, CaptureProxyInfo};
use crate::error::ApiError;
use crate::views::internal;

/// Settings key of the user's choice; the value is `true` or `false`.
const ENABLED_KEY: &str = "capture_proxy_enabled";

/// Whether the user turned the proxy on. Off when never chosen.
fn enabled(data: &DataServices) -> Result<bool, ApiError> {
    Ok(data
        .settings
        .setting(ENABLED_KEY)
        .map_err(|e| internal(&e))?
        .is_some_and(|value| value == "true"))
}

/// The user's choice, what the proxy is doing and the base URL of each provider.
pub fn info(data: &DataServices) -> Result<CaptureProxyInfo, ApiError> {
    let status = data.proxy.status();
    let providers = data
        .capture_routes
        .iter()
        .map(|route| CaptureProvider {
            name: route.descriptor().display_name.to_owned(),
            base_url: format!(
                "{}/{}{}",
                status.endpoint,
                route.prefix(),
                route.client_suffix()
            ),
            upstream: format!("{}{}", route.upstream(), route.client_suffix()),
        })
        .collect();
    Ok(CaptureProxyInfo {
        enabled: enabled(data)?,
        listening: status.listening,
        endpoint: status.endpoint,
        error: status.error,
        relayed: status.relayed,
        records: status.records,
        last_relayed_at: status.last_relayed_at.map(|at| at.0),
        last_failure: status.failure,
        providers,
    })
}

/// Keeps the user's choice and applies it at once.
pub fn set(data: &DataServices, on: bool) -> Result<CaptureProxyInfo, ApiError> {
    data.settings
        .set_setting(ENABLED_KEY, if on { "true" } else { "false" })
        .map_err(|e| internal(&e))?;
    data.proxy.set_enabled(on);
    info(data)
}

/// Starts the proxy at launch when the user had turned it on.
pub fn restore(data: &DataServices) {
    match enabled(data) {
        Ok(on) => data.proxy.set_enabled(on),
        Err(error) => eprintln!("veyra: cannot read the capture proxy setting: {error}"),
    }
}
