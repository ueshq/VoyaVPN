# Bundled sing-box seed

This directory holds the sing-box binary that ships inside the app bundle as a
**seed**. At startup the app copies the seed into the per-user core dir:

```
resources/core-seeds/<core>/        ->  {appConfigDir}/bin/<core>/
resources/core-seeds/sing_box/sing-box.exe  ->  …/app.voyavpn.desktop/bin/sing_box/sing-box.exe
```

(See `crates/voya-platform/src/coreinfo.rs::copy_seed_core_asset` and
`src-tauri/src/lib.rs` startup.) The folder name must match
`core_type_dir_name` — sing-box → `sing_box` — and the executable must be the
OS-resolved name (`sing-box.exe` on Windows, `sing-box` on Unix).

## Populating it

Binaries are **not** committed (large, separately licensed). A normal local
`pnpm install` runs the root `postinstall` hook, which fetches the pinned
sing-box release for the host platform, stages it here, and copies it into the
local app data `bin/sing_box/` directory so development builds can connect
immediately.

If install scripts were skipped, or you need to repair the local app data copy,
run:

```
pnpm core:sing-box:install
```

To only populate bundled seed resources before a package build, run:

```
node scripts/fetch-cores.mjs
# or pin a version:
SING_BOX_VERSION=v1.13.14 node scripts/fetch-cores.mjs
```

Set `VOYAVPN_SKIP_SING_BOX_POSTINSTALL=1` to skip the postinstall fetch. CI
skips it by default unless `VOYAVPN_FETCH_SING_BOX_ON_INSTALL=1` is set. Tauri
package builds still ensure the sing-box seed is present before generating the
resource overlay.

This downloads the pinned sing-box release, records its SHA256 in
`sing-box.seed.json`, and stages `sing-box` or `sing-box.exe` into `sing_box/`.

## Recovery path

If a user's `bin/sing_box/` is empty at connect time, the app surfaces a
one-click **Install core** prompt that re-runs the seed copy
(`install_core_seed` command). There is no core download/update fallback;
sing-box updates are delivered by shipping a new app package.
