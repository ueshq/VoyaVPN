# Release Rollback

Batch: `05-03-stable-runbooks-and-smoke`

Rollback is a release owner decision that should favor stopping updater exposure first, then removing direct-download exposure, then restoring core/geo/SRS manifests, then disabling diagnostics if privacy or payload risk is involved, and finally publishing a fixed build. Keep rollback actions reversible and record every command, URL, artifact hash, pointer hash, cache purge, quarantine path, and owner.

The generated runner does not execute external publication or rollback. It can generate evidence and docs only; CDN pointer changes, cache purge, artifact quarantine, signing, notarization, diagnostics disablement, and user communication are external release operations.

## Evidence Links

- Top-level release path: [runbook.md](runbook.md)
- Signing and updater details: [signing-notarization.md](signing-notarization.md)
- OS smoke evidence: [os-smoke-matrix.md](os-smoke-matrix.md)
- Release workflow and secret names: [ci-secrets.md](ci-secrets.md)
- Diagnostics privacy contract: [diagnostics-privacy.md](diagnostics-privacy.md)
- Stable external evidence checklist: [external-evidence-checklist.md](external-evidence-checklist.md)
- Stable release gate: [../verification/stable-release-gate.md](../verification/stable-release-gate.md)

## Rollback Triggers

Rollback is required when any of these occur after staging or publication:

- Real updater metadata points at the wrong version, wrong channel, unsigned payload, placeholder signature, or wrong URL.
- Manual CDN release index points at the wrong app package, wrong OS/arch, wrong checksum, wrong signature, or non-CDN production URL.
- Core manifest, geo manifest, or SRS manifest points at a corrupted, unapproved, wrong-architecture, unsupported, or checksum-mismatched asset.
- Diagnostics can emit forbidden data, ignores opt-out, uses an unapproved endpoint, or blocks app workflows.
- Signed package fails Gatekeeper, Authenticode, package-manager trust, install, launch, or uninstall checks.
- App launch corrupts user data, OS proxy state, routes, TUN devices, autostart, or hotkey state.
- Core acquisition bundles GPL or AGPL binaries in default installers without recorded approval.
- Crash, connectivity, update, core apply, or diagnostics failures affect enough stable users to violate the release owner threshold.
- Any signing key, updater private key, package repository token, or publication credential may be exposed.

## Immediate Triage

| Checkpoint | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Freeze publication | Release owner | VoyaVPN CDN stable channel, updater metadata host, package repositories when used | New uploads, pointer promotion, cache purge, and metadata changes are paused; current pointer object hashes, asset URLs, and artifact hashes are recorded. | Keep the freeze until a publish, hold, or rollback decision is recorded. |
| Classify severity | Release owner | Issue tracker, smoke evidence, support reports, crash/log intake | Severity, affected platforms, affected versions, and user impact are recorded. | If user OS state or credentials are at risk, rollback immediately before waiting for a fix. |
| Preserve evidence | Release engineer | Workflow run, artifacts, `latest.json`, logs, screenshots, package hashes | Copies of failing artifacts and metadata are retained in a private evidence location. | Do not delete evidence needed to reproduce or audit the incident. |
| Choose rollback target | Release owner | Previous stable release-index, `latest.json`, core manifest, geo/SRS manifests, diagnostics control state, and package set | Previous known-good pointer hashes, artifact hashes, and metadata are available. | If no prior stable exists, remove updater/manual/core exposure and direct users to uninstall, disable affected features, or wait for a fixed build. |

Rollback readiness is release-blocking before pointer promotion. The release owner records previous pointer hashes, cache purge evidence, diagnostics disablement control, quarantine location, monitoring thresholds, and rollback owner acknowledgement in [external-evidence-checklist.md](external-evidence-checklist.md).

## Rollback Actions

