//! Platform integration boundary.
//!
//! OS-specific paths, process control, system proxy, TUN, autostart, elevation,
//! and hotkey adapters are isolated here.

pub mod autostart;
pub mod coreinfo;
pub mod elevation;
pub mod hotkeys;
pub mod paths;
pub mod privilege;
pub mod process;
pub mod sysproxy;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tun;
