# Release Rollback

Batch: `08-03-release-runbooks`

Rollback is a release owner decision that should favor stopping updater exposure first, then removing direct-download artifacts, then publishing a fixed build. Keep rollback actions reversible and record every command, URL, artifact hash, and owner.

## Evidence Links

- Top-level release path: [runbook.md](runbook.md)
- Signing and updater details: [signing-notarization.md](signing-notarization.md)
- OS smoke evidence: [os-smoke-matrix.md](os-smoke-matrix.md)
- Release workflow and secret names: [ci-secrets.md](ci-secrets.md)

## Rollback Triggers

Rollback is required when any of these occur after staging or publication:

- Real updater metadata points at the wrong version, wrong channel, unsigned payload, placeholder signature, or wrong URL.
- Signed package fails Gatekeeper, Authenticode, package-manager trust, install, launch, or uninstall checks.
- App launch corrupts user data, OS proxy state, routes, TUN devices, autostart, or hotkey state.
- Core acquisition bundles GPL or AGPL binaries in default installers without recorded approval.
- Crash, connectivity, or update failures affect enough beta users to violate the release owner threshold.
- Any signing key, updater private key, package repository token, or publication credential may be exposed.

## Immediate Triage

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Freeze publication | Release owner | GitHub release, beta download host, update CDN, package repositories | New uploads and updater metadata changes are paused; current asset URLs and hashes are recorded. | Keep the freeze until a publish, hold, or rollback decision is recorded. |
| Classify severity | Release owner | Issue tracker, smoke evidence, support reports, crash/log intake | Severity, affected platforms, affected versions, and user impact are recorded. | If user OS state or credentials are at risk, rollback immediately before waiting for a fix. |
| Preserve evidence | Release engineer | Workflow run, artifacts, `latest.json`, logs, screenshots, package hashes | Copies of failing artifacts and metadata are retained in a private evidence location. | Do not delete evidence needed to reproduce or audit the incident. |
| Choose rollback target | Release owner | Previous beta package and previous `latest.json` | Previous known-good version, artifact hashes, and metadata are available. | If no prior beta exists, remove updater metadata and direct users to uninstall or wait for a fixed build. |

## Rollback Actions

| Action | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Disable updater exposure | Release engineer | Beta update host and `latest.json` | Clients no longer see the bad version, or they see the previous known-good version. `latest.json` has real signatures and expected URLs. | Restore the previous `latest.json` from release evidence or remove the channel metadata document. |
| Remove direct-download artifacts | Release owner | GitHub release, CDN, package host, package repository | Bad assets are hidden, deleted, yanked, or marked pre-release/private according to host capability. Checksums for removed assets are no longer advertised. | If deletion is not possible, mark the release as withdrawn and move fixed artifacts to a new version. |
| Yank platform package | Platform owner | Windows, macOS, or Linux distribution surface | The affected platform asset is no longer recommended or reachable from beta notes. Other platforms remain published only if smoke evidence shows they are unaffected. | Republish only after a rebuilt package passes the platform matrix. |
| Revert package repository metadata | Linux release owner | Debian/RPM repository or checksum index | Package manager index points to previous known-good package or no package. Repository signatures validate. | Publish corrected repository metadata and keep the bad package file quarantined. |
| Revoke or rotate exposed credentials | Security owner | Apple Developer, Windows signer, updater key storage, GitHub secrets | Compromised credentials are revoked or rotated; CI secrets are updated; old workflows cannot sign new artifacts. | Publish a new build with new keys only after clients have a compatible public key or manual install path. |
| Publish fixed build | Release owner | Git tag, release workflow, signing systems, update host | New version has distinct version number or approved rebuild identifier, fresh checksums, signed metadata, and full smoke evidence. | If fixed build fails, keep updater disabled and maintain direct user instructions. |
| User communication | Release owner | Beta notes, issue tracker, support channel | Users can see affected versions, symptoms, rollback/fix status, and remediation steps. | Update communication when metadata or package availability changes. |

## Updater Metadata Rollback

Owner: release engineer.

System: beta update host and signed updater metadata.

1. Locate the previous known-good `latest.json` and `latest.evidence.json` from release evidence.
2. Confirm platform URLs and signatures still resolve to available artifacts.
3. Replace the current beta `latest.json` with the previous known-good metadata, or remove the channel metadata file if no safe target exists.
4. From an older signed build, check for updates and confirm it does not see the bad version.
5. From the bad version, check whether the app can update forward to a fixed version. Do not assume updater downgrade works.

Verification: update checks no longer offer the bad version, metadata contains no placeholder signatures, and the release owner records the active metadata hash.

Rollback notes: if metadata replacement cannot be guaranteed because of CDN cache, purge the cache or move the channel to a new metadata path and update publication notes.

## Package Rollback By Platform

| Platform | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| macOS | macOS release owner | DMG/App bundle host and updater payload host | Bad DMG or updater payload is removed or hidden; previous notarized asset still validates if retained. | Do not attempt to modify a notarized package in place. Rebuild, re-sign, and re-notarize. |
| Windows | Windows release owner | NSIS/MSI host and updater payload host | Bad installers are removed or hidden; previous signed installers still validate if retained. | If Windows installer upgrade code behavior caused the issue, test install, repair, upgrade, and uninstall before republishing. |
| Linux | Linux release owner | `.deb`, `.rpm`, `.AppImage`, package repository metadata | Bad package is yanked or repository metadata points to previous safe package; checksums and repository signatures match. | Keep direct AppImage rollback instructions separate from package-manager rollback instructions. |

## Post-Rollback Verification

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Update channel safe state | Release engineer | Beta update host | Older clients do not see the bad version; fixed or previous version metadata validates. | Keep updater disabled if no safe state can be confirmed. |
| Download page safe state | Release owner | Public beta download surface | Bad artifacts are absent or clearly withdrawn; checksums match remaining assets. | Remove stale cache links and issue corrected notes. |
| OS state remediation | Platform owner | Affected user machines or smoke machines | Reproduction machine can restore proxy, routes, TUN devices, autostart, hotkeys, and running processes. | Publish manual remediation steps if automatic cleanup cannot be relied on. |
| Incident closeout | Release owner | Issue tracker and release notes | Root cause, affected artifacts, rollback actions, fixed version, and remaining risk are recorded. | Keep the release blocked until closeout has an owner and follow-up issue. |
