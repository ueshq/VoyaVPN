import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { repoRootFromScript } from "../../lib/common.mjs";

// The native apps carry the same distributable notices as the desktop bundle.
const root = repoRootFromScript(import.meta.url);
const target = resolve(root, "apps/mobile/src/generated/legal-notices.json");
const content = `${JSON.stringify({
  license: readFileSync(resolve(root, "LICENSE"), "utf8"),
  thirdParty: readFileSync(resolve(root, "docs/release/THIRD_PARTY_NOTICES.md"), "utf8"),
}, null, 2)}\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(target, "utf8") !== content) throw new Error("Mobile legal notices are stale: node scripts/native/mobile/legal-assets.mjs");
} else writeFileSync(target, content);
