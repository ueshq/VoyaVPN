//! Versioned data contracts shared by the application and desktop shell.
//!
//! This crate intentionally contains no domain behavior, persistence, network,
//! platform, or Tauri dependencies. Contracts are strict and use one canonical
//! camel-case representation for both serialization and deserialization.

mod data;
mod events;
mod messages;
mod operations;
mod policy_groups;
mod profiles;
mod proxy;
mod runtime;
mod self_host;
mod settings;
mod settings_apply;
mod shell;
mod speedtest;
mod tun;

pub use data::*;
pub use events::*;
pub use messages::*;
pub use operations::*;
pub use policy_groups::*;
pub use profiles::*;
pub use proxy::*;
pub use runtime::*;
pub use self_host::*;
pub use settings::*;
pub use settings_apply::*;
pub use shell::*;
pub use speedtest::*;
pub use tun::*;
