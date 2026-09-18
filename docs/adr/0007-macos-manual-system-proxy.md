# ADR 0007: macOS Manual System Proxy and Tunnel Ownership

Status: Accepted

Date: 2026-09-09

## Context

A PacketTunnel start request can be accepted while its initial status is still
Disconnected. Treating that sample as a failure loses track of a tunnel that
subsequently starts. Separately, sandboxed macOS applications cannot depend on
executing generated scripts to change system network settings. Quarantine can
reject those scripts before execution as well.

## Decision

All macOS channels, including development, keep App Sandbox. Extension packaging,
provisioning, signatures and sing-box generation remain unchanged. This amends
ADR 0004's automatic system-proxy restoration requirement for macOS only.

- macOS system proxy management is always manual. No lifecycle path generates
  or executes system-proxy scripts. Custom script paths were retired.
  Windows/Linux retain automatic management.
- Home uses a TUN mode switch: on selects TUN, off selects system proxy. A
  single traffic-mode control offers smart routing (rule) and global.
  Both are core routing modes; selecting one preserves the existing
  system-proxy preference and TUN flag. Offline changes save the preference
  without connecting. Online changes save it, apply the core mode, then close
  existing core connections so applications reconnect under the new mode.
  Each live step has a timeout, reports its own failure through AppError, and
  keeps the saved preference available for an explicit retry. Startup and
  config reload retain their separate mode-application behavior.
  Manual proxy addresses, observation, recheck and system-settings actions
  live in Settings → Network; Home retains the factual connection status.
  Current-baseline settings using the removed direct traffic mode are
  normalized to rule before loading, without changing other preferences.
  Invalid settings and historical database baselines remain rejected. Fresh
  installs default to system proxy. Automatic OS proxy setup still
  waits for a connected core. The selected mode is
  separate from observed system settings. A running local core is described as
  locally ready; only a connected VPN is described as protecting macOS traffic.
- The platform adapter reads HTTP, HTTPS, SOCKS and PAC for every network service
  via SystemConfiguration. Read failures and WPAD are unknown, never clear.
  Any loopback proxy is conservatively considered to need manual cleanup; the
  application does not claim ownership of or modify third-party settings.
- macOS ignores historical `proxy-dirty` files and never reads or removes them.
  Cleanup reminders and user-requested rechecks depend only on current system
  observations. Windows/Linux retain their automatic crash-recovery markers.
  Exit reminders distinguish detected
  local proxies from an unverified state and offer Open Network settings as
  the default action, Quit anyway, and Cancel. Opening settings or cancelling
  keeps the connection running; the next exit request checks settings again.
- The app hosts no local PAC listener. The local proxy address is exposed only
  while a connected core serves it. Disconnect/VPN/exit retire that address
  and remind the user to remove manually configured proxies.
- Opening Network settings is a fixed platform action, not a renderer-supplied
  URL. Locale JSON supplies both renderer help and native exit reminders.
- Accepted tunnel starts tolerate initial Disconnected until Connecting or
  Reasserting has been observed; only Connected succeeds. The waiter uses a
  monotonic 20-second deadline and current-session notifications. Failure or
  timeout stops the session and waits up to 10 seconds for cleanup. A stop
  timeout is an error, never success.
- Failed cleanup retains supervisor ownership as `cleanupPending`, without
  publishing a usable core API endpoint. The original start error and cleanup
  error are preserved separately. A subsequent disconnect retries cleanup.
- PlugInKit discovery denied by sandbox is unavailable, not a path mismatch.
  Explicit mismatches still fail. The external NE doctor owns full registration
  checks. Historical provider logs are diagnostic evidence, not connection state.

## Verification

Injected native status/clock tests exercise the production waiter without
changing VPN preferences. Rust tests cover ownership after cleanup failure,
manual lifecycle behavior, endpoint retirement on failure and observation/marker handling. Renderer
and mock smoke tests check actionable retry, unknown observations and manual
configuration help. Signed channel and real traffic checks remain a separate
release gate: see [macOS acceptance](../release/macos-vpn-manual-proxy-acceptance.md).

## Amendment (2026-09-13): PAC Hosting and Custom Scripts Retired

The PAC system-proxy mode, the local PAC listener, the custom PAC file path,
the custom Linux system-proxy script path and the Windows advanced proxy
template are removed on every platform. The system proxy is either forced
change (the local mixed inbound), forced clear, or left unchanged. A stored
`pac` preference is normalized to `forcedChange` when the database opens, and
the retired settings keys are stripped at the same persistence boundary.
Observation still reads PAC URLs that other tools configured, because a
loopback PAC URL is evidence of a local proxy the user may need to remove.

## Amendment (2026-09-13): macOS Has Only the PacketTunnel VPN

The manual system proxy mode is retired on macOS. macOS captures traffic only
through the NetworkExtension PacketTunnel running sing-box; Windows and Linux
keep both capture modes.

- `voya-platform::sysproxy::system_proxy_management` reports `Unsupported` for
  macOS. The SystemConfiguration observation, the Open Network settings action,
  the native `macos_sysproxy.m` bridge and the exit-time proxy reminder are
  removed, together with the `recheck_system_proxy`, `open_network_settings`
  and `set_system_proxy_mode` commands.
- Leaving VPN mode on macOS is refused by `TunManager::plan_set_enabled` with
  `TunManagerError::VpnRequired`, mapped to `AppErrorKind::Unsupported`.
  `ConnectionModeStatus` reports `systemProxyAvailable` and
  `processRulesSupported`, both false for the PacketTunnel backend.
- Loading and every commit keep a macOS configuration in VPN mode with the
  system proxy type `Unchanged`, without migrating stored rows. A fresh install
  starts in VPN mode on Windows and macOS and in system proxy mode on Linux.
- The capture mode is no longer a Home control. Home offers only the traffic
  mode (Rule / Global). Windows and Linux choose VPN mode or the system proxy,
  labelled a compatibility mode, in Settings -> Network.

## Amendment (2026-09-14): Kill Switch and Where Capture Lives

- Settings are grouped as General / Connection / Advanced / Updates. The
  capture mode choice (Windows and Linux only) lives under Advanced, next to
  the TUN options, and shows what the running connection actually does: VPN
  capturing, VPN not active for this connection, system proxy set or not set.
  The window refreshes the TUN and system proxy channels whenever it regains
  focus.
- Connection -> Kill switch ("Block traffic outside the VPN") is the stored
  `network.tun.strictRoute`. Windows and Linux pass it to sing-box as
  `strict_route`; macOS passes it through `SupervisorStartRequest.kill_switch`
  and `NativeTunStartRequest.kill_switch` to the PacketTunnel bridge, which
  sets `includeAllNetworks` from it and always sets `excludeLocalNetworks`.
  The separate Strict route checkbox is gone. Where the system proxy is the
  active capture mode, the switch says it only applies in VPN mode.
- A tray Connect that fails with `AppErrorKind::ElevationRequired` requests
  authorization once and retries, as the window does; any tray failure brings
  the main window forward so its notice is seen.

