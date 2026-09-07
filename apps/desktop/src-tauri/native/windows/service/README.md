# Windows Tunnel Service

VoyaVPN's Windows transparent tunnel is service-backed.

Service contract:

- Service name: `VoyaVPNTunnelService`
- Display name: `VoyaVPN Tunnel Service`
- Start command from the desktop app:
  `sc.exe start VoyaVPNTunnelService <main-config-path>`
- Stop command from the desktop app:
  `sc.exe stop VoyaVPNTunnelService`

The service owns:

- launching and stopping the managed `sing-box.exe` staged beside it in
  `%ProgramFiles%\VoyaVPN\sing_box`
- Wintun driver/device lifecycle
- route and DNS lifecycle
- stale tunnel cleanup before restart
- service-level logs suitable for diagnostics

The desktop app owns:

- profile selection
- generated sing-box JSON
- writing runtime config into the normal VoyaVPN app-data directory
  (`%APPDATA%\app.voyavpn.desktop\binConfigs`), which the service accepts as a
  config source and copies into `%ProgramData%\VoyaVPN\runtime`
- asking the service to start or stop
- surfacing service installation/running/error state through IPC

The desktop app does **not** decide which sing-box the service runs. See
[runtime-protocol.md](runtime-protocol.md) for the trust boundary, the fixed
config roots, and the service DACL the installer applies.

The initial platform controller already targets this service name and command
shape. The installer must add the service before Windows TUN is considered
release-ready.

Build and install helpers:

```sh
pnpm native:windows:tunnel:build
pnpm native:windows:tunnel:install
pnpm native:windows:tunnel:status
pnpm native:windows:tunnel:uninstall
```

`install`, `status`, and `uninstall` must be run from an elevated Windows
terminal. Installation is idempotent: it stops an existing service, copies and
hash-verifies the new binary to
`%ProgramFiles%\VoyaVPN\voyavpn-tunnel-service.exe` and the pinned sing-box
seed to `%ProgramFiles%\VoyaVPN\sing_box\sing-box.exe`, creates
`%ProgramData%\VoyaVPN\runtime` with inheritance removed (SYSTEM and
Administrators only), configures demand start, applies the service DACL that
lets interactive users start and stop it, and leaves the service stopped.
`uninstall` removes the service and exactly those two managed executables
without recursively deleting the containing directories.

Install fails fast when the sing-box seed is missing; run
`pnpm core:sing-box:install` first.

For the complete unsigned local client, installer, and TUN-service flow, use:

```powershell
pnpm build:windows:local
```

The service binary also supports foreground smoke checks:

```sh
voyavpn-tunnel-service.exe run --config %APPDATA%\app.voyavpn.desktop\binConfigs\config.json
```

The foreground mode applies the same containment rules as the service mode, so
the config must already live in an accepted root and the managed core must be
installed.

Smoke requirements:

- `codex` and `claude` work from PowerShell/CMD without proxy environment
  variables while TUN is enabled.
- Browser traffic and terminal traffic follow the same VoyaVPN routing rules.
- Disconnect removes Wintun routes and restores DNS state.
- Service upgrade preserves a clean stop/start lifecycle.
