# Release Runbook

Batch: `05-03-stable-runbooks-and-smoke`

This runbook is the top-level release path for VoyaVPN production stable packages. It separates local and CI-generated evidence from external publication, signing, CDN mutation, smoke machines, and release approval.

## Evidence Links

- Packaging posture and unsigned debug artifact evidence: [packaging.md](packaging.md)
- Release workflow, artifact naming, and secret names: [ci-secrets.md](ci-secrets.md)
- Signing, notarization, and updater-key details: [signing-notarization.md](signing-notarization.md)
- Third-party notices and core redistribution gate: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
- Diagnostics privacy contract: [diagnostics-privacy.md](diagnostics-privacy.md)
- Release OS smoke matrix: [os-smoke-matrix.md](os-smoke-matrix.md)
- Rollback procedures: [rollback.md](rollback.md)
- Stable external evidence checklist: [external-evidence-checklist.md](external-evidence-checklist.md)
- Stable release gate: [../verification/stable-release-gate.md](../verification/stable-release-gate.md)
- Manual OS evidence template: [../verification/manual-os-smoke.md](../verification/manual-os-smoke.md)
- Automated smoke coverage and gaps: [../verification/cross-platform-smoke.md](../verification/cross-platform-smoke.md)
- Update subsystem evidence: [../verification/updates.md](../verification/updates.md)

## Publication Boundary

The generated rollout runner, local scripts, and GitHub Actions workflow produce evidence only. They do not publish artifacts, upload to the CDN, mutate stable pointers, purge caches, access signing secrets, notarize applications, approve legal notices, approve diagnostics, or perform real OS smoke.

Production stable download, updater, core, geo, SRS, checksum, and signature URLs must resolve from the approved VoyaVPN CDN. GitHub Actions may be used to build and preserve evidence, and upstream GitHub URLs may appear as source/license references, but GitHub is not a production download URL for stable app, updater, core, geo, or SRS assets.

## Release Modes

| Mode | Purpose | Allowed artifacts | Publication status |
| --- | --- | --- | --- |
| Local debug packaging | Validate bundle config and artifact collection without credentials. | Unsigned debug bundles from `pnpm tauri:build --debug`. | Never publish to beta users or update channels. |
| Release CI dry run | Validate tests, package jobs, artifact normalization, checksums, and dry-run updater metadata. | Workflow artifacts with non-publishable updater signatures. | Never publish to beta users or update channels. |
| Production stable CI staging | Validate the six-target stable workflow, stable readiness checker, CDN release-index staging, signed updater metadata generation, and package artifact uploads. | Workflow artifacts for `darwin-aarch64`, `darwin-x86_64`, `linux-aarch64`, `linux-x86_64`, `windows-aarch64`, and `windows-x86_64`. | Never publish automatically; CDN publication is an external approved step. |
| Production stable publication | Publish the approved stable app, updater metadata, bundled seed assets, and core update CDN assets through own CDN only. | Signed app artifacts, signed Tauri updater payloads, approved Xray/mihomo/sing-box seed assets, core manifests, checksums, source references, and release evidence. | Publish only after every stable checkpoint, including legal/source approval, is complete. |
| Public beta publication | Build, sign, notarize where required, generate real updater metadata, smoke test, and publish. | Signed or platform-approved artifacts plus real `latest.json`. | Publish only after every manual checkpoint below is complete. |

## Release Readiness Checker

`scripts/check-release-readiness.mjs` is the local fail-closed gate for release metadata shape, required docs, bundled notices, Tauri updater config, and production-blocking placeholders.

Dry-run mode uses repository fixtures and does not require signing secrets:

```sh
node scripts/check-release-readiness.mjs --mode dry-run --cdn-base-url https://cdn.voyavpn.test/stable
```

Stable mode is intended for a prepared release environment:

```sh
pnpm check:release:stable
```

