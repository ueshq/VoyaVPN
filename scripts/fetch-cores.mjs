// Fetch & stage proxy core binaries into src-tauri/resources/core-seeds/<core>/ so that
// Tauri bundles them and the app's startup seed-copy installs them into app data.
//
// This script intentionally stages seed resources only. Use
// `node scripts/install-xray-core.mjs` or `pnpm core:xray:install` when the local
// app data `bin/xray/` directory also needs to be repaired.

import {
  fetchAndStageXraySeed,
  isCliEntrypoint,
  repoRootFromScript,
} from "./xray-core-installer.mjs";

async function main() {
  await fetchAndStageXraySeed({
    repoRoot: repoRootFromScript(import.meta.url),
  });
  console.log("Done. Core seeds staged for", `${process.platform}:${process.arch}`);
}

if (isCliEntrypoint(import.meta.url)) {
  await main();
}
