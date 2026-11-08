//! The command catalogue: the only place where commands are declared.
//!
//! Each entry reads `command_name(RequestDto) -> ResponseDto;` and maps 1:1 to the
//! [`crate::Api`] method of the same name. Consumers expand the catalogue through a
//! callback macro, so the Tauri handlers, the Tauri permission allow-list and the
//! generated TypeScript command map can never drift apart.

/// Expands `$callback!` with every command in the catalogue.
///
/// The callback macro receives a list of `name(Request) -> Response;` entries.
#[macro_export]
macro_rules! for_each_command {
    ($callback:ident) => {
        $callback! {
            get_app_info($crate::dto::AppInfoRequest) -> $crate::dto::AppInfo;
        }
    };
}

macro_rules! command_names {
    ($($name:ident($request:ty) -> $response:ty;)*) => {
        /// Names of every command, in catalogue order.
        pub const COMMAND_NAMES: &[&str] = &[$(stringify!($name)),*];
    };
}

for_each_command!(command_names);

#[cfg(test)]
mod tests {
    use super::COMMAND_NAMES;

    #[test]
    fn command_names_are_unique_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for name in COMMAND_NAMES {
            assert!(seen.insert(*name), "duplicate command `{name}`");
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "command `{name}` must be snake_case"
            );
        }
    }
}
