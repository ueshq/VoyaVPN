# Release CI Secrets

Batch: `08-02-release-ci`

## Workflow Entry Point

The release workflow lives at `.github/workflows/release.yml` and is triggered with `workflow_dispatch`.

Default validation inputs are intentionally credential-free:

- `channel`: `beta`
- `build_profile`: `debug`
- `dry_run`: `true`
- `updater_metadata`: `true`

With those defaults the workflow runs the normal CI checks, builds unsigned/debug Tauri packages across the six first-stable target entries, normalizes artifact names, writes `SHA256SUMS`, uploads workflow artifacts, generates CDN staging metadata, generates core staging metadata from the repository fixture, and generates dry-run updater metadata with explicit non-publishable signatures.

Dry-run updater metadata is not publishable. It exists only to validate the `latest.json` generation path before updater signing keys and upload locations are provisioned. The workflow rejects `channel=stable` when `dry_run=true`, and stable dispatches must use `build_profile=release`.

## Target Matrix

The workflow target names are the stable metadata names consumed by `scripts/release-index.mjs` and `scripts/release-updater-metadata.mjs`.

| Target name | Runner label | Rust target |
| --- | --- | --- |
| `darwin-x86_64` | `macos-15-intel` | `x86_64-apple-darwin` |
| `darwin-aarch64` | `macos-15` | `aarch64-apple-darwin` |
| `windows-x86_64` | `windows-2025` | `x86_64-pc-windows-msvc` |
| `windows-aarch64` | `windows-11-arm` | `aarch64-pc-windows-msvc` |
| `linux-x86_64` | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` |
| `linux-aarch64` | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` |

Runner limitation: arm64 hosted runner labels must be enabled for the repository or organization plan. If GitHub-hosted arm64 capacity is unavailable, release owners must run an equivalent self-hosted runner with the same Rust target and keep the artifact target name unchanged.

## Artifact Outputs

Each package job uploads an artifact named:

```text
voyavpn-<channel>-<tauri-target>-<build-profile>
```

The uploaded directory contains:

- Normalized package files named `voyavpn-<version>-<channel>-<tauri-target>-<kind>.<ext>`.
- `SHA256SUMS` with SHA-256 checksums for every normalized package file.
- `artifact-manifest.json` with original bundle paths, normalized names, sizes, and hashes.

The updater metadata job uploads:

```text
voyavpn-<channel>-updater-metadata-<build-profile>
```

That artifact contains `latest.json` and `latest.evidence.json`.

The CDN staging metadata job uploads:

```text
voyavpn-<channel>-cdn-staging-metadata-<build-profile>
```

That artifact contains `release-index.json` and `release-index.evidence.json`. It is CI evidence only. The workflow has no CDN upload, cache purge, cloud-console, or publication step.

The core staging metadata job uploads:

```text
voyavpn-<channel>-core-staging-metadata-<build-profile>
```

That artifact contains `source-core-assets.json`, `core-assets.json`, and `core-assets.evidence.json`. Stable runs require `VOYAVPN_CORE_ASSETS_JSON`; dry-run or non-stable validation copies the repository fixture into `source-core-assets.json` so the script path can be exercised without publication.

Stable non-dry-run workflows also upload:

```text
voyavpn-<channel>-final-readiness-<build-profile>
```

That artifact contains the final readiness output generated after the package matrix, updater metadata, CDN staging metadata, and core staging metadata jobs complete.

## Secrets Referenced By The Workflow

These names are placeholders for GitHub Actions repository or environment secrets. The values must never be committed.

| Secret | Required for dry run | Purpose |
| --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | No | Tauri updater signing private key. Required when `dry_run` is `false` and real updater metadata is generated unless `TAURI_SIGNING_PRIVATE_KEY_PATH` is used. |
| `TAURI_SIGNING_PRIVATE_KEY_PATH` | No | Path to the updater signing private key on a prepared release machine or runner. Required when `dry_run` is `false` unless `TAURI_SIGNING_PRIVATE_KEY` is used. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | No | Password for the updater private key when the key was generated with one. |
| `APPLE_CERTIFICATE` | No | Future macOS signing certificate payload for release jobs that import a Developer ID certificate. |
| `APPLE_CERTIFICATE_PASSWORD` | No | Password for `APPLE_CERTIFICATE`. |
| `APPLE_ID` | No | Apple account used for notarization in manual or future automated release steps. |
| `APPLE_PASSWORD` | No | App-specific Apple password for notarization. |
| `APPLE_TEAM_ID` | No | Apple developer team identifier for notarization. |
| `WINDOWS_CERTIFICATE_BASE64` | No | Future Windows Authenticode certificate payload. |
| `WINDOWS_CERTIFICATE_PASSWORD` | No | Password for `WINDOWS_CERTIFICATE_BASE64`. |

These GitHub Actions variables are public configuration, not private signing material:

