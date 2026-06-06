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

`bundle.createUpdaterArtifacts` is `false` by default so local debug packaging does not require updater private keys. Stable release jobs do not edit the committed config; `scripts/tauri-build.mjs` writes a generated overlay at `target/release-config/tauri.updater.stable.generated.json` when `VOYAVPN_RELEASE_CHANNEL=stable` or `VOYAVPN_TAURI_UPDATER_CONFIG=stable`.

## Release Workflow Matrix

The release workflow packages six stable target entries and preserves these names in artifact manifests, updater metadata, and release-index evidence:

| Stable target | Rust target | Notes |
| --- | --- | --- |
| `darwin-x86_64` | `x86_64-apple-darwin` | Native Intel macOS hosted runner. |
| `darwin-aarch64` | `aarch64-apple-darwin` | Native Apple Silicon hosted runner. |
| `windows-x86_64` | `x86_64-pc-windows-msvc` | Native Windows x64 hosted runner. |
| `windows-aarch64` | `aarch64-pc-windows-msvc` | Requires hosted or self-hosted Windows arm64 runner capacity. |
| `linux-x86_64` | `x86_64-unknown-linux-gnu` | Native Ubuntu x64 hosted runner. |
| `linux-aarch64` | `aarch64-unknown-linux-gnu` | Requires hosted or self-hosted Ubuntu arm64 runner capacity. |

The workflow uploads package artifacts, `SHA256SUMS`, `artifact-manifest.json`, updater metadata when requested, and CDN staging `release-index` evidence as GitHub Actions artifacts. It does not upload to the CDN, mutate stable pointers, purge caches, sign externally, or notarize; those remain release-owner gates.

Local verification for this batch passed with unsigned debug artifacts:

- `pnpm tauri:build --debug`
- `test -f docs/release/packaging.md`

The debug build produced:

- `target/debug/bundle/macos/VoyaVPN.app`
- `target/debug/bundle/dmg/VoyaVPN_0.1.0_x64.dmg`

The bundled notices resource was present at `target/debug/bundle/macos/VoyaVPN.app/Contents/Resources/release/THIRD_PARTY_NOTICES.md`.

## CDN Release Index

`scripts/release-index.mjs` turns one or more `artifact-manifest.json` files from `scripts/release-artifacts.mjs` into the manual-download CDN release index and a sibling evidence JSON file.

Stable generation requires `--base-url` or `VOYAVPN_CDN_BASE_URL`. Every generated artifact URL is derived from that base URL; artifact manifest URL fields are not trusted. Stable generation fails when the base URL is missing, empty, an example host, or a GitHub host.

Required stable artifact fields:

- `channel`
- `version`
- `target` or an inferable platform such as Windows, macOS, or Linux
- `arch` or an inferable `x64`/`arm64` architecture
- `kind`
- `path` or `name`
- `bytes`
- `sha256`
- `originalName`

Fixture generation:

```sh
node scripts/release-index.mjs --input tests/fixtures/release/artifacts --out /tmp/voyavpn-release-index.json --base-url <cdn-base-url> --channel stable
```

The evidence file defaults to the output filename with `.evidence.json`, for example `/tmp/voyavpn-release-index.evidence.json`.

## Core Asset Manifest

`scripts/core-assets.mjs` turns fixture input into the stable core asset manifest consumed by later core seed/update work. The first stable manifest covers Xray, mihomo, and sing-box for Windows, macOS, and Linux on x64 and arm64.

Stable generation requires `--base-url` or `VOYAVPN_CDN_BASE_URL`. Generated `url` values are always derived from that CDN base URL plus each fixture `path`; fixture download URL fields are not trusted. GitHub URLs are allowed only in `upstreamUrl`, where they record source and license reference material.

Required stable core asset fields:

- `coreType`: one of `Xray`, `mihomo`, or `sing_box`
- `version`
- `license`
- `os`: `windows`, `macos`, or `linux`
- `arch`: `x64` or `arm64`
- `archiveFormat`: `zip`, `tar.gz`, or `gz`
- `executableCandidates`: ordered candidate names to probe after extraction
- `path` or `name`: relative CDN artifact path used to derive `url`
- `sha256`
- `bytes`
- `upstreamUrl`: source reference URL, not the stable production download URL

