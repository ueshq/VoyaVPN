//! Application orchestration layer.
//!
//! Managers that combine domain, persistence, network, and platform adapters
//! live here. Tauri command wiring stays in `src-tauri`.

pub mod autostart;
mod backoff;
pub mod config_mutation;
pub mod connection_ip;
pub mod connection_mode;
pub mod contract_map;
pub mod core_flow;
mod coregen;
pub mod dns;
pub mod elevation;
pub mod exports;
pub mod input_safety;
pub mod invalidation;
pub mod language;
pub mod lifecycle;
pub mod log_batch;
pub mod logging;
pub mod policy_groups;
pub mod profiles;
pub mod proxy_runtime;
pub mod qr;
pub mod redaction;
pub mod routing;
pub mod runtime;
pub mod services;
pub mod settings;
pub mod speedtest;
pub mod startup;
pub mod statistics;
pub mod subscriptions;
pub mod supervisor;
pub mod sysproxy;
pub mod tray;
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
