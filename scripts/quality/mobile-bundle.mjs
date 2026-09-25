import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { repoRootFromScript, run } from "../lib/common.mjs";
import { checkMobileLegalAssets } from "../native/mobile/legal-assets.mjs";

/**
 * Bundles the mobile app for both platforms.
 *
 * This is the check that a pnpm workspace makes necessary: typecheck and Jest
 * both resolve modules the Node way, while the app is built by Metro, whose
 * resolver is configured separately. A `@voya/*` package that imports fine in
 * an editor and fails on a device is the failure this catches, and it is not
 * hypothetical — Metro resolves workspace symlinks, `exports` maps and
 * TypeScript sources under `watchFolders` by its own rules.
 */
const repoRoot = repoRootFromScript(import.meta.url);
const mobileRoot = resolve(repoRoot, "apps/mobile");
const outputDir = mkdtempSync(join(tmpdir(), "voyavpn-mobile-bundle-"));

try {
  checkMobileLegalAssets();
  for (const platform of ["ios", "android"]) {
    run(
      "pnpm",
      [
        "exec",
        "react-native",
        "bundle",
        "--platform",
        platform,
        "--dev",
        "false",
        "--entry-file",
        "index.js",
        "--bundle-output",
        join(outputDir, `${platform}.jsbundle`),
        "--assets-dest",
        join(outputDir, platform),
      ],
      { cwd: mobileRoot },
    );
  }

  console.log("Mobile bundles built for ios and android.");
} finally {
  rmSync(outputDir, { force: true, recursive: true });
}
