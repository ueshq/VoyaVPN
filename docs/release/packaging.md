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

`bundle.createUpdaterArtifacts` stays `false` in the committed config by design. The base `src-tauri/tauri.conf.json` is credential-free and safe for local debug builds, CI dry runs, and code review because it does not contain updater endpoints, updater public keys, private-key paths, or generated release state. Stable release jobs do not edit the committed config; `scripts/tauri-build.mjs` writes a generated overlay at `target/release-config/tauri.updater.stable.generated.json` when `VOYAVPN_RELEASE_CHANNEL=stable` or `VOYAVPN_TAURI_UPDATER_CONFIG=stable`.

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

## Workflow CDN Staging Inputs

For a stable, non-dry-run release, CDN staging starts from GitHub Actions artifacts produced by the `Release` workflow, not from `tests/fixtures`.

The six package artifacts named `voyavpn-stable-<release_target>-release` are the app package inputs for manual CDN staging. Each package artifact contains the normalized package files, `SHA256SUMS`, and `artifact-manifest.json`; `scripts/release-index.mjs` and `scripts/release-updater-metadata.mjs` use those manifests as source evidence.

The metadata artifacts are:

- `voyavpn-stable-cdn-staging-metadata-release`: contains `release-index.json` and `release-index.evidence.json` generated from the downloaded package artifacts.
- `voyavpn-stable-updater-metadata-release`: contains `latest.json` and `latest.evidence.json` generated from signed updater payloads and `.sig` files.
- `voyavpn-stable-core-staging-metadata-release`: contains `source-core-assets.json`, `core-assets.json`, and `core-assets.evidence.json`; in stable mode, `source-core-assets.json` comes from `VOYAVPN_CORE_ASSETS_JSON`, not from the fixture file. The current stable manifest is expected to contain an empty `assets` array because sing-box is bundled with the app package.
- `voyavpn-stable-final-readiness-release`: contains final readiness output proving the workflow downloaded the package and metadata artifacts and validated them together. This is evidence, not a CDN upload input.

The `*.evidence.json` files include channel, version or core-version summary, first-stable target counts, source artifact names, byte counts, and SHA-256 checksums. Evidence generated from `tests/fixtures` is labeled with `sourceInput.kind: "fixture"` and `sourceInput.nonPublishableFixture: true`; it proves script shape only and must not be used as production stable publication evidence.

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

`scripts/core-assets.mjs` turns fixture input into the stable core asset manifest used as release evidence. The current stable core manifest contains no downloadable core assets. sing-box is bundled with the app package instead of being listed in the core update manifest.

Stable generation requires `--base-url` or `VOYAVPN_CDN_BASE_URL`. If a future approved core asset is added, generated `url` values must be derived from that CDN base URL plus each fixture `path`; fixture download URL fields are not trusted. GitHub URLs are allowed only in `upstreamUrl`, where they record source and license reference material.

Required stable core asset fields if a future release adds an approved downloadable core:

- `coreType`
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

Stable validation fails when any core asset is present without an approved core type, an OS or architecture is outside the first-stable matrix, a checksum or size is invalid, the CDN base URL is an example or GitHub host, or a GitHub URL is supplied as a production download URL.

Fixture generation:

```sh
node scripts/core-assets.mjs --fixture tests/fixtures/release/core-assets.json --out /tmp/voyavpn-core-assets.json --base-url <cdn-base-url>
```

The evidence file defaults to the output filename with `.evidence.json`, for example `/tmp/voyavpn-core-assets.evidence.json`.

## Core Distribution Classes

Keep these release assets separate in manifests, package resources, and evidence:

| Distribution class | Contents | Host or package location | Release gate |
| --- | --- | --- | --- |
| Bundled core seed assets | The approved sing-box seed generated during `pnpm install` or stable build preparation. At runtime it is copied from `core-seeds/sing_box/` into app data `bin/sing_box/` before execution. | Stable package resources only; never `bundle.externalBin`, and never executed from the read-only app bundle. | Requires the core redistribution approval record in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), including source URL, license name, SHA-256, byte size, and source availability evidence for the seed file. |
| App updater payloads | Signed Tauri application update archives, matching `.sig` files, and `latest.json` metadata. They update the VoyaVPN app package, not proxy cores, geo data, or SRS rulesets. | Approved updater CDN base URL from `VOYAVPN_UPDATES_BASE_URL`. | Requires updater key provisioning, signed payload evidence, app artifact checksums, and OS smoke. |

