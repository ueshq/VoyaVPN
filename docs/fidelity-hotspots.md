# Fidelity Hotspots

Batch: `00-01-baseline-inventory`

Reference root: `/Users/afu/Dev/refs/v2rayN/v2rayN`

These are the parity points most likely to drift during the rewrite. Each item lists concrete v2rayN evidence and the VoyaVPN owners that should carry the behavior.

## Sudo Lifecycle And Elevated Runtime

Reference paths:

- `ServiceLib/Manager/CoreManager.cs`
- `ServiceLib/Manager/CoreAdminManager.cs`
- `ServiceLib/Manager/AppManager.cs`
- `ServiceLib/ViewModels/StatusBarViewModel.cs`
- `v2rayN.Desktop/Views/SudoPasswordInputView.axaml`
- `v2rayN.Desktop/Views/SudoPasswordInputView.axaml.cs`
- `ServiceLib/Sample/kill_as_sudo_linux_sh`
- `ServiceLib/Sample/kill_as_sudo_osx_sh`

VoyaVPN owners: `crates/voya-platform::elevation`, `crates/voya-platform::tun`, `crates/voya-app::supervisor`, `src/features/tun`, `src/ipc`.

Parity rules:

- On Unix, collect sudo password when the user enables TUN, store it only in memory, and zeroize it on stop/shutdown. Do not persist it to SQLite or JSON.
- Linux and macOS both use `sudo -S`; the OS-specific difference is the embedded kill script name.
- The supervisor reads the in-memory password synchronously when a sudo-wrapped process is spawned.
- `AllowEnableTun` semantics depend on a non-empty sudo password on Linux/macOS.
- Teardown order is sudo kill first, then main process, then pre process. Windows job containment and Unix sudo PID cleanup are part of the same lifecycle.
- Future tests should cover password-required UI events, wrong-password rejection, process start, clean stop, crash cleanup, and no orphaned elevated process.

## Xray `finalmask`

Reference paths:

- `ServiceLib/Models/Entities/ProfileItem.cs`
- `ServiceLib/Models/CoreConfigs/V2rayConfig.cs`
- `ServiceLib/Handler/Fmt/BaseFmt.cs`
- `ServiceLib/Handler/Builder/NodeValidator.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayOutboundService.cs`
- `ServiceLib/Services/CoreConfig/V2ray/CoreConfigV2rayService.cs`

VoyaVPN owners: `crates/voya-core::fmt`, `crates/voya-core::coregen::xray`, `tests/golden/xray`.

Parity rules:

- Model `finalmask` as one subsystem, not separate fragment and noise features.
- Share links use `fm`; valid JSON is normalized on import/export, otherwise preserve the raw string behavior.
- Per-transport generation can set `streamSettings.finalmask` for KCP and hysteria/quic.
- `_node.Finalmask` overrides generated transport masks when present.
- `ApplyOutboundFragment` merges fragment into `finalmask["tcp"]` and noise into `finalmask["udp"]` only when those arrays are empty, and skips chained outbounds with `dialerProxy`.
- Golden fixtures must include raw finalmask, KCP finalmask, hysteria/quic finalmask, fragment/noise merge, and proxy-chain outbounds.

## Policy Groups

Reference paths:

- `ServiceLib/Enums/EMultipleLoad.cs`
- `ServiceLib/Models/Entities/ProfileGroupItem.cs`
- `ServiceLib/Models/Entities/ProtocolExtraItem.cs`
- `ServiceLib/ViewModels/AddGroupServerViewModel.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayBalancerService.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayRoutingService.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayConfigTemplateService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxOutboundService.cs`

VoyaVPN owners: `crates/voya-core::groups`, `crates/voya-core::coregen::{xray,singbox}`, `crates/voya-app::profiles`, `src/features/groups`.

Parity rules:

- Preserve `EMultipleLoad` behavior for least ping, fallback, random, round robin, and least load.
- Xray policy groups emit balancers and observatory/burst-observatory structures with the same subject selector ordering and fallback tags.
- Xray templates append/merge generated balancers and observatory rather than dropping generated group behavior.
- sing-box groups map to selector/urltest behavior with the same child filtering and tag shape as reference output.
- Group UI must validate child presence, subscription child source, and disallow invalid nested types consistently with the reference.

## Proxy Chains

Reference paths:

- `ServiceLib/Enums/EConfigType.cs`
- `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs`
- `ServiceLib/Handler/Builder/NodeValidator.cs`
- `ServiceLib/Models/Entities/ProfileGroupItem.cs`
- `ServiceLib/Models/Entities/ProtocolExtraItem.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayOutboundService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxOutboundService.cs`

