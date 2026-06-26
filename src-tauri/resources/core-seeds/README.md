# Bundled core seeds

This directory holds the proxy-core binaries that ship inside the app bundle as **seeds**.
At startup the app copies each seed into the per-user core dir:

```
resources/core-seeds/<core>/        ->  {appConfigDir}/bin/<core>/
resources/core-seeds/xray/xray.exe  ->  …/app.voyavpn.desktop/bin/xray/xray.exe
```

(See `crates/voya-platform/src/coreinfo.rs::copy_seed_core_asset` and
`src-tauri/src/lib.rs` startup.) The folder name must match
`core_type_dir_name` — Xray → `xray`, mihomo → `mihomo`, sing-box → `sing_box` —
and the executable must be the OS-resolved name (`xray.exe` on Windows, `xray` on Unix).

## Populating it

Binaries are **not** committed (large, separately licensed). Fetch them for your host
platform before bundling:

```
node scripts/fetch-cores.mjs
# or pin a version:
XRAY_VERSION=v26.3.27 node scripts/fetch-cores.mjs
```

This downloads the pinned Xray release, verifies its SHA256, and stages `xray.exe`
(plus `geoip.dat` / `geosite.dat` for `XRAY_LOCATION_ASSET`) into `xray/`.

## Recovery path

If a user's `bin/<core>/` is empty at connect time, the app surfaces a one-click
**Install core** prompt that re-runs the seed copy (`install_core_seed` command). If no
seed is bundled, it falls back to the **Updates** download flow.
