# VoyaVPN Third-Party Notices

This document is bundled with release packages as attribution. It is not a legal opinion, and it does not itself approve third-party binary redistribution. Stable publication must record the approval checkpoint below before the bundled sing-box seed is redistributed.

## Application

- VoyaVPN: MIT license.
- Tauri: Apache-2.0 OR MIT.
- React: MIT.
- TypeScript, Vite, Tailwind CSS, Radix UI, TanStack, Zustand, i18next, and Lucide React: bundled through npm dependencies under their published package licenses.
- Rust crates: bundled through Cargo dependencies under their published crate licenses.

## Runtime Cores

VoyaVPN debug and dry-run packages do not include proxy core binaries by default. Production stable packages include the approved sing-box seed generated during `pnpm install` or stable build preparation after the stable legal approval checkpoint is recorded. Packaged seed assets are copied from the read-only app resources into the user app data `bin/` tree before execution; VoyaVPN must not run proxy cores from the app bundle.

Stable core manifests are empty for this rollout. sing-box is distributed only as a bundled seed asset and is updated only by shipping a new application package.

| Component | Stable redistribution scope | Source and release URLs | License name | Checksum and source availability expectations |
| --- | --- | --- | --- | --- |
| sing-box | Bundled seed asset copied into app data and updated only by app package releases. It is not a core update CDN asset. | Source: https://github.com/SagerNet/sing-box. Releases: https://github.com/SagerNet/sing-box/releases. | GNU General Public License v3.0 or later (`GPL-3.0-or-later`). | Record exact version, upstream release archive URL, SHA-256, byte size, license text, and corresponding source availability for the shipped binary. If VoyaVPN modifies, rebuilds, or repackages beyond checksum-preserved redistribution, publish the corresponding source, patches, and build scripts. |

Other cores are not part of the stable seed asset approval for this rollout. They require a separate notice update, source availability plan, implementation change, and approval checkpoint before any redistribution path is added.

## Stable Approval Checkpoint

Before any stable package redistributes sing-box binaries, the release owner must attach an approval record to the stable release evidence. The approval record must include:

- Legal or release owner name, approval date, release version, and package matrix.
- Exact core names, versions, OS/architecture targets, archive names, SHA-256 values, byte sizes, and production CDN paths.
- Upstream source repository, release URL, license file URL, and source tag or source archive for each shipped binary.
- Confirmation that GPL source availability obligations for sing-box are satisfied for the exact binary shipped.
- Confirmation that no AGPL core and no unsupported core is bundled as a seed asset or published as a core update CDN asset.
- If any binary is modified or rebuilt by VoyaVPN, the published corresponding source location, patches, build scripts, and checksums.

## Release Rule

Do not add GPL or AGPL core binaries to stable packages or CDN core manifests without recorded approval, source and license attribution, checksum evidence, and source availability evidence. GPL and AGPL obligations must not be summarized as "handled by upstream" unless the approval record ties the shipped binary to an exact upstream source tag or source archive and confirms that no VoyaVPN modifications were made.

## Stable Core Redistribution Evidence Template

Complete this template in the external release evidence tracker before publishing stable packages or seed assets that redistribute sing-box binaries. This template is for approval evidence only; upstream GitHub URLs are allowed here only as source availability and attribution evidence, not as stable production download URLs consumed by clients.

### Release-Level Legal Decision

| Field | Value to record |
| --- | --- |
| Release version |  |
| Frozen commit SHA |  |
| Package matrix | Windows, macOS, and Linux x64/arm64 entries included in this release. |
| Legal or release owner |  |
| Approval record ID |  |
| Decision | `approved` or `blocked` |
| Third-party notices hash | SHA-256 of this notice file included with release packages. |
| Core manifest hash | SHA-256 of the generated stable core manifest reviewed for publication. It is expected to contain no core update assets for this rollout. |
| CDN staging evidence ID | Evidence record proving no core update CDN assets are published for this rollout. |
| Unsupported core exclusion | Evidence that no unapproved GPL, AGPL, or unsupported core is bundled as a seed asset or listed as a CDN core update asset. |
| Source availability evidence location | External evidence folder, legal tracker, or public source reference for corresponding source, license files, notices, patches, and build scripts when applicable. |
| Stop or rollback owner |  |
| Residual risk notes |  |

### Per-Core Asset Evidence

Record one row for every redistributed archive, including each OS/architecture variant and each bundled seed or CDN-delivered core asset.

| Core | Exact version | Target OS | Arch | Redistribution path | Archive name | Production CDN path or seed package path | License name/SPDX | Upstream source repository URL | Upstream release URL | Source tag or source archive URL | License file URL | sha256 | Byte size | Source availability evidence ID | GPL/MPL obligation evidence | Modified, rebuilt, or repackaged by VoyaVPN? | Approval status or blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sing-box |  |  |  | `seed` |  |  | `GPL-3.0-or-later` |  |  |  |  |  |  |  | Corresponding source availability for the exact shipped binary, license text, patches, and build scripts when modified, rebuilt, or repackaged. |  |  |

Minimum approval checks:

- Exact core versions, licenses, source URLs, source tag or archive URLs, sha256 checksums, byte sizes, source availability records, and GPL obligations are complete for every redistributed core archive.
- Legal or the release owner explicitly marks the decision as `approved` before seed assets or CDN core assets are exposed. Any `blocked` decision removes the affected core archive from stable packages and CDN manifests until corrected evidence is approved.