VoyaVPN owners: `crates/voya-core::groups`, `crates/voya-core::coregen::{xray,singbox}`, `crates/voya-app::subscriptions`, `src/features/groups`.

Parity rules:

- `PolicyGroup = 101` and `ProxyChain = 102` discriminants are stable config contracts.
- Subscription `PrevProfile` and `NextProfile` create a virtual proxy-chain node in the context builder before generation.
- Xray chains use `streamSettings.sockopt.dialerProxy`; sing-box chains use `detour`.
- 2-hop and 3-hop mixed-core chains, sub-chains, and chain-start fanout need explicit golden fixtures.
- Chain finalmask/fragment handling must not apply to outbounds whose traffic is chained through another dialer.

## DNS

Reference paths:

- `ServiceLib/Models/Entities/DNSItem.cs`
- `ServiceLib/Models/Configs/ConfigItems.cs`
- `ServiceLib/ViewModels/DNSSettingViewModel.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayDnsService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxDnsService.cs`
- `ServiceLib/Sample/dns_v2ray_normal`
- `ServiceLib/Sample/dns_singbox_normal`
- `ServiceLib/Sample/tun_singbox_dns`
- `ServiceLib/Sample/singbox_fakeip_filter`

VoyaVPN owners: `crates/voya-core::dns`, `crates/voya-core::coregen::{xray,singbox}`, `crates/voya-db`, `src/features/dns`.

Parity rules:

- Cover simple DNS and per-core advanced raw DNS.
- Preserve fakeip, global/non-global fakeip, fakeip filters, hosts, system hosts, common hosts, expected IPs, bootstrap DNS, strategies, serve-stale, parallel query, and block-binding-query behavior.
- Xray DNS adds routing rules and direct DNS tags; sing-box DNS uses the newer typed server schema with `type`, `domain_resolver`, `predefined`, `action: "predefined"`, `independent_cache`, and `inet4_range 198.18.0.0/15`.
- Final DNS/direct detection depends on the last routing rule shape and must be fixture-tested for direct and proxy cases.
- Regional preset DNS fetches depend on network access and must have null fallback behavior documented in implementation tests.

## Statistics

Reference paths:

- `ServiceLib/Manager/StatisticsManager.cs`
- `ServiceLib/Services/Statistics/StatisticsXrayService.cs`
- `ServiceLib/Services/Statistics/StatisticsSingboxService.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayStatisticService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxStatisticService.cs`
- `ServiceLib/Models/Entities/ServerStatItem.cs`
- `ServiceLib/Models/Dto/ServerSpeedItem.cs`
- `ServiceLib/ViewModels/StatusBarViewModel.cs`

VoyaVPN owners: `crates/voya-app::stats`, `crates/voya-db`, `src/features/status`, `src/features/server-table`.

Parity rules:

- Xray stats require generated API/metrics/stats config when statistics or real-time speed is enabled.
- sing-box stats consume the Clash API traffic WebSocket on `StatePort2`.
- Keep live speed snapshots separate from persistent per-server totals.
- Persist `ServerStatItem`-equivalent totals and reset today counters at date rollover.
- UI should show proxy/direct realtime displays and traffic columns without overloading the hot path.

## System Proxy

Reference paths:

- `ServiceLib/Enums/ESysProxyType.cs`
- `ServiceLib/Handler/SysProxy/SysProxyHandler.cs`
- `ServiceLib/Handler/SysProxy/ProxySettingWindows.cs`
- `ServiceLib/Handler/SysProxy/ProxySettingLinux.cs`
- `ServiceLib/Handler/SysProxy/ProxySettingOSX.cs`
- `ServiceLib/Manager/PacManager.cs`
- `ServiceLib/Sample/pac`
- `ServiceLib/Sample/proxy_set_linux_sh`
- `ServiceLib/Sample/proxy_set_osx_sh`
- `ServiceLib/ViewModels/StatusBarViewModel.cs`

VoyaVPN owners: `crates/voya-platform::sysproxy`, `crates/voya-platform::pac`, `crates/voya-app::settings`, `src/features/status`.

Parity rules:

- Preserve modes: forced clear, forced change, unchanged, and PAC.
- PAC is Windows-only and uses a local PAC endpoint with cache-busting query ticks.
- Windows mutates Internet Settings and falls back as reference does; Linux/macOS execute generated shell scripts.
- System proxy must restore on disconnect, app exit, crash restart, and forced disable.
- Status bar mode, tray icon, and proxy command copy text should update after every mode change.

## TUN

Reference paths:

