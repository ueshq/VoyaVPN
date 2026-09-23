# VoyaVPN Packaging Runbook

Batch: `08-01-tauri-packaging`

## Current Package Configuration

Tauri packaging is configured in `apps/desktop/src-tauri/tauri.conf.json` for the public beta bundle matrix:

| Platform | Bundle targets | Signing posture |
| --- | --- | --- |
| macOS | `.app`, `.dmg` | `hardenedRuntime` is enabled, but no signing identity is configured in the repo. Developer ID signing and notarization are manual release steps. |
| Windows | NSIS, MSI | NSIS defaults to current-user install. MSI has a pinned upgrade code: `81f9b48c-cd6b-566b-9904-9f89ac741525`. Authenticode signing is a manual release step. |
| Linux | `.deb`, `.rpm`, `.AppImage` | Package metadata is configured, but repository publication and checksum signing are manual release steps. |

The required local debug build is intentionally unsigned:

```sh
pnpm tauri:build --debug
```

The package script runs through `scripts/tauri/cli.mjs`, which forwards all Tauri CLI arguments and normalizes `CI=1`/`CI=0` to the boolean strings required by the Tauri 2 CLI. This keeps local and runner debug packaging deterministic without requiring signing credentials.

`bundle.createUpdaterArtifacts` stays `false` in the committed config by design. The base `apps/desktop/src-tauri/tauri.conf.json` is credential-free and safe for local debug builds, CI dry runs, and code review because it does not contain updater endpoints, updater public keys, private-key paths, or generated release state. Stable release jobs do not edit the committed config; `scripts/tauri/cli.mjs` writes a generated overlay at `target/release-config/tauri.updater.stable.generated.json` when `VOYAVPN_RELEASE_CHANNEL=stable` or `VOYAVPN_TAURI_UPDATER_CONFIG=stable`.

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

The six package artifacts named `voyavpn-stable-<release_target>-release` are the app package inputs for manual CDN staging. Each package artifact contains the normalized package files, `SHA256SUMS`, and `artifact-manifest.json`; the `index` and `updater` release subcommands use those manifests as source evidence.

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

`pnpm release -- index` turns one or more `artifact-manifest.json` files from `pnpm release -- artifacts` into the manual-download CDN release index and a sibling evidence JSON file.

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
pnpm release -- index --input tests/fixtures/release/artifacts --out /tmp/voyavpn-release-index.json --base-url <cdn-base-url> --channel stable
```

The evidence file defaults to the output filename with `.evidence.json`, for example `/tmp/voyavpn-release-index.evidence.json`.

## Core Asset Manifest

`pnpm release -- core-assets` turns fixture input into the stable core asset manifest used as release evidence. The current stable core manifest contains no downloadable core assets. sing-box is bundled with the app package instead of being listed in the core update manifest.

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
pnpm release -- core-assets --fixture tests/fixtures/release/core-assets.json --out /tmp/voyavpn-core-assets.json --base-url <cdn-base-url>
```

The evidence file defaults to the output filename with `.evidence.json`, for example `/tmp/voyavpn-core-assets.evidence.json`.

## Core Distribution Classes

Keep these release assets separate in manifests, package resources, and evidence:

