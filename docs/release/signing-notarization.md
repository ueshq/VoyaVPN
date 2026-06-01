# Signing, Notarization, And Updater Keys

Batch: `08-03-release-runbooks`

This document records the manual credentialed steps required before public beta publication. Do not commit private keys, certificates, passwords, app-specific passwords, token exports, or production updater metadata.

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
| Tauri updater public key | Release engineer | `src-tauri/tauri.conf.json` | Placeholder `VOYAVPN_UPDATER_PUBLIC_KEY_PLACEHOLDER_REPLACE_BEFORE_RELEASE` is replaced with the approved public key before real updater publication. | Revert to the previous approved public key or disable update metadata for the affected channel. |
| Update hosting base URL | Release owner | GitHub Actions variable `VOYAVPN_UPDATES_BASE_URL` and beta update host | The generated `latest.json` URLs resolve to signed beta updater payloads and no longer use `updates.voyavpn.example` unless that host is the approved endpoint. | Restore the previous `latest.json`, remove bad payloads from the host, and rerun metadata generation. |

## Updater Key Procedure

Owner: security owner.

System: Tauri signer, secure key storage, `src-tauri/tauri.conf.json`, and the beta update host.

1. Generate the updater keypair outside the repository:

   ```sh
   pnpm tauri signer generate --write-keys <secure-private-key-path> --ci
   ```

2. Store the private key in the approved secret system as `TAURI_SIGNING_PRIVATE_KEY` or make it available to the signing machine as `TAURI_SIGNING_PRIVATE_KEY_PATH`.
3. Store `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` only if the generated key is password-protected.
4. Replace the updater public key placeholder in `src-tauri/tauri.conf.json`.
5. Enable updater artifact creation only through a release-controlled config overlay or signing step. The repository default keeps `bundle.createUpdaterArtifacts` set to `false` so debug packaging remains credential-free.
6. Generate real updater metadata without `--placeholder-signatures`.

Verification:

- `latest.json` contains real signatures, not `VOYAVPN_UPDATER_SIGNATURE_PLACEHOLDER_*`.
- `latest.evidence.json` records signed-artifact sources for each target.
- An older signed beta build discovers the update from the beta endpoint.

Rollback:

- Re-publish the previous `latest.json` for the channel.
- Remove or quarantine the bad updater payloads.
- Rotate the updater keypair if private key custody is uncertain.

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
- Never publish artifacts generated with placeholder updater signatures.
- Never publish unsigned Windows or macOS beta artifacts as public beta packages.
- Do not bundle GPL or AGPL proxy cores in default installers unless legal approval is recorded outside the automated runner and reflected in notices.
- Keep app updater metadata separate from core, geo, and ruleset update flows.
