import { readdirSync } from "node:fs";
import { resolve } from "node:path";

import { capture, isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";

/**
 * Parses the iOS app's own Swift.
 *
 * These sources have no host-side build: they reach React, NetworkExtension,
 * Libbox and the uniffi bindings, none of which typecheck without an Xcode
 * project and the staged frameworks. What *can* be checked anywhere with the
 * command line tools is that they parse — which catches the mistakes that are
 * otherwise found by a twenty-minute Xcode build, or by nobody until someone
 * tries to ship.
 *
 * The provider sources shared with macOS are typechecked properly by
 * `pnpm check:native:macos:bridge`; this covers the app-side half.
 */
const SOURCE_DIR = ["apps", "mobile", "ios", "VoyaVPN", "Native"];

export function swiftSources(root) {
  const directory = resolve(root, ...SOURCE_DIR);

  return readdirSync(directory)
    .filter((name) => name.endsWith(".swift"))
    .sort()
    .map((name) => resolve(directory, name));
}

export function main() {
  const root = repoRootFromScript(import.meta.url);
  if (process.platform !== "darwin") {
    // Skipped with evidence rather than silently: a green run on Linux must
    // not read as "the Swift is fine".
    console.log("SKIP iOS Swift parse: swiftc is only available on macOS.");
    return;
  }

  const sources = swiftSources(root);
  const result = capture("xcrun", ["swiftc", "-parse", ...sources], { cwd: root });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`iOS Swift sources did not parse:\n${result.stderr ?? ""}`);
  }

  console.log(`iOS Swift sources parsed: ${sources.length} files.`);
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