| Action | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| App updater pointer rollback | Release engineer | Stable updater CDN pointer and signed `latest.json` | Clients no longer see the bad version, or they see the previous known-good version. `latest.json` has real signatures, approved CDN URLs, and the recorded pointer hash. | Restore the previous `latest.json` pointer from release evidence or remove the channel metadata document. |
| Manual index rollback | Release owner and CDN owner | Stable manual release-index pointer, checksum index, download page data | Bad direct-download entries are absent; remaining CDN entries match `SHA256SUMS`, signatures, and package evidence. | Restore the previous release-index pointer, purge caches, and keep affected direct downloads unavailable until rebuilt. |
| Core manifest rollback | Release engineer | Empty core asset manifest pointer, geo manifest pointer, SRS manifest pointer | Clients resolve the expected empty core manifest and previous known-good geo/SRS entries; checksums match previous evidence. sing-box seed rollback follows the app package rollback path. | Restore previous manifest pointers, purge caches, and keep bad geo/SRS assets quarantined. |
| Diagnostics disablement | Privacy/security owner | Diagnostics endpoint, ingest routing, release config or remote disable control | New diagnostics delivery is disabled or routed to the approved safe state; opt-out behavior remains intact; no forbidden payload is accepted. | Keep diagnostics disabled until privacy/security approves a fixed endpoint, schema, or app build. |
| Bad artifact quarantine | Release engineer and CDN owner | Private evidence storage, CDN staging area, package repositories | Bad app packages, updater payloads, core archives, manifests, checksums, and signatures are no longer publicly advertised and are retained with SHA-256, byte size, source path, and incident id. | Do not reuse quarantined filenames or mutable paths for a fixed build; publish a new version or approved rebuild identifier. |
| Yank platform package | Platform owner | Windows, macOS, or Linux distribution surface | The affected platform asset is no longer recommended or reachable from stable release notes or CDN metadata. Other platforms remain published only if smoke evidence shows they are unaffected. | Republish only after a rebuilt package passes the platform matrix. |
| Revert package repository metadata | Linux release owner | Debian/RPM repository or checksum index | Package manager index points to previous known-good package or no package. Repository signatures validate. | Publish corrected repository metadata and keep the bad package file quarantined. |
| Revoke or rotate exposed credentials | Security owner | Apple Developer, Windows signer, updater key storage, GitHub secrets | Compromised credentials are revoked or rotated; CI secrets are updated; old workflows cannot sign new artifacts. | Publish a new build with new keys only after clients have a compatible public key or manual install path. |
| Publish fixed build | Release owner | Git tag, release workflow, signing systems, update host | New version has distinct version number or approved rebuild identifier, fresh checksums, signed metadata, and full smoke evidence. | If fixed build fails, keep updater disabled and maintain direct user instructions. |
| User communication | Release owner | Stable release notes, issue tracker, support channel | Users can see affected versions, symptoms, rollback/fix status, and remediation steps. | Update communication when metadata, package availability, diagnostics state, or fixed-build status changes. |

## App Updater Pointer Rollback

Owner: release engineer.

System: stable updater CDN pointer and signed updater metadata.

1. Locate the previous known-good `latest.json` and `latest.evidence.json` from release evidence.
2. Confirm platform URLs and signatures still resolve to available artifacts.
3. Replace the current stable `latest.json` pointer with the previous known-good metadata, or remove the channel metadata file if no safe target exists.
4. From an older signed build, check for updates and confirm it does not see the bad version.
5. From the bad version, check whether the app can update forward to a fixed version. Do not assume updater downgrade works.

Verification: update checks no longer offer the bad version, metadata contains no dry-run signatures, and the release owner records the active metadata hash.

Rollback notes: if metadata replacement cannot be guaranteed because of CDN cache, purge the cache or move the channel to a new metadata path and update publication notes.

## Manual Index Rollback

Owner: release owner and CDN owner.

System: stable manual CDN release-index pointer, checksum index, notices, and download page data.

1. Locate the previous known-good `release-index.json`, `release-index.evidence.json`, `SHA256SUMS`, notices, and download page metadata.
2. Confirm every remaining app artifact URL resolves from the approved CDN and matches recorded SHA-256 and signature/notarization evidence.
3. Restore the previous stable release-index pointer or remove only the affected OS/arch entries if release owner approves a partial platform hold.
4. Purge or bypass CDN caches for the release index, checksum index, and download page data.
5. Run manual download smoke for one unaffected previous package and confirm the bad package is no longer advertised.

Verification: stable manual downloads no longer expose the bad artifact, checksum evidence matches the active index, and the active release-index pointer hash is recorded.

Rollback notes: direct package deletion is optional and host-dependent. Even when hidden or deleted from public paths, the bad artifact must remain quarantined in private evidence storage with hash, byte size, incident id, and original CDN path.

## Core Manifest Rollback

Owner: release engineer, with legal/release owner approval when redistribution scope changes.

System: stable core asset manifest pointer, geo manifest pointer, SRS manifest pointer, and CDN core/geo/SRS assets.

1. Locate previous known-good manifests and evidence for the empty core manifest, geo files, and SRS assets. For sing-box seed regressions, locate the previous known-good app package evidence.
2. Confirm previous manifest URLs resolve from the approved CDN and match SHA-256, byte size, license, and source evidence.
3. Restore the previous core manifest pointer. Restore geo/SRS manifest pointers if the bad release affected those lifecycles.
4. Purge or bypass CDN caches for affected manifests.
5. From a clean smoke profile, run core smoke for the affected OS/arch and confirm update checks resolve the previous known-good entries.

