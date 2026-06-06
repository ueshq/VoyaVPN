//! Application orchestration layer.
//!
//! Managers that combine domain, persistence, network, and platform adapters
//! live here. Tauri command wiring stays in `src-tauri`.

pub mod autostart;
pub mod backup;
pub mod clash;
pub mod diagnostics;
pub mod dns;
pub mod groups;
pub mod hotkeys;
pub mod presets;
pub mod profiles;
pub mod qr;
pub mod routing;
pub mod runtime;
pub mod speedtest;
pub mod statistics;
pub mod subscriptions;
pub mod sudo;
pub mod supervisor;
pub mod sysproxy;
pub mod tun;
pub mod updates;

/// Static application metadata exposed to the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMetadata {
    pub name: &'static str,
    pub version: &'static str,
}

/// Return compile-time metadata for the current package.
#[must_use]
pub fn metadata() -> AppMetadata {
    AppMetadata {
        name: "VoyaVPN",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_names_the_product() {
        assert_eq!(metadata().name, "VoyaVPN");
    }
}
