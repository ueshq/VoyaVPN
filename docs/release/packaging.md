# VoyaVPN Packaging Runbook

Batch: `08-01-tauri-packaging`

## Current Package Configuration

Tauri packaging is configured in `src-tauri/tauri.conf.json` for the public beta bundle matrix:

| Platform | Bundle targets | Signing posture |
| --- | --- | --- |
| macOS | `.app`, `.dmg` | `hardenedRuntime` is enabled, but no signing identity is configured in the repo. Developer ID signing and notarization are manual release steps. |
| Windows | NSIS, MSI | NSIS defaults to current-user install. MSI has a pinned upgrade code: `81f9b48c-cd6b-566b-9904-9f89ac741525`. Authenticode signing is a manual release step. |
| Linux | `.deb`, `.rpm`, `.AppImage` | Package metadata is configured, but repository publication and checksum signing are manual release steps. |

The required local debug build is intentionally unsigned:

```sh
pnpm tauri:build --debug
```

The package script runs through `scripts/tauri-build.mjs`, which forwards all Tauri CLI arguments and normalizes `CI=1`/`CI=0` to the boolean strings required by the Tauri 2 CLI. This keeps local and runner debug packaging deterministic without requiring signing credentials.

`bundle.createUpdaterArtifacts` is `false` by default so local debug packaging does not require updater private keys. Release jobs may override this only after the updater public key and signing secrets have been provisioned.

Local verification for this batch passed with unsigned debug artifacts:

- `pnpm tauri:build --debug`
- `test -f docs/release/packaging.md`

The debug build produced:

- `target/debug/bundle/macos/VoyaVPN.app`
- `target/debug/bundle/dmg/VoyaVPN_0.1.0_x64.dmg`

The bundled notices resource was present at `target/debug/bundle/macos/VoyaVPN.app/Contents/Resources/release/THIRD_PARTY_NOTICES.md`.

## Updater Metadata

The Tauri updater plugin is registered in `src-tauri/src/lib.rs` and configured under `plugins.updater`:

- Public key placeholder: `VOYAVPN_UPDATER_PUBLIC_KEY_PLACEHOLDER_REPLACE_BEFORE_RELEASE`.
- Channel placeholder: the endpoint query currently pins `channel=beta`.
- Endpoint template: `https://updates.voyavpn.example/{{target}}/{{arch}}/{{current_version}}?channel=beta`.
- Windows updater install mode: `passive`.

Before a real beta release:

1. Generate the updater keypair outside the repo:

   ```sh
   pnpm tauri signer generate --write-keys <secure-private-key-path> --ci
   ```

2. Replace only the public key placeholder in `src-tauri/tauri.conf.json`.
3. Store the private key in CI or local release secrets through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`. Store the key password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when one is used.
4. Enable updater artifact creation for release builds, publish the generated update archives and signatures, and publish one channel metadata document per channel.
5. Keep app updater metadata separate from core, geo, and ruleset update metadata. Core updates continue through the in-app update manager and are stored under the user app config directory.

## Core And Sidecar Policy

No proxy core binary is bundled by default:

- `bundle.externalBin` is an empty list.
- The only bundled release resource is `docs/release/THIRD_PARTY_NOTICES.md`.
- Runtime core lookup and downloads use the app data `bin/` tree, not the installer payload.
- GPL and AGPL cores, including sing-box, mihomo, and juicity, must remain download-on-first-run or user-supplied unless there is explicit legal approval for a separate distribution.

Optional sidecars for future builds must follow this rule:

1. Add sidecars only through an explicit release profile or platform config overlay.
2. Document the exact binary name, version, license, source URL, checksum, and legal approval.
3. Keep GPL or AGPL sidecars out of the default public beta installers.
4. Re-run package builds on every target OS after adding any sidecar because Tauri resolves sidecars by target triple.

## First-Run Core Download Flow

First run should not assume any core executable is present in the bundle.

1. App startup creates the app config, `bin/`, `binConfigs/`, log, backup, and temp directories.
2. The profile table and status bar may show profiles before cores exist, but connect should surface a typed missing-core error instead of failing silently.
3. The user opens Check Updates, keeps the selected core targets, and downloads required cores. The update manager fetches through proxy first when available, then falls back to direct download.
4. Downloaded archives are staged under temp update paths, extracted into the appropriate `bin/<core>/` directory, and made executable on Unix.
5. Core launch discovery uses the same app data `bin/` tree for Xray, sing-box, mihomo, and later supported cores.
6. Geo files and sing-box rulesets are acquired separately from the app updater so ruleset or core refreshes do not require an app release.

## Attribution And Licenses

The bundled attribution document is `docs/release/THIRD_PARTY_NOTICES.md`. It records the app license, Tauri and frontend framework licenses, and the current core acquisition policy.

Keep this document current whenever a runtime core, bundled asset, or release dependency changes. For default beta installers, the notices document must not imply that GPL or AGPL cores are redistributed inside the installer.

## Manual Release Checks Not Run By This Batch

The automated runner does not have signing certificates, notarization credentials, updater private keys, package repository credentials, or real Windows/Linux/macOS smoke machines. Capture those checks manually before publication:

- macOS Developer ID sign, notarize with `notarytool`, staple, install `.dmg`, launch, and verify TUN/elevation prompts.
- Windows Authenticode sign NSIS/MSI, install as current user, launch, uninstall, and verify WebView2 bootstrap behavior.
- Linux install `.deb`, `.rpm`, and `.AppImage` on clean distributions, verify desktop entry, execute bit, and core download path.
- Publish updater metadata to the beta channel and verify that an older signed build sees the update.
