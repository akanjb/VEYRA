//! The application facade: one method per use case, named after its command.

use crate::dto::{AppInfo, AppInfoRequest, BuildProfile};
use crate::error::ApiError;

/// Static facts about the build, supplied by the composition root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMetadata {
    pub product_name: String,
    pub version: String,
    pub build_profile: BuildProfile,
}

/// Entry point for every use case exposed to the presentation layer.
///
/// Method names must match the command catalogue in `commands.rs`.
#[derive(Debug)]
pub struct Api {
    metadata: AppMetadata,
}

impl Api {
    #[must_use]
    pub fn new(metadata: AppMetadata) -> Self {
        Self { metadata }
    }

    /// Returns product name, version, build profile and platform.
    ///
    /// # Errors
    /// Never fails today; the `Result` keeps the signature uniform across commands.
    pub fn get_app_info(&self, _request: AppInfoRequest) -> Result<AppInfo, ApiError> {
        Ok(AppInfo {
            product_name: self.metadata.product_name.clone(),
            version: self.metadata.version.clone(),
            build_profile: self.metadata.build_profile,
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Api, AppMetadata};
    use crate::dto::{AppInfoRequest, BuildProfile};

    fn api() -> Api {
        Api::new(AppMetadata {
            product_name: "Veyra".to_owned(),
            version: "1.2.3".to_owned(),
            build_profile: BuildProfile::Release,
        })
    }

    #[test]
    fn app_info_reports_metadata_and_platform() {
        let info = api().get_app_info(AppInfoRequest {});
        assert_eq!(
            info.map(|i| (i.product_name, i.version, i.build_profile, i.os, i.arch)),
            Ok((
                "Veyra".to_owned(),
                "1.2.3".to_owned(),
                BuildProfile::Release,
                std::env::consts::OS.to_owned(),
                std::env::consts::ARCH.to_owned(),
            ))
        );
    }
}
