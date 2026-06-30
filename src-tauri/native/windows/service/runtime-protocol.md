# VoyaVPN Tunnel Service Runtime Protocol

Version: 1

The desktop app writes the generated sing-box config and invokes:

```text
sc.exe start VoyaVPNTunnelService <main-config-path>
```

The service must validate:

- the config path is absolute
- the config file exists
- the config file is inside the VoyaVPN app-data directory or an approved test
  directory
- the config passes `sing-box check -c <main-config-path>` before launch

The service should run:

```text
sing-box run -c <main-config-path> --disable-color
```

Future protocol versions may add:

- a named-pipe control plane for start/stop/status
- a separate pre-socks config path
- service-emitted structured logs
- sing-box API health checks
