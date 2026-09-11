//! Versioned data contracts shared by the application and desktop shell.
//!
//! This crate intentionally contains no domain behavior, persistence, network,
//! platform, or Tauri dependencies. Contracts are strict and use one canonical
//! camel-case representation for both serialization and deserialization.

mod certificates;
mod data;
mod events;
mod messages;
mod operations;
mod profiles;
mod proxy;
mod runtime;
mod settings;
mod settings_apply;
mod shell;
mod speedtest;
mod tun;

pub use certificates::*;
pub use data::*;
pub use events::*;
pub use messages::*;
pub use operations::*;
pub use profiles::*;
pub use proxy::*;
pub use runtime::*;
pub use settings::*;
pub use settings_apply::*;
pub use shell::*;
pub use speedtest::*;
pub use tun::*;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
