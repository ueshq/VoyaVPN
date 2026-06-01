//! Platform integration boundary.
//!
//! OS-specific paths, process control, system proxy, TUN, autostart, elevation,
//! and hotkey adapters are isolated here.

pub mod autostart;
pub mod coreinfo;
pub mod elevation;
pub mod hotkeys;
pub mod paths;
pub mod process;
pub mod sysproxy;
pub mod tun;
