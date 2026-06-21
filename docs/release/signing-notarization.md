# Signing, Notarization, And Updater Keys

Batch: `08-03-release-runbooks`

This document records the manual credentialed steps required before public beta or stable publication. Do not commit private keys, certificates, passwords, app-specific passwords, token exports, or production updater metadata.

## Evidence Links

- Top-level release path: [runbook.md](runbook.md)
- Secret names used by CI: [ci-secrets.md](ci-secrets.md)
- Packaging posture and updater placeholders: [packaging.md](packaging.md)
- Rollback procedures: [rollback.md](rollback.md)

## Credential Checkpoints

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Apple signing credentials | macOS release owner | Apple Developer account, secure local keychain, or future GitHub secret import | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` are supplied only through approved secure storage. `security find-identity -v -p codesigning` shows the expected Developer ID identity on the signing machine. | Remove the certificate from the runner or keychain, revoke exposed credentials in Apple Developer, and rebuild from clean credentials. |
| Windows signing credentials | Windows release owner | Authenticode certificate, hardware token, cloud signer, or future GitHub secret import | `WINDOWS_CERTIFICATE_BASE64` and `WINDOWS_CERTIFICATE_PASSWORD` are supplied only through approved secure storage or the external signer. A test file signature verifies with Windows trust tooling. | Revoke or rotate the certificate if exposed. Remove signed artifacts from staging and regenerate packages. |
| Tauri updater private key | Security owner | Offline key storage, CI secret, or local release machine | `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH` is available to the signing step; `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is set only when the key requires it. No private key material is printed. | Rotate the updater keypair, restore the previous trusted public key, and publish rollback metadata. |
| Tauri updater public key | Release engineer | `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY` | The generated stable overlay contains the approved public key, while `src-tauri/tauri.conf.json` remains credential-free. | Regenerate the overlay with the previous approved public key or disable update metadata for the affected channel. |
| CDN release base URL | Release owner | GitHub Actions variable `VOYAVPN_CDN_BASE_URL` and stable CDN host | Generated `release-index.json`, core manifests, and staging evidence derive stable URLs from the approved CDN base URL. | Restore the previous release-index pointer and hold new app/core assets outside public paths. |
| Update hosting base URL | Release owner | GitHub Actions variable `VOYAVPN_UPDATES_BASE_URL` and stable update host | The generated `latest.json` URLs resolve to signed stable updater payloads on the approved CDN endpoint. | Restore the previous `latest.json`, remove bad payloads from the host, and rerun metadata generation. |
| Diagnostics endpoint | Privacy/security owner | GitHub Actions variable `VOYAVPN_DIAGNOSTICS_ENDPOINT` and approved HTTPS ingest host | Stable preflight validates that diagnostics delivery is configured without URL credentials, query strings, fragments, local hosts, source-control hosts, or fixture hosts. | Disable diagnostics delivery through the approved control path and hold stable publication until privacy approval is restored. |

## Updater Key Procedure

Owner: security owner.

System: Tauri signer, secure key storage, `src-tauri/tauri.conf.json`, and the stable update host.

1. Generate the updater keypair outside the repository:

   ```sh
   pnpm tauri signer generate --write-keys <secure-private-key-path> --ci
   ```

2. Store the private key in the approved secret system as `TAURI_SIGNING_PRIVATE_KEY` or make it available to the signing machine as `TAURI_SIGNING_PRIVATE_KEY_PATH`.
3. Store `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` only if the generated key is password-protected.
4. Store the approved public key in `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY` for the release build.
5. Enable updater artifact creation only through the generated stable overlay. The repository default keeps `bundle.createUpdaterArtifacts` set to `false` so debug packaging remains credential-free and the committed config does not contain populated updater endpoints, public keys, private-key paths, or generated release state.
6. Generate the stable overlay at `target/release-config/tauri.updater.stable.generated.json` from a prepared shell where release-time environment names have already been supplied by the approved secret system or signing machine:

   ```sh
   export VOYAVPN_RELEASE_CHANNEL=stable
   pnpm tauri:stable-updater-config
   ```

7. Keep the generated overlay and private signing material out of git. The overlay contains the approved public key and updater CDN endpoint; private key material comes only from `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`.
8. Generate real updater metadata without dry-run signatures.

Verification:

- `latest.json` contains real signatures from signed updater artifacts.
- `latest.evidence.json` records signed-artifact sources for each target.
- An older signed build discovers the update from the intended channel endpoint.

Rollback:

- Re-publish the previous `latest.json` for the channel.
- Remove or quarantine the bad updater payloads.
- Rotate the updater keypair if private key custody is uncertain.

## Stable Tauri latest.json Generation

Owner: release engineer.

System: signed Tauri updater payloads, `.sig` files, stable CDN base URL, and `scripts/release-updater-metadata.mjs`.

Stable inputs:

- One signed updater payload and matching `.sig` file for each supported target: `darwin-aarch64`, `darwin-x86_64`, `linux-aarch64`, `linux-x86_64`, `windows-aarch64`, and `windows-x86_64`.
- `artifact-manifest.json` entries for each payload and signature with `kind`, `target`, `channel`, `version`, `bytes`, `sha256`, `name`, `path`, `originalName`, and `originalRelativePath`.
- A release-owner-approved HTTPS CDN base URL for the stable channel. Do not use example, GitHub, or placeholder hosts for stable.
- Optional `--version`, `--notes`, and `--pub-date` values when the release record must override package defaults.

