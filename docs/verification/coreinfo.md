# CoreInfo Verification

Batch: `04-01-coreinfo-process-model`

## Reference Sources

- Rollout batch requirements: `.agents/rollouts/voyavpn-full-rewrite/plan.md`
- Core launch table: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Manager/CoreInfoManager.cs`
- Runtime argument/env substitution: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Manager/CoreManager.cs`
- App/bin/config/log/temp path behavior and Unix chmod: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Common/Utils.cs`

## Implemented Surface

- `voya-platform::paths` resolves the Voya app directory plus `bin`, `binConfigs`, `guiConfigs`, `guiLogs`, and `guiTemps`.
- Portable mode follows the reference shape: use the app base directory when writable, fall back to local user data when forced or blocked by `NotStoreConfigHere.txt`.
- `voya-platform::coreinfo` contains all 15 reference core entries: `v2rayN`, `v2fly`, `v2fly_v5`, `Xray`, `mihomo`, `hysteria`, `naiveproxy`, `tuic`, `sing_box`, `juicity`, `hysteria2`, `brook`, `overtls`, `shadowquic`, and `mieru`.
- Argument substitution preserves the reference `{0}` behavior, including brook's quoted absolute config path and mihomo's `-d "{bin}"` portable data directory.
- Env resolution preserves the reference keys for `V2RAY_LOCATION_ASSET`, `XRAY_LOCATION_ASSET`, `XRAY_LOCATION_CERT`, and `MIERU_CONFIG_JSON_FILE`.
- Executable discovery probes the per-core bin subdirectory in table order, adds `.exe` on Windows, and applies executable bits on Unix for discovered binaries.
- `voya-app::runtime` exposes the platform table and launch-plan resolution without moving OS-specific behavior into the app layer.

## Verification Commands

```sh
cargo test -p voya-platform coreinfo --all-targets
cargo test -p voya-app coreinfo --all-targets
test -f docs/verification/coreinfo.md
```

All three commands passed locally.

## Skipped Checks

No external runtime checks were required for this batch. Actual core process start/stop and traffic validation are covered by later Runtime Alpha batches.
