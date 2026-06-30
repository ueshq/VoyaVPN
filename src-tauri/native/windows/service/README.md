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

- launching and stopping `sing-box.exe`
- Wintun driver/device lifecycle
- route and DNS lifecycle
- stale tunnel cleanup before restart
- service-level logs suitable for diagnostics

The desktop app owns:

- profile selection
- generated sing-box JSON
- writing runtime config into the normal VoyaVPN app-data directory
- asking the service to start or stop
- surfacing service installation/running/error state through IPC

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
terminal. The service binary also supports foreground smoke checks:

```sh
voyavpn-tunnel-service.exe run --config C:\path\to\VoyaVPN\binConfigs\config.json
```

Smoke requirements:

- `codex` and `claude` work from PowerShell/CMD without proxy environment
  variables while TUN is enabled.
- Browser traffic and terminal traffic follow the same VoyaVPN routing rules.
- Disconnect removes Wintun routes and restores DNS state.
- Service upgrade preserves a clean stop/start lifecycle.