Bundled sing-box seed assets are the only supported acquisition path for the proxy core. They are updated by shipping a new application package, not by the in-app update manager. Downloadable core CDN assets are not published in this rollout; adding any redistributed core requires a separate release profile and legal notice update.

## Updater Metadata

The Tauri updater plugin is registered in `src-tauri/src/lib.rs`. The committed `src-tauri/tauri.conf.json` keeps an empty `plugins.updater` block and keeps `bundle.createUpdaterArtifacts` disabled so local debug builds initialize the plugin without updater credentials or endpoints.

Stable packaging uses the generated overlay from `scripts/tauri-build.mjs`:

- Overlay path: `target/release-config/tauri.updater.stable.generated.json`.
- `bundle.createUpdaterArtifacts`: `true`.
- `plugins.updater.pubkey`: read from `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY`.
- `plugins.updater.endpoints`: `<VOYAVPN_UPDATES_BASE_URL>/latest.json`.
- Windows updater install mode: `passive`.

The overlay generation command is exact and should be run from a prepared shell where release-time environment names have already been supplied by the approved secret system or signing machine:

```sh
export VOYAVPN_RELEASE_CHANNEL=stable
pnpm tauri:stable-updater-config
```

The command writes only `target/release-config/tauri.updater.stable.generated.json`. Do not commit that generated file, copy it into `src-tauri/tauri.conf.json`, or commit private updater keys. Private signing input is supplied through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`; it is required so updater artifacts can be created, but it is not written to the overlay.

Before a real stable release:

1. Generate the updater keypair outside the repo:

   ```sh
   pnpm tauri signer generate --write-keys <secure-private-key-path> --ci
   ```

2. Store only the public key in `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY`; do not commit it into the base config.
3. Store the private key in CI or local release secrets through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`. Store the key password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when one is used.
4. Set the prepared stable environment names described in [runbook.md](runbook.md), including `VOYAVPN_CDN_BASE_URL`, `VOYAVPN_UPDATES_BASE_URL`, `VOYAVPN_UPDATER_PUBLIC_KEY`, diagnostics, updater signing, platform signing, and real artifact input names.
5. Generate and inspect the overlay before packaging with the command above.

6. Run the stable readiness check against the generated overlay:

   ```sh
   pnpm check:release:stable
   ```

7. Build release packages with the same environment. The wrapper passes the generated overlay to `tauri build` through `--config`.
8. Publish the generated update archives, signatures, and one channel metadata document per channel only after external signing and smoke gates pass.
9. Keep app updater metadata separate from geo and ruleset update metadata. Proxy core updates are delivered by application package releases because sing-box is bundled as a seed.

## Core And Sidecar Policy

Debug packages and credential-free release dry runs do not bundle proxy core binaries unless the sing-box seed has been explicitly staged for a local release rehearsal:

- `bundle.externalBin` is an empty list.
- The bundled release resources are `docs/release/THIRD_PARTY_NOTICES.md` and the generated sing-box seed overlay when present.
- Runtime core lookup uses the app data `bin/` tree. sing-box is copied there from the bundled seed.
- GPL and AGPL cores must remain user-supplied or separately approved unless there is explicit legal approval for a distribution path.

Production stable seed redistribution is the only approved packaging exception in this rollout, and it is conditional until the approval record exists:

1. Seed resources may include only sing-box for the current package target.
2. The release owner must record the exact binary name, version, license, source URL, checksum, byte size, source availability evidence, and legal approval before stable publication.
3. Seed resources are copied into app data `bin/<core>/` before runtime discovery; do not execute them from the app bundle.
4. `bundle.externalBin` remains empty for proxy cores so Tauri sidecar resolution does not blur app payloads with core seed assets.
5. AGPL cores and unsupported cores are not bundled or published as first-stable core CDN assets by this rollout.

Optional sidecars for future builds must follow this rule:

1. Add sidecars only through an explicit release profile or platform config overlay.
2. Document the exact binary name, version, license, source URL, checksum, and legal approval.
3. Keep GPL or AGPL sidecars out of default installers unless the approval record explicitly covers sidecar redistribution and source availability.
4. Re-run package builds on every target OS after adding any sidecar because Tauri resolves sidecars by target triple.

## Native Tunnel Packaging

macOS and Windows transparent TUN use native OS components instead of the
desktop UI process owning routes directly.

macOS release builds must stage and sign the PacketTunnel assets before the
final package is signed and notarized:

```sh
pnpm native:macos:libbox
export VOYAVPN_MACOS_APP_BUNDLE="$PWD/target/release/bundle/macos/VoyaVPN.app"
export VOYAVPN_CODESIGN_IDENTITY="<Developer ID or Apple Distribution identity>"
pnpm native:macos:tunnel
pnpm native:macos:tunnel:verify
pnpm native:macos:app:sign
pnpm native:macos:app:notarize
```

`pnpm native:macos:libbox` builds sing-box's Apple `Libbox.xcframework` from the
pinned sing-box source tag and places it at
`src-tauri/native/macos/Frameworks/Libbox.xcframework`. Release owners may
instead provide an already-built framework through `VOYAVPN_LIBBOX_XCFRAMEWORK`.
`VOYAVPN_MACOS_APP_BUNDLE` points the staging, verification, signing, and
notarization helpers at the actual Tauri `.app`; when it is omitted, the scripts
use `target/native/macos/VoyaVPN.app` for local staging only.

The staged assets are:

- `VoyaVPN.app/Contents/MacOS/voyavpn-macos-tunnelctl`
- `VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex`
- `VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex/Contents/Frameworks/Libbox.framework`
  when libbox is available

The containing app uses `src-tauri/entitlements/macos-app.plist`; the extension
uses `src-tauri/entitlements/packet-tunnel.plist`. App Store/TestFlight builds
must provision the matching App Group `group.app.voyavpn.desktop` and Network
Extension entitlement for `packet-tunnel-provider`. Set
`VOYAVPN_REQUIRE_LIBBOX=1` and `VOYAVPN_REQUIRE_CODESIGN=1` in release lanes so
missing libbox or unsigned native tunnel assets fail the build instead of
becoming a runtime-only error.

For notarization, prefer a keychain profile stored on the signing machine:

```sh
xcrun notarytool store-credentials "<profile-name>"
export VOYAVPN_NOTARY_KEYCHAIN_PROFILE="<profile-name>"
```

The script also supports `VOYAVPN_NOTARY_APPLE_ID`,
`VOYAVPN_NOTARY_TEAM_ID`, and `VOYAVPN_NOTARY_PASSWORD`. These values must come
from the release secret system and must not be committed.

Windows release builds must build and install the service on smoke machines:

```sh
pnpm native:windows:tunnel:build
pnpm native:windows:tunnel:install
pnpm native:windows:tunnel:status
```

`VoyaVPNTunnelService` runs `sing-box check -c` before launching sing-box with
Wintun. Install and uninstall commands must run from an elevated Windows
terminal or an installer custom action with equivalent service-management
rights.

## First-Run Core Acquisition Flow

First run should not assume any core executable is present in debug or dry-run packages. Stable packages copy the approved sing-box seed into app data when present. Missing sing-box seed assets do not fall back to an online sing-box download; rebuild or reinstall the package so the seed is present.

1. App startup creates the app config, `bin/`, `binConfigs/`, log, backup, and temp directories.
2. The profile table and status bar may show profiles before cores exist, but connect should surface a typed missing-core error instead of failing silently.
3. On startup or connect, the approved sing-box seed is copied from package resources into app data only when `bin/sing_box/` is missing.
4. The user opens Check Updates for app, geo, and SRS updates. The update manager fetches through proxy first when available, then falls back to direct download.
5. Core launch discovery uses the app data `bin/sing_box/` tree populated from the bundled seed.
6. Geo files and sing-box rulesets are acquired separately from the app updater so ruleset refreshes do not require an app release.

## Attribution And Licenses

The bundled attribution document is `docs/release/THIRD_PARTY_NOTICES.md`. It records the app license, Tauri and frontend framework licenses, stable seed redistribution scope, upstream source URLs, license names, checksum expectations, and source availability expectations for sing-box.

Keep this document current whenever a runtime core, bundled seed asset, core update CDN asset, or release dependency changes. Notices must not imply that unsupported cores are bundled, and they must not claim GPL or AGPL redistribution approval until the stable approval checkpoint is attached to the release evidence.

## Manual Release Checks Not Run By This Batch

The automated runner does not have signing certificates, notarization credentials, updater private keys, package repository credentials, or real Windows/Linux/macOS smoke machines. Capture those checks manually before publication:

- macOS Developer ID sign, notarize with `notarytool`, staple, install `.dmg`, launch, and verify TUN/elevation prompts.
- Windows Authenticode sign NSIS/MSI, install as current user, launch, uninstall, and verify WebView2 bootstrap behavior.
- Linux install `.deb`, `.rpm`, and `.AppImage` on clean distributions, verify desktop entry, execute bit, and bundled core seed path.
- Publish updater metadata to the beta channel and verify that an older signed build sees the update.