Stable validation fails when a core type is unknown, an OS or architecture is outside the first-stable matrix, a required matrix entry is missing, a checksum or size is invalid, the CDN base URL is an example or GitHub host, or a GitHub URL is supplied as a production download URL.

Fixture generation:

```sh
node scripts/core-assets.mjs --fixture tests/fixtures/release/core-assets.json --out /tmp/voyavpn-core-assets.json --base-url <cdn-base-url>
```

The evidence file defaults to the output filename with `.evidence.json`, for example `/tmp/voyavpn-core-assets.evidence.json`.

## Core Distribution Classes

Keep these release assets separate in manifests, package resources, and evidence:

| Distribution class | Contents | Host or package location | Release gate |
| --- | --- | --- | --- |
| Bundled core seed assets | Optional first-stable seed files for Xray, mihomo, and sing-box only. At runtime they are copied from app resources such as `core-seeds/xray/`, `core-seeds/mihomo/`, and `core-seeds/sing_box/` into app data `bin/<core>/` before execution. | Stable package resources only; never `bundle.externalBin`, and never executed from the read-only app bundle. | Requires the core redistribution approval record in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), including source URL, license name, SHA-256, byte size, and source availability evidence for each seed file. |
| App updater payloads | Signed Tauri application update archives, matching `.sig` files, and `latest.json` metadata. They update the VoyaVPN app package, not proxy cores, geo data, or SRS rulesets. | Approved updater CDN base URL from `VOYAVPN_UPDATES_BASE_URL`. | Requires updater key provisioning, signed payload evidence, app artifact checksums, and OS smoke. |
| Core update CDN assets | Core archives referenced by the stable core asset manifest for Xray, mihomo, and sing-box. These are downloaded, checksum-verified, staged, extracted, chmodded on Unix, safely swapped into app data, and rolled back on failure. | Approved core CDN paths derived from `VOYAVPN_CDN_BASE_URL`; GitHub appears only as `upstreamUrl` source evidence. | Requires core manifest evidence, artifact checksums, upstream source/license references, source availability evidence, and the same legal approval checkpoint as bundled seed assets. |

Bundled seed assets are a startup convenience for missing app-data cores. They are not a replacement for the core update CDN manifest, and they do not change the Tauri app updater payload. A stable package may include seed assets only for the listed three core types; adding juicity, v2fly, hysteria, or any other core requires a separate release profile and legal notice update.

## Updater Metadata

The Tauri updater plugin is registered in `src-tauri/src/lib.rs`. The committed `src-tauri/tauri.conf.json` intentionally omits `plugins.updater` and keeps `bundle.createUpdaterArtifacts` disabled so local debug builds remain credential-free.

Stable packaging uses the generated overlay from `scripts/tauri-build.mjs`:

- Overlay path: `target/release-config/tauri.updater.stable.generated.json`.
- `bundle.createUpdaterArtifacts`: `true`.
- `plugins.updater.pubkey`: read from `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY`.
- `plugins.updater.endpoints`: `<VOYAVPN_UPDATES_BASE_URL>/latest.json`.
- Windows updater install mode: `passive`.

Before a real stable release:

1. Generate the updater keypair outside the repo:

   ```sh
   pnpm tauri signer generate --write-keys <secure-private-key-path> --ci
   ```

