# Windows Local TUN Testing

`pnpm build:windows:local` produces an unsigned release-profile Windows client
for local TUN testing. It builds an NSIS package for the machine's native x64
or arm64 architecture, silently installs it for the current user, and uses one
UAC prompt to install the native tunnel service.

The command is a local-only lane. It disables app and updater signing, does not
generate stable updater artifacts, does not publish anything, and does not
launch VoyaVPN automatically.

## Prerequisites

- Windows 10 or Windows 11 on x64 or arm64.
- Node.js 22 with the repository-pinned pnpm 11.5.0.
- Rust 1.96.0 with the native MSVC-hosted toolchain selected for this
  repository. For Windows x64:

  ```powershell
  rustup toolchain install 1.96.0-x86_64-pc-windows-msvc
  rustup override set 1.96.0-x86_64-pc-windows-msvc
  ```

  On Windows arm64, use `1.96.0-aarch64-pc-windows-msvc` instead.
- Visual Studio 2022 Build Tools with **Desktop development with C++**, MSVC
  v143, and a Windows 10/11 SDK.
- WebView2 Runtime. Windows 11 normally includes it; the NSIS installer uses the
  configured download bootstrapper when it is missing. The MSVC build links the
  native WebView2 loader statically, so `WebView2Loader.dll` is not an installed
  sidecar and must not be copied into the package manually.
- Internet access when the sing-box seed or WebView2 bootstrapper must be
  downloaded.

The local lane deliberately does not build MSI. WiX 3 runs MSI ICE validation
through the deprecated Windows VBScript engine, which is unavailable on some
new Windows installations. MSI remains a release-lane artifact and must be
built and tested on the prepared Windows release host.

Install the repository dependencies from a normal PowerShell first:

```powershell
corepack enable
corepack prepare pnpm@11.5.0 --activate
pnpm install --frozen-lockfile
```

## Build And Install

Quit the VoyaVPN GUI and disable TUN, then run from a normal, non-elevated
PowerShell:

```powershell
pnpm build:windows:local
```

The command performs these steps:

1. Rejects unsupported architectures and a running `voyavpn.exe`.
2. Allows an existing current-user NSIS install only at
   `%LOCALAPPDATA%\VoyaVPN`; it refuses to replace MSI or other installations.
3. Runs an unsigned release build for `nsis`.
4. Builds `voyavpn-tunnel-service.exe` separately.
5. Silently installs the NSIS package for the current user.
6. Opens one UAC prompt, stops an old `VoyaVPNTunnelService` when necessary,
   copies the service into `%ProgramFiles%\VoyaVPN`, and creates or updates the
   demand-start service.
7. Verifies the installed client, protected service binary, service
   registration, and stopped service state.

Expected paths are:

```text
%LOCALAPPDATA%\VoyaVPN\voyavpn.exe
%ProgramFiles%\VoyaVPN\voyavpn-tunnel-service.exe
target\<rust-target>\release\bundle\nsis\VoyaVPN_<version>_<arch>-setup.exe
```

The command prints the resolved paths and a `Start-Process` command when it
finishes. Launch the installed copy rather than
`target\<rust-target>\release\voyavpn.exe`.

## Verify The Tunnel

The service should be installed but stopped until the app enables TUN:

```powershell
sc.exe qc VoyaVPNTunnelService
sc.exe query VoyaVPNTunnelService
Start-Process "$env:LOCALAPPDATA\VoyaVPN\voyavpn.exe"
```

After enabling TUN in VoyaVPN:

1. Confirm the service enters `RUNNING`.
2. Confirm browser and terminal traffic follow the same routing rules.
3. Confirm DNS resolution uses the generated sing-box configuration.
4. Disable TUN and verify the service stops, Wintun routes disappear, DNS is
   restored, and no sing-box process remains.

Re-running `pnpm build:windows:local` updates the same local NSIS installation
and replaces the protected service binary without starting the service.

## Troubleshooting

- **VoyaVPN is still running** — quit the tray application and disable TUN;
  the script intentionally never force-kills the GUI.
- **Existing MSI or non-local installation** — uninstall that copy explicitly.
  The local command will not silently migrate or replace a production install.
- **UAC was cancelled** — rerun the command and approve only the final service
  installation prompt. The build and current-user NSIS installation do not run
  elevated.
- **Service stop timed out** — disable TUN, inspect
  `sc.exe query VoyaVPNTunnelService`, then retry. The script will not overwrite
  a running service executable.
- **Missing sing-box seed** — run `pnpm core:sing-box:install` and rebuild.

## Teardown

Disable TUN and quit VoyaVPN. In an elevated PowerShell, unregister the service
and remove only its managed executable:

```powershell
pnpm native:windows:tunnel:uninstall
```

The service helper does not recursively delete `%ProgramFiles%\VoyaVPN`; it
removes only `voyavpn-tunnel-service.exe`. Uninstall the current-user client
separately:

```powershell
& "$env:LOCALAPPDATA\VoyaVPN\uninstall.exe"
```

Never distribute artifacts from this local unsigned lane.