Stable mode requires `VOYAVPN_CDN_BASE_URL` or `--cdn-base-url`, `VOYAVPN_DIAGNOSTICS_ENDPOINT` or `--diagnostics-endpoint`, signed updater artifacts, real Tauri updater signing input through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, platform signing env names for macOS and Windows, a non-placeholder updater public key in the generated overlay, updater artifacts enabled, and no forbidden example, dry-run updater, or GitHub release/download URLs in production surfaces.

### Prepared Stable Environment Preflight

Release owners run this exact preflight only after external variables, signing secrets, and real artifact inputs already exist in the prepared release shell. Secret values stay in the approved secret system and must not be pasted into docs, commits, or logs.

Required prepared names:

| Input | Purpose |
| --- | --- |
| `VOYAVPN_RELEASE_CHANNEL` | Selects stable overlay generation for Tauri build wrappers. |
| `VOYAVPN_CDN_BASE_URL` | Approved HTTPS stable CDN base URL for release index, manual downloads, core assets, and staging evidence. |
| `VOYAVPN_UPDATES_BASE_URL` | Approved HTTPS stable updater CDN base URL used to derive `latest.json`. |
| `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY` | Approved non-placeholder Tauri updater public key written into the generated overlay. |
| `VOYAVPN_DIAGNOSTICS_ENDPOINT` | Approved HTTPS diagnostics ingest endpoint. |
| `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH` | Updater private signing input supplied by the approved secret system or signing machine. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional updater private-key password when the key requires one. |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | macOS signing and notarization inputs required for stable readiness. |
| `WINDOWS_CERTIFICATE_BASE64`, `WINDOWS_CERTIFICATE_PASSWORD` | Windows signing inputs required for stable readiness. |
| `VOYAVPN_RELEASE_ARTIFACTS_DIR`, `VOYAVPN_SIGNED_UPDATER_DIR`, `VOYAVPN_CORE_ASSETS_FILE` | Optional stable artifact input paths when the prepared environment does not use the default `dist/release/...` paths. |

Prepared release shell command sequence:

```sh
export VOYAVPN_RELEASE_CHANNEL=stable
pnpm tauri:stable-updater-config
pnpm check:release:stable
```

`pnpm tauri:stable-updater-config` writes `target/release-config/tauri.updater.stable.generated.json`. `pnpm check:release:stable` then scans that overlay merged over `src-tauri/tauri.conf.json` and validates stable environment inputs, generated release index input, signed updater input, core asset source input, diagnostics endpoint config, and production URL blockers.

