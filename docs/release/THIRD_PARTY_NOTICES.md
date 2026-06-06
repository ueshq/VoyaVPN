# VoyaVPN Third-Party Notices

This document is bundled with release packages as attribution. It is not a legal opinion, and it does not itself approve third-party binary redistribution. Stable publication must record the approval checkpoint below before Xray, mihomo, or sing-box seed assets or CDN core assets are redistributed.

## Application

- VoyaVPN: MIT license.
- Tauri: Apache-2.0 OR MIT.
- React: MIT.
- TypeScript, Vite, Tailwind CSS, Radix UI, TanStack, Zustand, i18next, and Lucide React: bundled through npm dependencies under their published package licenses.
- Rust crates: bundled through Cargo dependencies under their published crate licenses.

## Runtime Cores

VoyaVPN debug and dry-run packages do not include proxy core binaries by default. Production stable packages may include only the approved Xray, mihomo, and sing-box seed assets after the stable legal approval checkpoint is recorded. Packaged seed assets are copied from the read-only app resources into the user app data `bin/` tree before execution; VoyaVPN must not run proxy cores from the app bundle.

Stable core manifests record CDN-delivered archives separately from bundled seed assets and app updater payloads. Each core manifest entry must retain an upstream source URL for attribution and license review; that upstream URL is not the production download URL consumed by stable clients.

| Component | Stable redistribution scope | Source and release URLs | License name | Checksum and source availability expectations |
| --- | --- | --- | --- | --- |
| Xray | Optional bundled seed asset and core update CDN asset for the first stable matrix only. | Source: https://github.com/XTLS/Xray-core. Releases: https://github.com/XTLS/Xray-core/releases. | Mozilla Public License 2.0 (`MPL-2.0`). | Record exact version, upstream release archive URL, SHA-256, byte size, and source tag or source archive. If VoyaVPN modifies or rebuilds the binary, publish the corresponding source changes and build scripts with the release evidence. |
| mihomo | Optional bundled seed asset and core update CDN asset for the first stable matrix only. | Source branch: https://github.com/MetaCubeX/mihomo/tree/Meta. Releases: https://github.com/MetaCubeX/mihomo/releases. | GNU General Public License v3.0 (`GPL-3.0`). | Record exact version, upstream release archive URL, SHA-256, byte size, license text, and corresponding source availability for the shipped binary. If VoyaVPN modifies, rebuilds, or repackages beyond checksum-preserved redistribution, publish the corresponding source, patches, and build scripts. |
| sing-box | Optional bundled seed asset and core update CDN asset for the first stable matrix only. | Source: https://github.com/SagerNet/sing-box. Releases: https://github.com/SagerNet/sing-box/releases. | GNU General Public License v3.0 or later (`GPL-3.0-or-later`). | Record exact version, upstream release archive URL, SHA-256, byte size, license text, and corresponding source availability for the shipped binary. If VoyaVPN modifies, rebuilds, or repackages beyond checksum-preserved redistribution, publish the corresponding source, patches, and build scripts. |

Unsupported or out-of-scope cores, including juicity and other AGPL/GPL cores not listed above, are not part of the stable seed asset approval for this rollout. They must remain user-supplied or download-on-first-run until a separate notice update, source availability plan, and approval checkpoint are completed.

## Stable Approval Checkpoint

Before any stable package or CDN core manifest redistributes Xray, mihomo, or sing-box binaries, the release owner must attach an approval record to the stable release evidence. The approval record must include:

- Legal or release owner name, approval date, release version, and package matrix.
- Exact core names, versions, OS/architecture targets, archive names, SHA-256 values, byte sizes, and production CDN paths.
- Upstream source repository, release URL, license file URL, and source tag or source archive for each shipped binary.
- Confirmation that GPL source availability obligations for mihomo and sing-box are satisfied for the exact binaries shipped.
- Confirmation that no AGPL core and no unsupported core is bundled as a seed asset or published as a first-stable core update CDN asset.
- If any binary is modified or rebuilt by VoyaVPN, the published corresponding source location, patches, build scripts, and checksums.

## Release Rule

Do not add GPL, AGPL, or MPL core binaries to stable packages or CDN core manifests without recorded approval, source and license attribution, checksum evidence, and source availability evidence. GPL and AGPL obligations must not be summarized as "handled by upstream" unless the approval record ties the shipped binary to an exact upstream source tag or source archive and confirms that no VoyaVPN modifications were made.
