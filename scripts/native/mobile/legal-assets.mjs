import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { isCliEntrypoint, repoRootFromScript } from "../../lib/common.mjs";

// The native apps carry the same distributable notices as the desktop bundle.
const root = repoRootFromScript(import.meta.url);
const target = resolve(root, "apps/mobile/src/generated/legal-notices.json");

function legalNotices() {
  return `${JSON.stringify({
    license: readFileSync(resolve(root, "LICENSE"), "utf8"),
    thirdParty: readFileSync(resolve(root, "docs/release/THIRD_PARTY_NOTICES.md"), "utf8"),
  }, null, 2)}\n`;
}

/** Throws when the generated mobile notices lag LICENSE or the third-party notices. */
export function checkMobileLegalAssets() {
  if (readFileSync(target, "utf8") !== legalNotices()) {
    throw new Error("Mobile legal notices are stale: node scripts/native/mobile/legal-assets.mjs");
  }
}

if (isCliEntrypoint(import.meta.url)) {
  if (process.argv.includes("--check")) checkMobileLegalAssets();
  else writeFileSync(target, legalNotices());
}