Stable command shape:

```sh
node scripts/release-updater-metadata.mjs --input dist/release/signed-updater --out dist/updater/latest.json --channel stable --base-url "$VOYAVPN_UPDATES_BASE_URL"
```

Stable outputs:

- `latest.json`: Tauri updater metadata with `version`, `notes`, `pub_date`, and `platforms.<target>.url/signature` for every stable target.
- `latest.evidence.json`: generated evidence mapping every target to `source: signed-artifact`, the updater artifact name, the signature artifact name, URL, payload bytes, payload SHA-256, signature bytes, and signature SHA-256.

Stable fail-closed checks:

- `--placeholder-signatures` is rejected for `--channel stable`.
- Stable requires `--base-url` or `VOYAVPN_UPDATES_BASE_URL`; the URL must be HTTPS and must not point at example, GitHub, or placeholder hosts.
- Every stable target must have a payload and matching `.sig` file on disk.
- Manifest `bytes` and `sha256` values must match the payload and signature files.
- Empty, short, replacement, or placeholder signature values are rejected before `latest.json` is written.

## macOS Signing And Notarization

Owner: macOS release owner.

System: macOS release machine or macOS CI runner, Developer ID certificate, Apple notarization service, Tauri bundle output.

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Import Developer ID identity | macOS release owner | macOS keychain or signing runner | The signing identity is visible to `codesign` and access is restricted to the build process. | Delete the keychain item or ephemeral keychain, revoke if exposed, and stop macOS publication. |
| Build signed release package | macOS release owner | `pnpm tauri:build` in prepared signing environment | `.app` and `.dmg` are produced from the frozen commit and version. | Delete the local build output and rebuild from the same commit after fixing config or credentials. |
| Verify code signature | macOS release owner | `codesign` | `codesign --verify --deep --strict --verbose=2 <path-to-VoyaVPN.app>` passes and signer identity matches the release record. | Do not notarize or publish. Re-sign from clean artifacts. |
| Submit and staple notarization | macOS release owner | `xcrun notarytool` and `xcrun stapler` | Notary submission is accepted, stapling succeeds, and `xcrun stapler validate <path>` passes for the distributed artifact. | Do not publish. Rebuild, re-sign, resubmit, or hold macOS beta. |
| Gatekeeper launch smoke | macOS platform owner | Clean macOS machine | `spctl` assessment and first launch from DMG pass; sudo TUN, proxy restore, and uninstall smoke are recorded in [os-smoke-matrix.md](os-smoke-matrix.md). | Pull macOS assets or keep them in staging until the issue is fixed. |

## Windows Signing

Owner: Windows release owner.

System: Windows signing machine, Authenticode signer, NSIS and MSI artifacts.

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Prepare signing certificate | Windows release owner | Certificate store, hardware token, cloud signer, or secure CI import | Certificate subject, expiry, and chain are recorded without exposing private material. | Revoke or rotate exposed credentials and delete affected artifacts. |
| Build release installers | Windows release owner | `pnpm tauri:build` in prepared signing environment | NSIS `.exe` and MSI artifacts are produced from the frozen commit and version. | Delete build output and rebuild after fixing the signing environment. |
| Verify Authenticode | Windows release owner | Windows signature verification tooling | Installer signatures validate and certificate identity matches the release record. | Do not publish Windows assets. Re-sign clean artifacts. |
| Installer smoke | Windows platform owner | Clean Windows 11 and Windows 10 machines | Current-user install, launch, WebView2 bootstrap behavior, system proxy/PAC, TUN/UAC, uninstall, and no orphaned process checks pass. | Pull or hold Windows artifacts and rerun smoke after rebuilding. |

## Linux Package Approval

Owner: Linux release owner.

System: `.deb`, `.rpm`, `.AppImage`, checksum host, and optional package repository signer.

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Build Linux artifacts | Linux release owner | Linux release runner and Tauri bundle output | `.deb`, `.rpm`, and `.AppImage` are produced from the frozen commit and version. | Delete build output and rebuild from the frozen commit. |
| Verify package metadata | Linux release owner | Debian, RPM, and AppImage inspection tools | Package name, version, license, desktop entry, icon, dependencies, and executable bits match [packaging.md](packaging.md). | Do not publish bad packages. Rebuild with corrected metadata. |
| Sign checksums or repository metadata | Linux release owner | Checksum host or package repository signing system | `SHA256SUMS` matches uploaded assets; optional repository metadata signatures validate. | Remove the bad packages or repository metadata and republish the previous known-good index. |
| Linux smoke | Linux platform owner | Clean Debian-like, RPM-like, and AppImage-capable distributions as supported | Install, launch, first-run core acquisition, proxy shell restore, sudo TUN cleanup, autostart, hotkeys, and uninstall pass. | Pull or hold Linux packages and rerun smoke after rebuilding. |

## Publication Guardrails

- Use `dry_run=true` until all signing inputs are ready.
- Never publish artifacts generated with dry-run updater signatures.
- Never publish unsigned Windows or macOS beta artifacts as public beta packages.
- Do not bundle GPL or AGPL proxy cores in default installers unless legal approval is recorded outside the automated runner and reflected in notices.
- Keep app updater metadata separate from core, geo, and ruleset update flows.