| Distribution class | Contents | Host or package location | Release gate |
| --- | --- | --- | --- |
| Bundled core seed assets | The approved sing-box seed generated during `pnpm install` or stable build preparation, bundled into every package. Windows and Linux copy it from `core-seeds/sing_box/` into app data `bin/sing_box/` before execution. On macOS the PacketTunnel extension (Libbox) runs the connection; the seed only backs speedtests while disconnected (connected tests go through the PacketTunnel's Clash API) and runs in place from the signed bundle. | Package resources only; never `bundle.externalBin`. `pnpm native:macos:tunnel:verify` fails a macOS app whose `Contents/Resources/core-seeds/sing_box/sing-box` is missing or unsigned. | Requires the core redistribution approval record in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), including source URL, license name, SHA-256, byte size, and source availability evidence for the seed file. |
| App updater payloads | Signed Tauri application update archives, matching `.sig` files, and `latest.json` metadata. They update the VoyaVPN app package, not proxy cores, geo data, or SRS rulesets. | Approved updater CDN base URL from `VOYAVPN_UPDATES_BASE_URL`. | Requires updater key provisioning, signed payload evidence, app artifact checksums, and OS smoke. |

Bundled sing-box seed assets are the only supported acquisition path for the proxy core. They are updated by shipping a new application package, not by the in-app update manager. Downloadable core CDN assets are not published in this rollout; adding any redistributed core requires a separate release profile and legal notice update.

## Updater Metadata

The Tauri updater plugin is registered in `apps/desktop/src-tauri/src/lib.rs`. The committed `apps/desktop/src-tauri/tauri.conf.json` keeps an empty `plugins.updater` block and keeps `bundle.createUpdaterArtifacts` disabled so local debug builds initialize the plugin without updater credentials or endpoints.

Stable packaging uses the generated overlay from `scripts/tauri/cli.mjs`:

- Overlay path: `target/release-config/tauri.updater.stable.generated.json`.
- `bundle.createUpdaterArtifacts`: `true`.
- `plugins.updater.pubkey`: read from `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY`.
- `plugins.updater.endpoints`: `<VOYAVPN_UPDATES_BASE_URL>/latest.json`.
- Windows updater install mode: `passive`.

### Which File The Updater Serves

`bundle.createUpdaterArtifacts` is the plain `true`, not `"v1Compatible"`, so
Tauri 2 signs the installers **in place** rather than emitting zipped v1
payloads. Only macOS produces a separate archive. The updater payload per target
is therefore:

| Target | Updater payload | Sibling signature |
| --- | --- | --- |
| `darwin-x86_64`, `darwin-aarch64` | `bundle/macos/VoyaVPN.app.tar.gz` | `VoyaVPN.app.tar.gz.sig` |
| `windows-x86_64`, `windows-aarch64` | `bundle/nsis/*-setup.exe` | `*-setup.exe.sig` |
| `linux-x86_64`, `linux-aarch64` | `bundle/appimage/*.AppImage` | `*.AppImage.sig` |

Windows also signs the MSI and Linux also signs `.deb`/`.rpm`, so the payload is
not inferable from "has a signature": `pnpm release -- artifacts` picks the
designated installer above, records it as `updaterPayload: true` in
`artifact-manifest.json` (with `updaterSignature: true` on its `.sig` and
`updaterPayloadSource` on the manifest), and fails a stable collection when the
bundle contains no signed payload for its target. `pnpm release -- updater` and
`pnpm release -- readiness` require that explicit flag. Manifests without
it must be regenerated with the current artifact collector.
Normalized signature artifacts are named `<payload>.sig`.

The overlay generation command is exact and should be run from a prepared shell where release-time environment names have already been supplied by the approved secret system or signing machine:

```sh
export VOYAVPN_RELEASE_CHANNEL=stable
pnpm release -- updater-config
```

The command writes only `target/release-config/tauri.updater.stable.generated.json`. Do not commit that generated file, copy it into `apps/desktop/src-tauri/tauri.conf.json`, or commit private updater keys. Private signing input is supplied through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`; it is required so updater artifacts can be created, but it is not written to the overlay.

Before a real stable release:

1. Generate the updater keypair outside the repo:

   ```sh
   pnpm tauri signer generate --write-keys <secure-private-key-path> --ci
   ```

2. Store only the public key in `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY`; do not commit it into the base config.
3. Store the private key in CI or local release secrets through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`. Store the key password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when one is used.
4. Set the prepared stable environment names described in [runbook.md](runbook.md), including `VOYAVPN_CDN_BASE_URL`, `VOYAVPN_UPDATES_BASE_URL`, `VOYAVPN_UPDATER_PUBLIC_KEY`, updater signing, platform signing, and real artifact input names.
5. Generate and inspect the overlay before packaging with the command above.

6. Run the stable readiness check against the generated overlay (stable mode
   defaults `--tauri-config` to that overlay):

   ```sh
   pnpm release -- readiness --mode stable
   ```

7. Build release packages with the same environment. The wrapper passes the generated overlay to `tauri build` through `--config`.
8. Publish the generated update archives, signatures, and one channel metadata document per channel only after external signing and smoke gates pass.
9. Keep app updater metadata separate from geo and ruleset update metadata. Proxy core updates are delivered by application package releases because sing-box is bundled as a seed.

## Core And Sidecar Policy

Every `pnpm tauri:build` bundles the sing-box seed, including `--debug` builds and credential-free dry runs: the wrapper stages the seed for the target platform before invoking Tauri and injects a `bundle.resources` overlay for it. Treat any locally built package as a GPL redistribution:

- `bundle.externalBin` is an empty list.
- The bundled release resources are `docs/release/THIRD_PARTY_NOTICES.md` and the generated sing-box seed overlay.
- The seed archive is verified against the SHA-256 pinned in `scripts/core/sing-box-installer.mjs` before extraction, and the already-staged seed is re-verified against `sing-box.seed.json` before it is bundled. See [sing-box-seed-pinning.md](sing-box-seed-pinning.md) for the bump procedure and the `VOYAVPN_ALLOW_UNPINNED_SING_BOX` escape hatch.
- Windows and Linux runtime core lookup uses the app data `bin/` tree. sing-box is copied there from the bundled seed. macOS runs the seed from the signed app bundle.
- GPL and AGPL cores must remain user-supplied or separately approved unless there is explicit legal approval for a distribution path.

Production stable seed redistribution is the only approved packaging exception in this rollout, and it is conditional until the approval record exists:

1. Seed resources may include only sing-box for the current package target.
2. The release owner must record the exact binary name, version, license, source URL, checksum, byte size, source availability evidence, and legal approval before stable publication.
3. On Windows and Linux, seed resources are copied into app data `bin/<core>/` before runtime discovery. On macOS they are executed from the app bundle, since a copy would lose the bundle's code-signing context.
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
final package is signed. There are two separate distribution lanes.

Developer ID lane for direct DMG / drag-to-Applications testing:

```sh
pnpm native:macos:libbox
pnpm tauri:build --bundles app
export VOYAVPN_MACOS_APP_BUNDLE="$PWD/target/release/bundle/macos/VoyaVPN.app"
export VOYAVPN_CODESIGN_IDENTITY="<Developer ID Application identity>"
export VOYAVPN_PROVISIONING_PROFILE_DIR="<profile-dir-for-Developer-ID>"
export VOYAVPN_REQUIRE_PROVISIONING=1
export VOYAVPN_REQUIRE_CODESIGN=1
export VOYAVPN_REQUIRE_NOTARIZATION_READY=1
pnpm native:macos:tunnel
pnpm native:macos:tunnel:verify
pnpm native:macos:app:sign
pnpm native:macos:tunnel:verify
pnpm native:macos:dmg
pnpm native:macos:app:notarize
```

App Store/TestFlight lane:

```sh
pnpm native:macos:libbox
pnpm tauri:build --bundles app
export VOYAVPN_MACOS_APP_BUNDLE="$PWD/target/release/bundle/macos/VoyaVPN.app"
export VOYAVPN_CODESIGN_IDENTITY="<3rd Party Mac Developer Application or Apple Distribution identity>"
export VOYAVPN_MACOS_DISTRIBUTION=app-store
export VOYAVPN_PROVISIONING_PROFILE_DIR="<profile-dir-for-App-Store-or-TestFlight>"
export VOYAVPN_REQUIRE_PROVISIONING=1
pnpm native:macos:tunnel
pnpm native:macos:tunnel:verify
pnpm native:macos:app:sign
pnpm native:macos:dmg
```

App Store/TestFlight artifacts are submitted through App Store Connect. They are
not expected to pass `spctl` or launch by being copied directly into
`/Applications`; direct distribution requires the Developer ID lane plus
notarization and stapling. Developer ID artifacts are expected to pass
Gatekeeper assessment after `pnpm native:macos:app:notarize` has accepted and
stapled the app.

`pnpm native:macos:libbox` builds sing-box's Apple output from the pinned source
tag, verifies the arm64/x86_64 macOS architectures, and places only the universal
`apps/desktop/src-tauri/native/macos/Frameworks/Libbox.framework`. Release owners may
instead provide an already-built framework through `VOYAVPN_LIBBOX_FRAMEWORK`.
`VOYAVPN_MACOS_APP_BUNDLE` points the staging, verification, signing, and
notarization helpers at the actual Tauri `.app`; when it is omitted, the scripts
use `target/native/macos/VoyaVPN.app` for local staging only.
For App Store/TestFlight builds, set `VOYAVPN_PROVISIONING_PROFILE_DIR` or the
more specific `VOYAVPN_MACOS_APP_PROVISIONING_PROFILE` and
`VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE` paths. The scripts embed the
profiles and derive `com.apple.application-identifier`,
`com.apple.developer.team-identifier`, and keychain access group entitlements
from them.

Use `pnpm tauri:build --bundles app` for macOS native tunnel lanes. The
PacketTunnel extension is staged after Tauri creates the `.app`; `pnpm
native:macos:dmg` then creates the DMG from that final signed bundle and mounts
the image to verify the embedded PacketTunnel before the artifact is accepted.
Set `VOYAVPN_NOTARY_ARTIFACT` to the generated DMG before
`pnpm native:macos:app:notarize` when the release artifact itself should be
submitted and stapled.

The staged PacketTunnel provider depends on the macOS distribution lane:

- Developer ID direct distribution:
  `VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension`
- App Store/TestFlight and unsigned development:
  `VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex`
- `Contents/Frameworks/Libbox.framework` under the selected provider bundle
  only when the selected Libbox slice is dynamic.

The macOS app controls `NETunnelProviderManager` in-process; the production path
contains only the app and its PacketTunnel provider.

After launching local macOS bundles that contain the PacketTunnel provider,
check registration health with `pnpm native:macos:ne:doctor`. Stale PlugInKit
registrations can make app-extension builds start an old appex with the same
bundle id, while Developer ID System Extension builds must show an activated
entry in `systemextensionsctl list`. See
`docs/release/macos-networkextension-troubleshooting.md` for detection and
repair steps.

If the selected `Libbox.framework` slice is static, `pnpm native:macos:tunnel`
links the required Libbox symbols into `VoyaPacketTunnel` and does not embed a
framework in the extension bundle. `pnpm native:macos:tunnel:verify` accepts
either static symbols or an embedded dynamic framework when
`VOYAVPN_REQUIRE_LIBBOX=1` is set.

The containing app uses `apps/desktop/src-tauri/entitlements/macos-app.plist`;
the provider uses `apps/desktop/src-tauri/entitlements/packet-tunnel.plist`.
Developer ID direct builds must provision `packet-tunnel-provider-systemextension`
and package PacketTunnel as `.systemextension`; signing that entitlement into an
`.appex` causes macOS to reject the provider before `startTunnel` runs. App
Store/TestFlight builds must provision the matching App Group
`group.app.voyavpn.desktop` and Network Extension entitlement for
`packet-tunnel-provider`. Set
`VOYAVPN_REQUIRE_LIBBOX=1`, `VOYAVPN_REQUIRE_CODESIGN=1`, and
`VOYAVPN_REQUIRE_PROVISIONING=1` in release lanes so missing libbox, unsigned
native tunnel assets, or missing profiles fail the build instead of becoming a
runtime-only error.

For notarization, prefer a keychain profile stored on the signing machine:

```sh
xcrun notarytool store-credentials "<profile-name>"
export VOYAVPN_NOTARY_KEYCHAIN_PROFILE="<profile-name>"
# Optional when the profile was stored in a specific keychain:
export VOYAVPN_NOTARY_KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"
```

The script also supports `VOYAVPN_NOTARY_APPLE_ID`,
`VOYAVPN_NOTARY_TEAM_ID`, and `VOYAVPN_NOTARY_PASSWORD`. These values must come
from the release secret system and must not be committed.

For an unsigned, release-profile local Windows package with a working TUN
service, run this command from a normal PowerShell. It generates and silently
installs the current-user NSIS copy, then requests elevation only once to update
the service. The local lane explicitly targets the machine's native Windows
MSVC Rust triple and writes bundles below `target/<rust-target>/release/bundle`:

```powershell
pnpm build:windows:local
```

The local lane refuses to replace an MSI or a VoyaVPN install outside
`%LOCALAPPDATA%\VoyaVPN`. The service binary is copied out of the user-writable
`target/` tree into
`%ProgramFiles%\VoyaVPN\voyavpn-tunnel-service.exe`, then registered as a
demand-start service. See
[windows-local-tun-testing.md](windows-local-tun-testing.md) for prerequisites,
verification, and teardown.

The lower-level service helpers remain available. `install` and `uninstall`
must run from an elevated Windows terminal:

```powershell
pnpm native:windows:tunnel:build
pnpm native:windows:tunnel:install
pnpm native:windows:tunnel:status
pnpm native:windows:tunnel:uninstall
```

`VoyaVPNTunnelService` runs `sing-box check -c` before launching sing-box with
Wintun. The local workflow leaves it stopped until the client supplies a
runtime configuration and enables TUN.

## First-Run Core Acquisition Flow

On Windows and Linux, first run copies the bundled sing-box seed into app data; macOS runs it from the bundle and copies nothing. There is no download-on-first-run path in the app: a package that was built without a staged seed simply has no core. Missing sing-box seed assets do not fall back to an online sing-box download; rebuild or reinstall the package so the seed is present.

1. App startup creates the app config, `bin/`, `binConfigs/`, log, and temp directories.
2. The profile table may show profiles before cores exist, but connect should surface a typed missing-core error instead of failing silently.
3. On startup or connect, the approved sing-box seed is copied from package resources into app data only when `bin/sing_box/` is missing.
4. The user opens Check Updates for app, geo, and SRS updates. The update manager fetches through proxy first when available, then falls back to direct download.
5. Core launch discovery uses the app data `bin/sing_box/` tree populated from the bundled seed.
6. Geo files and sing-box rulesets are acquired separately from the app updater so ruleset refreshes do not require an app release.

## Package Size

What keeps the packages small, and what to check before undoing any of it:

- **Release profile** (root `Cargo.toml`): `opt-level = "z"`, fat LTO, one codegen unit, `strip = "symbols"`. Measured on macOS arm64, the `voyavpn` binary is 11.7 MB against 34.6 MB with the default profile. `panic` stays `unwind` so a panicking task cannot skip the shutdown that restores the system proxy/TUN. Release backtraces carry no symbol names; there is no crash symbolication pipeline to feed.
- **No development binaries in the bundle.** Tauri bundles every `bin` target, so the IPC binding exporter is a cargo example (`cargo run -p voyavpn --example export-bindings`). `verify-tunnel.mjs` fails a macOS app that still contains `export-bindings`.
- **The macOS seed is deliberate** (see Core And Sidecar Policy above): it duplicates the Go runtime in the PacketTunnel extension (~50 MB), but it is what lets a disconnected app run latency tests. See ADR 0004.
- **PacketTunnel extension** links with `-dead_strip` and is `strip -x`ed before signing (68 MB to 52 MB). Only the Libbox entry points the provider calls survive; `verify-tunnel.mjs` checks those.
- **DMG** uses `ULMO` (LZMA), readable from macOS 10.15, the app's minimum.
- **Frontend**: country flags load only the 4x3 artwork of two-letter codes, as separate files rather than inlined into CSS or JS; `removeUnusedCommands` drops plugin commands the capabilities never allow.
- `pnpm size:report` prints the current sizes of the built binaries and bundles.


## Attribution And Licenses

The bundled attribution document is `docs/release/THIRD_PARTY_NOTICES.md`. It records the app license, Tauri and frontend framework licenses, stable seed redistribution scope, upstream source URLs, license names, checksum expectations, and source availability expectations for sing-box.

Keep this document current whenever a runtime core, bundled seed asset, core update CDN asset, or release dependency changes. Notices must not imply that unsupported cores are bundled, and they must not claim GPL or AGPL redistribution approval until the stable approval checkpoint is attached to the release evidence.

## Manual Release Checks Not Run By This Batch

The automated runner does not have signing certificates, notarization credentials, updater private keys, package repository credentials, or real Windows/Linux/macOS smoke machines. Capture those checks manually before publication:

- macOS Developer ID sign, notarize with `notarytool`, staple, install `.dmg`, launch, and verify TUN/elevation prompts.
- Windows Authenticode sign NSIS/MSI, install as current user, launch, uninstall, and verify WebView2 bootstrap behavior.
- Linux install `.deb`, `.rpm`, and `.AppImage` on clean distributions, verify desktop entry, execute bit, and bundled core seed path.
- Publish updater metadata to the beta channel and verify that an older signed build sees the update.
