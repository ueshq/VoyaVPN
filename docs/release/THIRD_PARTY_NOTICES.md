# VoyaVPN Third-Party Notices

This document is bundled with beta packages as release attribution. It is not a legal opinion and must be reviewed before public redistribution changes.

## Application

- VoyaVPN: MIT license.
- Tauri: Apache-2.0 OR MIT.
- React: MIT.
- TypeScript, Vite, Tailwind CSS, Radix UI, TanStack, Zustand, i18next, and Lucide React: bundled through npm dependencies under their published package licenses.
- Rust crates: bundled through Cargo dependencies under their published crate licenses.

## Runtime Cores

VoyaVPN installers do not include proxy core binaries by default. The app downloads or discovers cores in the user app data `bin/` tree.

| Component | Current default acquisition | License note |
| --- | --- | --- |
| Xray | Download on first run or user supplied | MPL-2.0 upstream license. |
| sing-box | Download on first run or user supplied | GPL-3.0 upstream license; not bundled by default. |
| mihomo | Download on first run or user supplied | GPL-3.0 upstream license; not bundled by default. |
| juicity | Download on first run or user supplied | AGPL-3.0 upstream license; not bundled by default. |
| Other supported cores | Download on first run or user supplied | Review each upstream license before redistribution. |

## Release Rule

Do not add GPL or AGPL core binaries to the default installer payload. Any exception requires a separate release profile, recorded approval, source and license attribution, and platform package evidence.
