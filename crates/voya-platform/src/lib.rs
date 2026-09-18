//! Platform integration boundary.
//!
//! OS-specific paths, process control, system proxy, TUN, autostart, and
//! elevation adapters are isolated here.

pub mod apps;
pub mod autostart;
pub mod clipboard;
pub mod coreinfo;
pub mod elevation;
pub mod filesystem;
pub mod firewall;
pub mod locale;
pub mod netif;
pub mod paths;
pub mod privilege;
pub mod process;
pub mod screen_capture;
pub mod sysproxy;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tun;
#[cfg(target_os = "macos")]
pub mod window_chrome;
