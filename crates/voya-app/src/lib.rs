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
pub mod post_commit;
pub mod profiles;
pub mod proxy_runtime;
pub mod qr;
pub mod redaction;
pub mod routing;
pub mod runtime;
pub mod self_host;
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
