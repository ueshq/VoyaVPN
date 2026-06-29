// Fetch & stage the sing-box proxy core into src-tauri/resources/core-seeds/sing_box/
// so that Tauri bundles it and the app's startup seed-copy installs it into app data.
//
// This script intentionally stages seed resources only. Use
// `node scripts/install-sing-box-core.mjs` or `pnpm core:sing-box:install` when the
// local app data `bin/sing_box/` directory also needs to be repaired.

import {
  fetchAndStageSingBoxSeed,
  isCliEntrypoint,
  repoRootFromScript,
} from "./sing-box-core-installer.mjs";

async function main() {
  await fetchAndStageSingBoxSeed({
    repoRoot: repoRootFromScript(import.meta.url),
  });
  console.log("Done. Core seeds staged for", `${process.platform}:${process.arch}`);
}

if (isCliEntrypoint(import.meta.url)) {
  await main();
}
