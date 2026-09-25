import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { sha256Text } from "../../lib/fs.mjs";

const inputs = ["pnpm-lock.yaml", "apps/mobile/package.json", "apps/mobile/ios/Podfile", "apps/mobile/ios/Podfile.lock"];
const stamp = "apps/mobile/ios/Pods/.voya-inputs-sha256";
function fingerprint(root) {
  return sha256Text(Buffer.concat(inputs.flatMap((path) => [
    Buffer.from(path),
    existsSync(resolve(root, path)) ? readFileSync(resolve(root, path)) : Buffer.from("missing"),
  ])));
}
function installed(root) {
  const lock = resolve(root, "apps/mobile/ios/Podfile.lock");
  const manifest = resolve(root, "apps/mobile/ios/Pods/Manifest.lock");
  return existsSync(lock) && existsSync(manifest) &&
    existsSync(resolve(root, "apps/mobile/ios/Pods/Pods.xcodeproj/project.pbxproj")) &&
    readFileSync(lock).equals(readFileSync(manifest));
}
export function podsUpToDate(root) {
  return installed(root) && existsSync(resolve(root, stamp)) && readFileSync(resolve(root, stamp), "utf8") === fingerprint(root);
}
/** Call only after pod install has exited successfully. */
export function recordInstalledPods(root) {
  if (!installed(root)) throw new Error("pod install did not produce a consistent lock and project");
  writeFileSync(resolve(root, stamp), fingerprint(root));
}