Expected failures in an unprepared local shell are environment-only skips, not repository blockers: missing `VOYAVPN_CDN_BASE_URL`, missing `VOYAVPN_UPDATES_BASE_URL`, missing `VOYAVPN_UPDATER_PUBLIC_KEY`, missing `VOYAVPN_DIAGNOSTICS_ENDPOINT`, missing `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, missing Apple or Windows signing inputs, missing real stable artifact directories, fixture paths used in stable mode, placeholder updater signatures, or forbidden example, `.test`, localhost, placeholder, or GitHub production download URLs.

Expected pass criteria in a prepared stable environment: the overlay exists at `target/release-config/tauri.updater.stable.generated.json`, it enables `bundle.createUpdaterArtifacts`, updater metadata uses the approved HTTPS updater CDN and real signatures, release/core metadata use only approved CDN-derived production URLs, stable artifact inputs are not fixtures, and `pnpm check:release:stable` exits successfully with zero failures. Stable pointer promotion must not begin until this prepared-environment check passes.

Generate the stable Tauri updater overlay before stable readiness when inspecting the overlay directly from a prepared shell:

```sh
export VOYAVPN_RELEASE_CHANNEL=stable
pnpm tauri:stable-updater-config
```

The generated overlay path is `target/release-config/tauri.updater.stable.generated.json`. `pnpm check:release:stable` scans that overlay merged over `src-tauri/tauri.conf.json`.

The committed Tauri config intentionally keeps `bundle.createUpdaterArtifacts` disabled and omits `plugins.updater` so repository-controlled builds do not contain updater credentials or generated release state. The stable overlay is the only release path that enables `createUpdaterArtifacts`; it is generated from environment variables, used by the package job through `--config`, and left uncommitted.

Required stable overlay inputs:

| Input | Purpose |
| --- | --- |
| `VOYAVPN_RELEASE_CHANNEL=stable` or `VOYAVPN_TAURI_UPDATER_CONFIG=stable` | Selects the stable updater overlay path in `scripts/tauri-build.mjs`. |
| `VOYAVPN_UPDATES_BASE_URL` | Approved HTTPS updater CDN base URL used to derive `<base>/latest.json`. |
| `VOYAVPN_UPDATER_PUBLIC_KEY` or `TAURI_UPDATER_PUBLIC_KEY` | Approved non-placeholder Tauri updater public key written into the generated overlay. |
| `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH` | Private signing material or path supplied by the approved secret system. It is required for updater artifact creation and must not be committed. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional password when the private key requires one. |

The checker writes generated release index, updater metadata, core manifest, and evidence files to a temporary directory by default. It does not upload artifacts, access cloud consoles, sign packages, notarize apps, or approve external release gates.

### Release Evidence Helpers

Generate a fillable stable release record from the current commit before external signing and smoke work starts:

```sh
pnpm release:record
```

The default output is `dist/release/stable-release-record.md`. It records the current version, branch, commit, worktree status, required command evidence, target artifact rows, CDN pointer rows, external gate rows, and final Go/No-Go fields. It is evidence scaffolding only and does not approve any gate.

Validate staged metadata before stable pointer promotion:

```sh
pnpm release:verify-staging -- --release-index <release-index.json> --updater-metadata <latest.json> --core-manifest <core-assets.json>
pnpm release:verify-staging -- --release-index <release-index.json> --updater-metadata <latest.json> --core-manifest <core-assets.json> --probe
pnpm release:verify-staging -- --release-index <release-index.json> --updater-metadata <latest.json> --core-manifest <core-assets.json> --download-and-hash
```

The staging verifier checks stable target completeness, approved HTTPS CDN hosts, no GitHub/example/local/test production artifact URLs, updater signature presence, core asset matrix completeness, byte sizes, and SHA-256 shape. `--probe` validates URL reachability without full downloads; `--download-and-hash` downloads referenced assets and verifies checksums. It does not upload, purge, mutate pointers, sign, notarize, or approve publication.

## Local Debug Packaging

Owner: release engineer.

System: local workstation with repo prerequisites, Tauri toolchain, Rust, Node, and pnpm.

Run:

```sh
pnpm install --frozen-lockfile
pnpm run verify:ci
pnpm tauri:build --debug
node scripts/release-artifacts.mjs --input target/debug/bundle --output dist/release/local --target local-debug --channel beta --allow-empty
node scripts/release-updater-metadata.mjs --input dist/release --out dist/updater/latest.json --target darwin-aarch64,darwin-x86_64,linux-aarch64,linux-x86_64,windows-aarch64,windows-x86_64 --placeholder-signatures
```

Verification: packages build or fail with a concrete local prerequisite error, `dist/release/local/SHA256SUMS` exists when artifacts are present, and dry-run updater metadata remains clearly non-publishable.

Rollback: delete `dist/release/local`, `dist/updater`, and unsigned artifacts. Do not upload unsigned debug builds to public channels.

## Production Stable Publication

Owner: release owner, with named platform, security, legal, privacy, and CDN owners for their checkpoints.

System: GitHub Actions `Release` workflow for evidence, approved signing systems, approved diagnostics endpoint, clean Windows/macOS/Linux smoke machines, and the VoyaVPN CDN for staging and final stable pointers.

Stable publication follows this order:

1. Freeze the commit, version, channel, and target matrix. Record the commit SHA, tag, workflow dispatch inputs, and planned stable version.
2. Run local automated gates from the frozen commit: `pnpm run verify:ci`, `pnpm run build`, and local debug packaging when required by release policy.
3. Dispatch the `Release` workflow with `channel=stable`, `build_profile=release`, `dry_run=false`, and updater metadata enabled. The workflow must produce six package artifacts, `SHA256SUMS`, artifact manifests, updater metadata evidence, CDN release-index evidence, and core manifest evidence. It must not publish externally.
4. Complete signing, notarization, updater key, legal redistribution, diagnostics approval, and platform package checks from the ledger below.
5. Stage immutable CDN paths for the approved version: manual download artifacts and `release-index.json`, Tauri updater payloads and `latest.json`, Xray/mihomo/sing-box core archives and manifest, geo/SRS assets and manifests, checksums, signatures, notices, and evidence JSON.
6. Verify CDN staging before pointer promotion. Every staged URL must use the approved CDN host, return the expected byte size, match SHA-256 evidence, and avoid example hosts, fixture hosts, placeholder signatures, and production GitHub download URLs.
7. Run updater smoke from an older signed build for each stable target. The older build must detect the exact new stable version from the staged updater metadata, validate signatures, apply the update, relaunch, and show the expected version.
8. Run manual download smoke from the staged release index. Each platform owner downloads the package for the exact OS/arch target from the CDN, verifies checksum/signature evidence, installs or launches it, and records clean-machine results.
9. Run core smoke for Xray, mihomo, and sing-box. Confirm seed copy into app data when a seed is included, CDN core manifest download, checksum verification, staged extraction, Unix chmod where applicable, safe swap, rollback-on-failure behavior, runtime restart, geo/SRS separation, and no execution from the read-only app bundle.
10. Run diagnostics smoke. Confirm default-on production diagnostics, visible opt-out, redacted event envelope, no forbidden payload fields, bounded queue behavior, endpoint delivery or approved endpoint-disable state, and opt-out clearing pending events.
11. Promote stable CDN pointers only after every staged verification passes: manual release index pointer, app updater `latest.json` pointer, core manifest pointer, geo/SRS manifest pointers, checksums, and notices. Record pointer object hashes before and after promotion.
12. Monitor install, launch, updater smoke, manual download smoke, core smoke, diagnostics, and crash-class feedback for the approved window. Keep rollback owner and previous pointers on call until closeout.

Verification: every external checkpoint below has owner, system, verification, rollback or stop condition, and artifact/hash evidence attached through [external-evidence-checklist.md](external-evidence-checklist.md) and linked from the stable gate.

Rollback: execute [rollback.md](rollback.md). Stop updater exposure first, then manual download exposure, then core/geo/SRS manifest exposure, then diagnostics delivery if privacy or payload risk is involved. Quarantine bad artifacts with hashes instead of deleting the only evidence copy.

## Public Beta Publication

Public beta requires real signing and publication ownership. The release owner must record the build commit, workflow run URL, artifact hashes, signer identity evidence, smoke evidence, and rollback decision before publishing.

1. Run the manual `Release` workflow with `channel=beta`, `build_profile=debug`, `dry_run=true`, and `updater_metadata=true`.
2. Confirm all workflow artifacts, checksums, and dry-run updater metadata are present.
3. Prepare signing credentials and updater keys using [signing-notarization.md](signing-notarization.md).
4. Build release packages in a prepared signing environment. The current repo keeps `bundle.createUpdaterArtifacts` disabled by default, so updater artifacts require the generated stable overlay and signing inputs without committing private material.
5. Sign and notarize or approve platform packages.
6. Run the OS smoke matrix from [os-smoke-matrix.md](os-smoke-matrix.md).
7. Publish packages and real updater metadata to the beta distribution systems.
8. Monitor install, launch, updater, and crash feedback. Be ready to execute [rollback.md](rollback.md).

## Manual Checkpoint Ledger

Every publication checkpoint must have an owner, system, verification, and rollback or stop condition.

| Checkpoint | Owner | System | Verification | Rollback or stop notes |
| --- | --- | --- | --- | --- |
| Automated release dry run | Release engineer | GitHub Actions `Release` workflow | `tests`, package matrix, artifact uploads, `SHA256SUMS`, `artifact-manifest.json`, dry-run `latest.json`, and `latest.evidence.json` exist. | Stop publication. Re-run after fixing workflow or package failures. Delete stale dry-run artifacts if they were downloaded locally. |
| Stable CI staging | Release engineer | GitHub Actions `Release` workflow with `channel=stable`, `build_profile=release`, and `dry_run=false` | Stable preflight validates CDN base URL, updater public key, updater signing key, Apple and Windows signing inputs, diagnostics endpoint, readiness checker output, six package artifacts, updater metadata, and CDN staging metadata. | Do not publish. Fix missing inputs or failed target jobs, then rerun from the frozen commit. |
| Build commit freeze | Release owner | Git branch, tag, and GitHub workflow dispatch | Commit SHA, branch, version, and channel are recorded in release notes and smoke evidence. | Cancel the release if new fixes land. Start over from a new commit and regenerate artifacts. |
| Secret provisioning | Security owner | GitHub Actions secrets, Apple account, Windows certificate store, local keychain or token | Required names from [ci-secrets.md](ci-secrets.md) are present in the chosen secure system; no values appear in git, logs, manifests, or docs. | Revoke or rotate any exposed credential. Delete affected workflow artifacts and rerun signing from clean credentials. |
| Updater key provisioning | Security owner | Tauri updater keypair and update metadata host | The generated stable overlay contains the approved public key, private key is supplied through `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, and a test build verifies updater signatures. | Restore the previous approved public key and metadata. Rotate keys if private key handling is suspect. |
| macOS signing and notarization | macOS release owner | Apple Developer ID, `codesign`, `notarytool`, DMG/App bundle | `codesign --verify`, notarization accepted, `stapler validate`, quarantine launch, and macOS smoke evidence pass. | Do not publish macOS assets. Revoke bad artifacts, rebuild, re-sign, and re-notarize. |
| Windows signing | Windows release owner | Authenticode certificate or signing service, NSIS, MSI | Signature validates, installer trust prompt is expected, install/uninstall smoke passes on Windows 11 and Windows 10 targets. | Do not publish Windows assets. Remove uploaded installers, rebuild, and re-sign. |
| Linux package verification | Linux release owner | `.deb`, `.rpm`, `.AppImage`, checksum and optional repository signing system | Package metadata, install/uninstall, desktop entry, execute bit, and AppImage launch pass on clean distributions. | Do not publish Linux assets. Remove packages from staging or repo and rebuild. |
| Updater metadata signing | Release engineer | `scripts/release-updater-metadata.mjs`, signed updater artifacts, stable updater CDN staging path | Real `latest.json` has no dry-run signatures, URLs point at the approved CDN base URL, and an older signed build detects the exact target update. | Stop pointer promotion. Restore the previous stable `latest.json` pointer or remove the channel metadata document. Keep packages available only for direct CDN download if approved. |
| CDN staging | CDN owner | VoyaVPN CDN immutable versioned paths for app, updater, core, geo, SRS, checksums, signatures, notices, and evidence | Staged objects resolve from the approved CDN host, byte sizes and SHA-256 values match generated evidence, cache headers match release policy, and no stable URL points to GitHub, an example host, or a fixture host. | Stop pointer promotion. Remove public reachability if exposed accidentally, purge stale caches, and quarantine bad staged objects with hashes. |
| Stable pointer promotion | Release owner and CDN owner | VoyaVPN CDN mutable stable pointers for release index, updater metadata, core manifest, geo/SRS manifests, checksums, and notices | Before and after pointer object hashes are recorded; clients resolve the promoted stable version only after all gate evidence is complete. | Roll back pointers to the previous known-good release index, `latest.json`, core manifest, and geo/SRS manifests, then purge or bypass caches. |
| Manual download smoke | Platform owners | Stable CDN release index and signed platform packages | Each x64 and arm64 platform downloads from the CDN release index, validates SHA-256 and signature/notarization evidence, installs or launches, and records clean-machine smoke. | Hold or roll back the affected manual release-index entries and remove bad package exposure. |
| Updater smoke | Release engineer and platform owners | Older signed build, stable updater CDN metadata, signed updater payloads | Each supported target detects the new version, validates updater signature, downloads from CDN, applies, relaunches, and reports the expected version. | Restore the previous `latest.json` pointer or remove updater metadata for the affected target before direct-download rollback. |
| Core smoke | Platform owners and release engineer | In-app update manager, app-data `bin/`, core manifest, Xray/mihomo/sing-box CDN assets, geo/SRS manifests | Seed copy, manifest check, download, checksum verification, staged extraction, chmod on Unix, safe swap, rollback on failed apply, runtime restart, and geo/SRS separation pass on x64 and arm64 targets. | Restore the previous core manifest pointer, quarantine bad core archives, and keep previous app-data core backups for reproduction. |
| Diagnostics smoke | Privacy/security owner and platform owners | Stable diagnostics settings, event envelope, endpoint or approved endpoint-disable control | Default-on state, opt-out, queue clearing, redaction tests, endpoint delivery or approved disablement, and absence of node URLs, credentials, IP addresses, full logs, generated configs, and traffic destinations are verified. | Disable diagnostics delivery through the approved control path and stop publication if forbidden data can be emitted. |
| Core seed redistribution approval | Legal or release owner | Installer/package resources, `docs/release/THIRD_PARTY_NOTICES.md`, core asset manifest, CDN staging metadata, and source evidence | Approval record lists exact Xray, mihomo, and sing-box versions, OS/architecture assets, source URLs, license names, SHA-256 values, byte sizes, source availability evidence, and confirms that GPL obligations for mihomo and sing-box are satisfied. It also confirms no AGPL or unsupported core is bundled or published as a first-stable core CDN asset. | Block stable publication. Remove unapproved seed or CDN core assets, publish corrected notices/source evidence, and rerun package and CDN metadata generation. |
| OS smoke testing | Platform owners | Windows, macOS, and Linux clean machines | Matrix in [os-smoke-matrix.md](os-smoke-matrix.md) passes with logs, screenshots, commands, hashes, and skipped-check reasons recorded. | Stop publication for the failed platform. Pull or hold only that platform if others are already approved. |
| Stable publication | Release owner | VoyaVPN CDN stable channel, release notes, checksum host, evidence tracker, monitoring systems | Published assets match `SHA256SUMS`, stable release notes cite known residual risks, release index and `latest.json` reach clients, and monitoring owner/window are active. | Execute pointer rollback, artifact withdrawal, diagnostics disablement, or fixed-build publication from [rollback.md](rollback.md). |
| Public beta publication | Release owner | Non-stable beta distribution systems, update host, checksum host | Published beta assets match `SHA256SUMS`, release notes include known gaps, and beta `latest.json` reaches clients. | Execute artifact or updater rollback from [rollback.md](rollback.md). |
| Post-publish monitoring | Release owner | Issue tracker, crash/log intake, update host metrics, support channel | Install, launch, update, and core-download feedback is reviewed for the declared monitoring window. | Trigger rollback if severity thresholds in [rollback.md](rollback.md) are met. |

## Publication Approval

Publication is approved only when:

- Local or CI debug packaging evidence exists.
- All automated checks in the release workflow have passed.
- Platform signing and notarization evidence is attached where required.
- Manual OS smoke evidence covers the supported stable x64 and arm64 matrix.
- Updater metadata uses real signatures and approved CDN URLs.
- The stable external evidence checklist has an entry for CDN staging, pointer promotion, signing/notarization, Windows signing, Linux package verification, updater smoke, manual download smoke, core smoke, diagnostics smoke, legal approval, privacy approval, rollback, and monitoring.
- CDN staging, pointer promotion, updater smoke, manual download smoke, core smoke, diagnostics smoke, and rollback readiness are recorded.
- Rollback owner and rollback assets are ready.
- Xray, mihomo, and sing-box seed/core CDN redistribution has a recorded legal/source approval checkpoint when included.
- No AGPL core or unsupported core binary is present in stable seed resources or first-stable core CDN assets without a separate approval record.