Verification: clients no longer download the bad core/geo/SRS assets, previous checksums match, app-data safe-swap backups are preserved for reproduction, and the active manifest hash is recorded.

Rollback notes: app-side core apply rollback protects local app data after a failed swap, but publication rollback still requires manifest pointer rollback so other clients stop seeing the bad asset.

## Diagnostics Disablement

Owner: privacy/security owner.

System: diagnostics ingest endpoint, routing or firewall control, release config, and any approved remote disable control.

1. If diagnostics can emit forbidden fields or ignore opt-out, disable network ingest first.
2. If the app supports an approved remote/config disable control, set it to stop new diagnostics delivery while preserving local user opt-out state.
3. Confirm the endpoint rejects or drops new events without storing payloads beyond the approved incident evidence.
4. Record whether a fixed app build is required to correct local event construction, queueing, or opt-out behavior.
5. Keep diagnostics disabled until the privacy/security owner signs off on the fixed endpoint, schema, config, or app build.

Verification: diagnostics smoke shows no network delivery from stable clients except the approved disabled/skipped state, and forbidden payload tests remain attached to the incident.

Rollback notes: diagnostics disablement must not disable updater checks or bundled core seed recovery. Those systems remain separate unless the incident affects shared release configuration.

## Bad Artifact Quarantine

Owner: release engineer and CDN owner.

System: private evidence storage, CDN staging area, package repositories when used, and release incident tracker.

1. Record artifact name, kind, target, version, channel, original public path, byte size, SHA-256, signature path, manifest entry, and first failing evidence.
2. Remove the artifact from public release-index, updater, core, geo, or SRS metadata before moving files.
3. Move or copy the bad artifact and its metadata into private quarantine storage with restricted write access.
4. Preserve enough evidence to reproduce the failure, but do not keep user credentials, node URLs, IP addresses, full logs, generated configs, or traffic destinations in quarantine notes.
5. Publish a fixed build under a new version or approved rebuild identifier. Do not overwrite the quarantined object in place.

Verification: public metadata no longer references the quarantined object, private evidence contains the hash and incident id, and fixed artifacts use fresh checksums and metadata.

Rollback notes: quarantine is not a substitute for pointer rollback. Users must stop receiving the bad artifact through active stable metadata before quarantine is considered complete.

## Package Rollback By Platform

| Platform | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| macOS | macOS release owner | Stable CDN DMG/App bundle path and updater payload path | Bad DMG or updater payload is removed from stable metadata or hidden; previous notarized asset still validates if retained. | Do not attempt to modify a notarized package in place. Rebuild, re-sign, and re-notarize. |
| Windows | Windows release owner | Stable CDN NSIS/MSI path and updater payload path | Bad installers are removed from stable metadata or hidden; previous signed installers still validate if retained. | If Windows installer upgrade code behavior caused the issue, test install, repair, upgrade, and uninstall before republishing. |
| Linux | Linux release owner | `.deb`, `.rpm`, `.AppImage`, package repository metadata | Bad package is yanked or repository metadata points to previous safe package; checksums and repository signatures match. | Keep direct AppImage rollback instructions separate from package-manager rollback instructions. |

## Post-Rollback Verification

| Checkpoint | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Update channel safe state | Release engineer | Stable updater CDN pointer | Older clients do not see the bad version; fixed or previous version metadata validates. | Keep updater disabled if no safe state can be confirmed. |
| Download page safe state | Release owner | Stable manual CDN release index and download page data | Bad artifacts are absent or clearly withdrawn; checksums match remaining assets. | Remove stale cache links and issue corrected notes. |
| Core manifest safe state | Release engineer | Stable empty core, geo, and SRS manifest pointers | Clients resolve previous or fixed manifests, checksums match, and core smoke passes for affected OS/arch targets. | Keep core manifest assets empty if no approved manifest can be confirmed. |
| Diagnostics safe state | Privacy/security owner | Diagnostics endpoint and release config | Diagnostics are disabled or fixed, opt-out remains honored, and no forbidden payload can be emitted. | Keep diagnostics disabled until privacy/security signs off. |
| OS state remediation | Platform owner | Affected user machines or smoke machines | Reproduction machine can restore proxy, routes, TUN devices, autostart, hotkeys, and running processes. | Publish manual remediation steps if automatic cleanup cannot be relied on. |
| Incident closeout | Release owner | Issue tracker and release notes | Root cause, affected artifacts, rollback actions, fixed version, and remaining risk are recorded. | Keep the release blocked until closeout has an owner and follow-up issue. |
