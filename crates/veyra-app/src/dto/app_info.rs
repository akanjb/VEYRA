use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request for [`crate::Api::get_app_info`]. Intentionally empty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AppInfoRequest {}

/// Basic facts about the running application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub product_name: String,
    pub version: String,
    pub build_profile: BuildProfile,
    /// Operating system family, e.g. `windows`, `macos`, `linux`.
    pub os: String,
    /// CPU architecture, e.g. `x86_64`, `aarch64`.
    pub arch: String,
}

/// Whether the binary was compiled with optimisations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BuildProfile {
    Debug,
    Release,
}