| Variable | Required for dry run | Purpose |
| --- | --- | --- |
| `VOYAVPN_CDN_BASE_URL` | No | Approved stable CDN base URL for manual downloads, release index entries, core assets, and staging evidence. Required for stable. |
| `VOYAVPN_UPDATES_BASE_URL` | No | Approved updater CDN base URL. Required for stable. Non-stable dry runs fall back to a `.test` URL. |
| `VOYAVPN_UPDATER_PUBLIC_KEY` | No | Approved Tauri updater public key used by the generated stable overlay. Required for stable. |
| `VOYAVPN_DIAGNOSTICS_ENDPOINT` | No | Approved HTTPS diagnostics ingest endpoint. Required for stable and validated without printing the value. |
| `VOYAVPN_CORE_ASSETS_JSON` | No | Stable core asset source JSON used to generate core staging metadata. Required for stable final readiness; must contain production core asset paths, checksums, sizes, licenses, and upstream source references. |

## Non-Dry-Run Expectations

Setting `dry_run` to `false` enables stricter validation:

1. `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH` must be present.
2. `scripts/release-updater-metadata.mjs` requires real signed updater payloads and matching `.sig` files.
3. Non-publishable dry-run updater signatures are disabled.
4. For `channel=stable`, `VOYAVPN_CDN_BASE_URL`, `VOYAVPN_UPDATES_BASE_URL`, `VOYAVPN_UPDATER_PUBLIC_KEY`, `VOYAVPN_DIAGNOSTICS_ENDPOINT`, Apple signing/notarization inputs, and Windows signing inputs must be present and non-placeholder.
5. For `channel=stable`, `VOYAVPN_CORE_ASSETS_JSON` must be present so core staging metadata is generated from explicit release input instead of `tests/fixtures`.
6. The workflow generates the stable updater overlay during preflight, builds the package matrix, generates CDN/updater/core metadata artifacts, then runs `scripts/check-release-readiness.mjs --mode stable` in the final readiness job against downloaded workflow artifacts and generated metadata.

The current repository keeps `bundle.createUpdaterArtifacts` disabled by default in `src-tauri/tauri.conf.json`, so stable packages use the generated overlay at `target/release-config/tauri.updater.stable.generated.json`. The overlay contains only the public updater key and updater CDN endpoint; private signing material stays in the approved secret system or release machine.

## Prepared Stable Environment Preflight

The local stable preflight is for a prepared release shell, not an ordinary developer shell. The approved secret system, release artifact handoff, or workflow final-readiness job must provide the required names before the commands run; this doc records names only and never records values.

Required variable and secret names:

- `VOYAVPN_RELEASE_CHANNEL`
- `VOYAVPN_CDN_BASE_URL`
- `VOYAVPN_UPDATES_BASE_URL`
- `VOYAVPN_UPDATER_PUBLIC_KEY`
- `VOYAVPN_DIAGNOSTICS_ENDPOINT`
- `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when the updater key requires one
- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`
- `WINDOWS_CERTIFICATE_BASE64`
- `WINDOWS_CERTIFICATE_PASSWORD`
- `VOYAVPN_RELEASE_ARTIFACTS_DIR`, `VOYAVPN_SIGNED_UPDATER_DIR`, and `VOYAVPN_CORE_ASSETS_FILE` when the prepared shell does not use the default stable artifact paths

Prepared release shell sequence:

```sh
export VOYAVPN_RELEASE_CHANNEL=stable
pnpm tauri:stable-updater-config
pnpm check:release:stable
```

Expected unprepared-shell failures include missing `VOYAVPN_CDN_BASE_URL`, missing `VOYAVPN_UPDATES_BASE_URL`, missing `VOYAVPN_UPDATER_PUBLIC_KEY`, missing `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, missing diagnostics or platform signing inputs, missing real stable artifacts, stable checks pointed at fixtures, placeholder updater signatures, or forbidden production URLs. These failures are not repository blockers when they occur in a local shell that has not been provisioned with external production inputs.

Expected prepared-environment pass criteria: `pnpm tauri:stable-updater-config` generates `target/release-config/tauri.updater.stable.generated.json`, the overlay enables updater artifacts with the approved public key and updater CDN endpoint, and `pnpm check:release:stable` exits successfully before any stable pointer promotion.

## What Must Stay Out Of Git

Do not commit:

- Updater private keys or passwords.
- Apple certificates, app-specific passwords, or notarization credentials.
- Windows signing certificates, passwords, or hardware-token exports.
- Package repository tokens.
- Published `latest.json` files that contain real production URLs until the release owner has approved the channel.
- GPL or AGPL proxy core binaries in installer payloads unless a separate legal approval is documented.

## Local Script Checks

The scripts can be exercised without secrets:

```sh
node scripts/release-artifacts.mjs --input target/debug/bundle --output dist/release/local --target local-debug --channel beta --allow-empty
node scripts/release-updater-metadata.mjs --input dist/release --out dist/updater/latest.json --target darwin-aarch64,darwin-x86_64,linux-aarch64,linux-x86_64,windows-aarch64,windows-x86_64 --placeholder-signatures
node scripts/core-assets.mjs --fixture tests/fixtures/release/core-assets.json --out dist/core-staging/core-assets.json --base-url https://cdn.voyavpn.test/beta --channel beta
```

Remove `--allow-empty` and `--placeholder-signatures` for real release validation. For stable, generate the overlay first with `pnpm tauri:stable-updater-config`, then run `pnpm check:release:stable`.
