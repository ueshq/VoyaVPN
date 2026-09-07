# VoyaVPN Tunnel Service Runtime Protocol

Version: 2

The desktop app writes the generated sing-box config and invokes:

```text
sc.exe start VoyaVPNTunnelService <main-config-path>
```

## Trust boundary

The service runs as LocalSystem, so nothing it executes may be chosen by the
caller. The start argument is a *selector* only:

- The sing-box executable is always `<service dir>\sing_box\sing-box.exe`,
  resolved from `std::env::current_exe()`. In a normal install that is
  `%ProgramFiles%\VoyaVPN\sing_box\sing-box.exe`, staged and hash-verified by
  `pnpm native:windows:tunnel:install` under the same elevation that registers
  the service. Its integrity rests on the `%ProgramFiles%` ACL; the service
  never runs the user-writable core in `%APPDATA%\app.voyavpn.desktop\bin`.
- The accepted config roots are fixed inside the service binary and are never
  derived from the argument:
  - `%ProgramData%\VoyaVPN\runtime` (the staging root), and
  - `<profile>\AppData\Roaming\app.voyavpn.desktop\binConfigs` for each local
    user profile, enumerated from the machine's profile directory.
- The accepted config is copied into `%ProgramData%\VoyaVPN\runtime\config.json`
  and both `sing-box check` and `sing-box run` use that copy with the staging
  directory as their working directory. The installer strips inherited access
  from that directory so only SYSTEM and Administrators can write it.

## Validation

The service validates, in this order:

- the managed sing-box exists at `<service dir>\sing_box\sing-box.exe`
- the config path is absolute and is an existing file
- the canonical config path is inside one of the fixed config roots above
- the config is at most 4 MiB and parses as JSON
- `log.output` and `experimental.cache_file.path` stay inside the staging
  directory — these are the files the core writes as the service account
- every `type: "local"` `route.rule_set[].path` stays inside the staging
  directory or a VoyaVPN app-data root
- the staged copy passes `sing-box check -c <staged-config>`

Relative paths inside the config are accepted when they contain no `..`
component, because sing-box resolves them against the staging working
directory. The config itself remains user-generated: this list constrains the
file-system reach of a config, not its proxy or routing content.

The service then runs:

```text
sing-box run -c %ProgramData%\VoyaVPN\runtime\config.json --disable-color
```

## Service DACL

The desktop client is a per-user, non-elevated process and starts the service
with plain `sc.exe start` (`voya_platform::tun::start_windows_tun_service`), so
the default SCM DACL — which grants SERVICE_START to Administrators only — is
not enough. `scripts/native/windows/tunnel-service.mjs` applies this SDDL with
`sc sdset` during install:

```text
D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)
```

Compared with the SCM default this adds `RP` (SERVICE_START) and `WP`
(SERVICE_STOP) for `IU` (Interactive Users). Granting stop to interactive users
is deliberate: any local process in an interactive session can drop the tunnel,
which is a availability trade rather than an integrity one, because no caller
can influence which binary the service runs or where it writes.

## Future protocol versions

- a named-pipe control plane for start/stop/status
- a separate pre-socks config path
- service-emitted structured logs
- sing-box API health checks