- `ServiceLib/Models/Configs/ConfigItems.cs`
- `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs`
- `ServiceLib/Handler/ConfigHandler.cs`
- `ServiceLib/Manager/CoreManager.cs`
- `ServiceLib/Manager/CoreAdminManager.cs`
- `ServiceLib/Common/WindowsUtils.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayInboundService.cs`
- `ServiceLib/Services/CoreConfig/V2ray/V2rayRoutingService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxInboundService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxDnsService.cs`
- `ServiceLib/Services/CoreConfig/Singbox/SingboxRoutingService.cs`
- `ServiceLib/Sample/SampleTunInbound`
- `ServiceLib/Sample/SampleTunRules`
- `ServiceLib/Sample/tun_singbox_inbound`
- `ServiceLib/Sample/tun_singbox_rules`

VoyaVPN owners: `crates/voya-platform::tun`, `crates/voya-platform::elevation`, `crates/voya-core::coregen::{xray,singbox}`, `crates/voya-app::supervisor`, `src/features/tun`.

Parity rules:

- sing-box and mihomo own the elevated TUN datapath on non-Windows.
- Xray separately emits a `tun` inbound when TUN is on; this is config generation, not the same as sudo-wrapped process launching.
- Interface naming must preserve macOS random `utun{n}` and non-mac names (`xray_tun`, `singbox_tun`) unless a later ADR changes it with tests.
- Preserve MTU, stack, auto route, strict route, IPv6 gateway, bind interface, fakeip-under-TUN, and TUN routing rules.
- Windows removes stale TUN devices before start with the reference device names and delay shape.
- Main/pre context generation must keep main core from handling TUN directly when pre-socks is used.

## Clash PATCH

Reference paths:

- `ServiceLib/Manager/ClashApiManager.cs`
- `ServiceLib/ViewModels/ClashProxiesViewModel.cs`
- `ServiceLib/ViewModels/ClashConnectionsViewModel.cs`
- `ServiceLib/Models/Dto/ClashProxies.cs`
- `ServiceLib/Models/Dto/ClashProviders.cs`
- `ServiceLib/Models/Dto/ClashConnections.cs`
- `ServiceLib/Services/CoreConfig/CoreConfigClashService.cs`

VoyaVPN owners: `crates/voya-net::clash`, `crates/voya-app::clash`, `src/features/clash-proxies`, `src/features/clash-connections`.

Parity rules:

- Rule-mode changes call HTTP `PATCH` on `/configs`; do not use PUT for this operation.
- Config reload calls PUT on `/configs?force=true` after closing connections.
- Active proxy selection calls PUT on `/proxies/{name}`.
- Proxies and providers are fetched together; connections are fetched/closed through `/connections`.
- Traffic and connections live updates should use WebSocket/polling behavior appropriate to the reference UI and core API.

## QR Scope

Reference paths:

- `ServiceLib/Common/QRCodeUtils.cs`
- `ServiceLib/Models/Dto/VmessQRCode.cs`
- `ServiceLib/ViewModels/MainWindowViewModel.cs`
- `ServiceLib/ViewModels/StatusBarViewModel.cs`
- `ServiceLib/Events/AppEvents.cs`
- `v2rayN/Common/QRCodeWindowsUtils.cs`
- `v2rayN.Desktop/Common/QRCodeAvaloniaUtils.cs`
- `v2rayN/Views/QrcodeView.xaml`
- `v2rayN.Desktop/Views/QrcodeView.axaml`
- `v2rayN/Views/MainWindow.xaml.cs`
- `v2rayN.Desktop/Views/MainWindow.axaml.cs`

VoyaVPN owners: `crates/voya-app::qr`, `src/features/qr`, `crates/voya-platform::capture` only if a native capture adapter is needed.

Parity rules:

- Backend scope is QR generation for share URLs and typed import of decoded text.
- Screen capture and image scanning are frontend/platform concerns; do not put OS capture APIs or image-decoder UI behavior in `voya-core`.
- Preserve separate app events for scan-from-screen and scan-from-image so permissions/errors can be handled by the shell.
- Parsed scan results feed the same import pipeline as clipboard/link import.

## Verification Follow-Up

- This batch records source evidence only. No cargo, pnpm, golden, core acceptance, or OS smoke checks were run because the implementation scaffold and fixtures are not present yet.
- Follow-up batches must convert each hotspot into unit, golden, IPC, or OS-smoke verification. The highest-risk first fixtures are: finalmask, policy group observatory ordering, 2/3-hop proxy chains, DNS final/direct detection, sudo/TUN teardown, system proxy restore, Clash rule-mode PATCH, and QR generation/import separation.
