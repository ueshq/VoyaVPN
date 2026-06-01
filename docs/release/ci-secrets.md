# Release CI Secrets

Batch: `08-02-release-ci`

## Workflow Entry Point

The release workflow lives at `.github/workflows/release.yml` and is triggered with `workflow_dispatch`.

Default validation inputs are intentionally credential-free:

- `channel`: `beta`
- `build_profile`: `debug`
- `dry_run`: `true`
- `updater_metadata`: `true`

With those defaults the workflow runs the normal CI checks, builds unsigned/debug Tauri packages on macOS, Windows, and Linux runners, normalizes artifact names, writes `SHA256SUMS`, uploads workflow artifacts, and generates dry-run updater metadata with explicit placeholder signatures.

Dry-run updater metadata is not publishable. It exists only to validate the `latest.json` generation path before updater signing keys and upload locations are provisioned.

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

## Secrets Referenced By The Workflow

These names are placeholders for GitHub Actions repository or environment secrets. The values must never be committed.

| Secret | Required for dry run | Purpose |
| --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | No | Tauri updater signing private key. Required when `dry_run` is `false` and real updater metadata is generated. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | No | Password for the updater private key when the key was generated with one. |
| `APPLE_CERTIFICATE` | No | Future macOS signing certificate payload for release jobs that import a Developer ID certificate. |
| `APPLE_CERTIFICATE_PASSWORD` | No | Password for `APPLE_CERTIFICATE`. |
| `APPLE_ID` | No | Apple account used for notarization in manual or future automated release steps. |
| `APPLE_PASSWORD` | No | App-specific Apple password for notarization. |
| `APPLE_TEAM_ID` | No | Apple developer team identifier for notarization. |
| `WINDOWS_CERTIFICATE_BASE64` | No | Future Windows Authenticode certificate payload. |
| `WINDOWS_CERTIFICATE_PASSWORD` | No | Password for `WINDOWS_CERTIFICATE_BASE64`. |

`VOYAVPN_UPDATES_BASE_URL` is read as a GitHub Actions variable, not a secret. When unset, the workflow uses `https://updates.voyavpn.example/<channel>` so dry-run metadata remains deterministic and clearly non-production.

## Non-Dry-Run Expectations

Setting `dry_run` to `false` enables stricter validation:

1. `TAURI_SIGNING_PRIVATE_KEY` must be present.
2. `scripts/release-updater-metadata.mjs` requires real signed updater payloads and matching `.sig` files.
3. Placeholder updater signatures are disabled.

The current repository keeps `bundle.createUpdaterArtifacts` disabled by default in `src-tauri/tauri.conf.json`, so a real public beta release still needs a controlled signing step or config overlay that creates updater artifacts without committing private keys.

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
node scripts/release-updater-metadata.mjs --input dist/release --out dist/updater/latest.json --target darwin-x86_64,windows-x86_64,linux-x86_64 --placeholder-signatures
```

Remove `--allow-empty` and `--placeholder-signatures` for real release validation.
