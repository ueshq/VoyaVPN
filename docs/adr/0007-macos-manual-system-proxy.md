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
  or executes system-proxy scripts. Stored custom script paths remain inert.
  Windows/Linux retain automatic management.
- Home uses a TUN mode switch: on selects TUN, off selects system proxy. A
  single traffic-mode control offers smart routing (rule), global and direct.
  All three are core routing modes; selecting one preserves the existing
  system-proxy/PAC preference and TUN flag. Offline changes save the preference
  without connecting. Online changes save it, apply the core mode, then close
  existing core connections so applications reconnect under the new mode.
  Each live step has a timeout, reports its own failure through AppError, and
  keeps the saved preference available for an explicit retry. Startup and
  config reload retain their separate mode-application behavior.
  Manual proxy addresses, observation, recheck and system-settings actions
  live in Settings → Network; Home retains the factual connection status.
  Persisted fields remain; old
  non-TUN `forcedClear`/`unchanged` preferences become `forcedChange` at startup,
  and fresh installs default to system proxy. Automatic OS proxy setup still
  waits for a connected core. The selected mode is
  separate from observed system settings. A running local core is described as
  locally ready; only a connected VPN is described as protecting macOS traffic.
- The platform adapter reads HTTP, HTTPS, SOCKS and PAC for every network service
  via SystemConfiguration. Read failures and WPAD are unknown, never clear.
  Any loopback proxy is conservatively considered to need manual cleanup; the
  application does not claim ownership of or modify third-party settings.
- Legacy `proxy-dirty` files cause a manual reminder. Startup/stop never clear
  them. A user-requested recheck (including the check before quitting) clears
  the marker only with no observed loopback proxy and a complete observation.
  Unknown observations cannot clear it. Exit reminders distinguish detected
  local proxies from an unverified state and offer Open Network settings as
  the default action, Quit anyway, and Cancel. Opening settings or cancelling
  keeps the connection running; the next exit request checks settings again.
- The app continues hosting local PAC. A URL is exposed only after its listener
  starts and remains stable until stopped. Disconnect/VPN/exit stop that local
  listener and remind the user to remove manually configured proxies.
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
manual lifecycle behavior, PAC failure and observation/marker handling. Renderer
and mock smoke tests check actionable retry, unknown observations and manual
configuration help. Signed channel and real traffic checks remain a separate
release gate: see [macOS acceptance](../release/macos-vpn-manual-proxy-acceptance.md).
