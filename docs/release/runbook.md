# Release Runbook

Batch: `08-03-release-runbooks`

This runbook is the top-level release path for VoyaVPN public beta packages. It separates credential-free validation from public publication.

## Evidence Links

- Packaging posture and unsigned debug artifact evidence: [packaging.md](packaging.md)
- Release workflow, artifact naming, and secret names: [ci-secrets.md](ci-secrets.md)
- Signing, notarization, and updater-key details: [signing-notarization.md](signing-notarization.md)
- Release OS smoke matrix: [os-smoke-matrix.md](os-smoke-matrix.md)
- Rollback procedures: [rollback.md](rollback.md)
- Manual OS evidence template: [../verification/manual-os-smoke.md](../verification/manual-os-smoke.md)
- Automated smoke coverage and gaps: [../verification/cross-platform-smoke.md](../verification/cross-platform-smoke.md)
- Update subsystem evidence: [../verification/updates.md](../verification/updates.md)

## Release Modes

| Mode | Purpose | Allowed artifacts | Publication status |
| --- | --- | --- | --- |
| Local debug packaging | Validate bundle config and artifact collection without credentials. | Unsigned debug bundles from `pnpm tauri:build --debug`. | Never publish to beta users or update channels. |
| Release CI dry run | Validate tests, package jobs, artifact normalization, checksums, and dry-run updater metadata. | Workflow artifacts with placeholder updater signatures. | Never publish to beta users or update channels. |
| Public beta publication | Build, sign, notarize where required, generate real updater metadata, smoke test, and publish. | Signed or platform-approved artifacts plus real `latest.json`. | Publish only after every manual checkpoint below is complete. |

## Local Debug Packaging

Owner: release engineer.

System: local workstation with repo prerequisites, Tauri toolchain, Rust, Node, and pnpm.

Run:

```sh
pnpm install --frozen-lockfile
pnpm run verify:ci
pnpm tauri:build --debug
node scripts/release-artifacts.mjs --input target/debug/bundle --output dist/release/local --target local-debug --channel beta --allow-empty
node scripts/release-updater-metadata.mjs --input dist/release --out dist/updater/latest.json --target darwin-x86_64,windows-x86_64,linux-x86_64 --placeholder-signatures
```

Verification: packages build or fail with a concrete local prerequisite error, `dist/release/local/SHA256SUMS` exists when artifacts are present, and dry-run updater metadata uses placeholder signatures only.

Rollback: delete `dist/release/local`, `dist/updater`, and unsigned artifacts. Do not upload unsigned debug builds to public channels.

## Public Beta Publication

Public beta requires real signing and publication ownership. The release owner must record the build commit, workflow run URL, artifact hashes, signer identity evidence, smoke evidence, and rollback decision before publishing.

1. Run the manual `Release` workflow with `channel=beta`, `build_profile=debug`, `dry_run=true`, and `updater_metadata=true`.
2. Confirm all workflow artifacts, checksums, and dry-run updater metadata are present.
3. Prepare signing credentials and updater keys using [signing-notarization.md](signing-notarization.md).
4. Build release packages in a prepared signing environment. The current repo keeps `bundle.createUpdaterArtifacts` disabled by default, so updater artifacts require a controlled release overlay or signing step that does not commit private material.
5. Sign and notarize or approve platform packages.
6. Run the OS smoke matrix from [os-smoke-matrix.md](os-smoke-matrix.md).
7. Publish packages and real updater metadata to the beta distribution systems.
8. Monitor install, launch, updater, and crash feedback. Be ready to execute [rollback.md](rollback.md).

## Manual Checkpoint Ledger

Every public beta checkpoint must have an owner, system, verification, and rollback or stop condition.