2. Store only the public key in `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY`; do not commit it into the base config.
3. Store the private key in CI or local release secrets through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`. Store the key password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when one is used.
4. Set `VOYAVPN_UPDATES_BASE_URL` to the approved HTTPS stable updater CDN base URL.
5. Generate and inspect the overlay before packaging:

   ```sh
   VOYAVPN_RELEASE_CHANNEL=stable \
   VOYAVPN_UPDATES_BASE_URL=<stable-updater-cdn-base-url> \
   VOYAVPN_UPDATER_PUBLIC_KEY=<approved-public-key> \
   TAURI_SIGNING_PRIVATE_KEY_PATH=<secure-private-key-path> \
   pnpm tauri:stable-updater-config
   ```

6. Run the stable readiness check against the generated overlay:

   ```sh
   node scripts/check-release-readiness.mjs --mode stable --cdn-base-url <stable-cdn-base-url> --updates-base-url <stable-updater-cdn-base-url> --diagnostics-endpoint <stable-diagnostics-endpoint> --tauri-config target/release-config/tauri.updater.stable.generated.json
   ```

7. Build release packages with the same environment. The wrapper passes the generated overlay to `tauri build` through `--config`.
8. Publish the generated update archives, signatures, and one channel metadata document per channel only after external signing and smoke gates pass.
9. Keep app updater metadata separate from core, geo, and ruleset update metadata. Core updates continue through the in-app update manager and are stored under the user app config directory.

## Core And Sidecar Policy

Debug packages and credential-free release dry runs do not bundle proxy core binaries:

- `bundle.externalBin` is an empty list.
- The only bundled release resource is `docs/release/THIRD_PARTY_NOTICES.md`.
- Runtime core lookup and downloads use the app data `bin/` tree, not the installer payload.
- GPL and AGPL cores must remain download-on-first-run or user-supplied unless there is explicit legal approval for a separate distribution.

Production stable seed redistribution is the only approved packaging exception in this rollout, and it is conditional until the approval record exists:

1. Seed resources may include only Xray, mihomo, and sing-box for the first-stable Windows, macOS, and Linux x64/arm64 matrix.
2. The release owner must record the exact binary name, version, license, source URL, checksum, byte size, source availability evidence, and legal approval before stable publication.
3. Seed resources are copied into app data `bin/<core>/` before runtime discovery; do not execute them from the app bundle.
4. `bundle.externalBin` remains empty for proxy cores so Tauri sidecar resolution does not blur app payloads with core seed assets.
5. AGPL cores and unsupported cores are not bundled or published as first-stable core CDN assets by this rollout.

Optional sidecars for future builds must follow this rule:

1. Add sidecars only through an explicit release profile or platform config overlay.
2. Document the exact binary name, version, license, source URL, checksum, and legal approval.
3. Keep GPL or AGPL sidecars out of default installers unless the approval record explicitly covers sidecar redistribution and source availability.
4. Re-run package builds on every target OS after adding any sidecar because Tauri resolves sidecars by target triple.

## First-Run Core Download Flow

First run should not assume any core executable is present in debug or dry-run packages. Stable packages may copy approved Xray, mihomo, or sing-box seed assets into app data when present, but missing seed assets still fall back to the normal core update flow.

1. App startup creates the app config, `bin/`, `binConfigs/`, log, backup, and temp directories.
2. The profile table and status bar may show profiles before cores exist, but connect should surface a typed missing-core error instead of failing silently.
3. On startup or connect, approved seed assets for Xray, mihomo, and sing-box are copied from package resources into app data only when the target core is missing.
4. The user opens Check Updates, keeps the selected core targets, and downloads required cores. The update manager fetches through proxy first when available, then falls back to direct download.
5. Downloaded archives are staged under temp update paths, extracted into the appropriate `bin/<core>/` directory, and made executable on Unix.
6. Core launch discovery uses the same app data `bin/` tree for Xray, sing-box, mihomo, and later supported cores.
7. Geo files and sing-box rulesets are acquired separately from the app updater so ruleset or core refreshes do not require an app release.

## Attribution And Licenses

The bundled attribution document is `docs/release/THIRD_PARTY_NOTICES.md`. It records the app license, Tauri and frontend framework licenses, stable seed redistribution scope, upstream source URLs, license names, checksum expectations, and source availability expectations for Xray, mihomo, and sing-box.

Keep this document current whenever a runtime core, bundled seed asset, core update CDN asset, or release dependency changes. Notices must not imply that unsupported cores are bundled, and they must not claim GPL or AGPL redistribution approval until the stable approval checkpoint is attached to the release evidence.

## Manual Release Checks Not Run By This Batch

The automated runner does not have signing certificates, notarization credentials, updater private keys, package repository credentials, or real Windows/Linux/macOS smoke machines. Capture those checks manually before publication:

- macOS Developer ID sign, notarize with `notarytool`, staple, install `.dmg`, launch, and verify TUN/elevation prompts.
- Windows Authenticode sign NSIS/MSI, install as current user, launch, uninstall, and verify WebView2 bootstrap behavior.
- Linux install `.deb`, `.rpm`, and `.AppImage` on clean distributions, verify desktop entry, execute bit, and core download path.
- Publish updater metadata to the beta channel and verify that an older signed build sees the update.
