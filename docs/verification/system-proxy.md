# System Proxy Verification

Batch: `04-04-system-proxy-tray`

## Covered Locally

- `voya-platform::sysproxy` plans all four v2rayN proxy modes:
  `ForcedClear`, `ForcedChange`, `Unchanged`, and `Pac`.
- Windows advanced proxy templates replace both `{http_port}` and
  `{socks_port}` with the single local SOCKS port and prepend `<local>;` when
  `NotProxyLocalAddress` is enabled.
- Linux and macOS adapters use generated shell scripts unless a valid custom
  script path is configured.
- PAC startup is gated to Windows. Non-Windows PAC requests are planned as
  unsupported and the UI command rejects them.
- Switching away from Windows PAC stops the PAC manager before applying the
  next mode.
- `voya-app::sysproxy` tests disconnect/exit restore semantics with fakes:
  restore forces an effective clear while preserving the requested persisted
  mode.

## Runtime Behavior

- `connect_active_profile` applies the persisted system proxy mode after the
  supervisor starts.
- `disconnect_core` forces a proxy clear unless the requested mode is
  `Unchanged`.
- app exit and tray Quit also force restore and stop PAC.
- `system_proxy_status` and `set_system_proxy_mode` expose status and mode
  changes over generated IPC and emit `sysProxyChanged`.
- The tray menu is rebuilt dynamically with recent servers and a checked system
  proxy submenu. PAC is only shown in the tray and status bar on Windows.

## External Checks

Real OS set/readback smoke checks are not run in this batch because they mutate
host-level proxy settings and need dedicated Windows, Linux, and macOS smoke
machines. Follow-up manual checks:

- Windows: set forced mode, verify WinINet proxy and browser traffic, switch to
  PAC, then disconnect and confirm proxy settings are cleared.
- Linux: set forced mode under GNOME/KDE, verify `gsettings` or `kioslaverc`,
  then disconnect and confirm mode is `none`.
- macOS: set forced mode, verify `networksetup` web/secure/SOCKS proxy state,
  then disconnect and confirm those proxy states are off.