| Checkpoint | Owner | System | Verification | Rollback or stop notes |
| --- | --- | --- | --- | --- |
| Automated release dry run | Release engineer | GitHub Actions `Release` workflow | `tests`, package matrix, artifact uploads, `SHA256SUMS`, `artifact-manifest.json`, dry-run `latest.json`, and `latest.evidence.json` exist. | Stop publication. Re-run after fixing workflow or package failures. Delete stale dry-run artifacts if they were downloaded locally. |
| Build commit freeze | Release owner | Git branch, tag, and GitHub workflow dispatch | Commit SHA, branch, version, and channel are recorded in release notes and smoke evidence. | Cancel the release if new fixes land. Start over from a new commit and regenerate artifacts. |
| Secret provisioning | Security owner | GitHub Actions secrets, Apple account, Windows certificate store, local keychain or token | Required names from [ci-secrets.md](ci-secrets.md) are present in the chosen secure system; no values appear in git, logs, manifests, or docs. | Revoke or rotate any exposed credential. Delete affected workflow artifacts and rerun signing from clean credentials. |
| Updater key provisioning | Security owner | Tauri updater keypair and update metadata host | `src-tauri/tauri.conf.json` contains the approved public key, private key is supplied through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, and a test build verifies updater signatures. | Restore the previous approved public key and metadata. Rotate keys if private key handling is suspect. |
| macOS signing and notarization | macOS release owner | Apple Developer ID, `codesign`, `notarytool`, DMG/App bundle | `codesign --verify`, notarization accepted, `stapler validate`, quarantine launch, and macOS smoke evidence pass. | Do not publish macOS assets. Revoke bad artifacts, rebuild, re-sign, and re-notarize. |
| Windows signing | Windows release owner | Authenticode certificate or signing service, NSIS, MSI | Signature validates, installer trust prompt is expected, install/uninstall smoke passes on Windows 11 and Windows 10 targets. | Do not publish Windows assets. Remove uploaded installers, rebuild, and re-sign. |
| Linux package verification | Linux release owner | `.deb`, `.rpm`, `.AppImage`, checksum and optional repository signing system | Package metadata, install/uninstall, desktop entry, execute bit, and AppImage launch pass on clean distributions. | Do not publish Linux assets. Remove packages from staging or repo and rebuild. |
| Updater metadata signing | Release engineer | `scripts/release-updater-metadata.mjs`, signed updater artifacts, beta update host | Real `latest.json` has no placeholder signatures, URLs point at the beta base URL, and an older signed build detects the update. | Re-publish the previous `latest.json` or remove the beta metadata document. Keep packages available only for direct download if approved. |
| Core redistribution review | Legal or release owner | Installer payload, attribution, first-run core download policy | `bundle.externalBin` remains empty for default beta installers, notices are current, and GPL/AGPL cores are not bundled without approval. | Remove restricted payloads immediately, publish corrected packages, and document the issue. |
| OS smoke testing | Platform owners | Windows, macOS, and Linux clean machines | Matrix in [os-smoke-matrix.md](os-smoke-matrix.md) passes with logs, screenshots, commands, hashes, and skipped-check reasons recorded. | Stop publication for the failed platform. Pull or hold only that platform if others are already approved. |
| Beta publication | Release owner | GitHub release or beta download location, update CDN, checksum host | Published assets match `SHA256SUMS`, release notes include known gaps, and beta `latest.json` reaches clients. | Execute artifact or updater rollback from [rollback.md](rollback.md). |
| Post-publish monitoring | Release owner | Issue tracker, crash/log intake, update host metrics, support channel | Install, launch, update, and core-download feedback is reviewed for the declared monitoring window. | Trigger rollback if severity thresholds in [rollback.md](rollback.md) are met. |

## Publication Approval

Public beta is approved only when:

- Local or CI debug packaging evidence exists.
- All automated checks in the release workflow have passed.
- Platform signing and notarization evidence is attached where required.
- Manual OS smoke evidence covers the supported beta matrix.
- Updater metadata uses real signatures and approved URLs.
- Rollback owner and rollback assets are ready.
- No GPL or AGPL core binaries are present in default installers without separate legal approval.
