import { copyFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint, repoRootFromScript, run } from "../../lib/common.mjs";
import { ensureSingBoxSource, singBoxSourceDir } from "../sing-box-source.mjs";

/**
 * Builds the `libbox.aar` the Android app links.
 *
 * The same pinned checkout the Apple builds use, through sing-box's own
 * `make lib_android` (gomobile). Unlike iOS there is no slice selection to
 * make: gomobile packages every requested ABI into the one archive, and the
 * Gradle build drops the ones the chosen `abiFilters` leaves out.
 *
 * Unlike iOS there is also no extension: Android's `VpnService` runs in the
 * app's own process, so this archive and the Rust `.so` are loaded by the same
 * process and the Clash API is reachable over plain loopback.
 */
const AAR_NAME = "libbox.aar";
/** Where gomobile leaves the archive, relative to the sing-box checkout. */
const BUILT_AAR = AAR_NAME;

const repoRoot = repoRootFromScript(import.meta.url);
const sourceDir = singBoxSourceDir(repoRoot);
const libsRoot = resolve(repoRoot, "apps", "mobile", "android", "app", "libs");
const targetAar = resolve(process.env.VOYAVPN_LIBBOX_ANDROID_AAR || resolve(libsRoot, AAR_NAME));

function buildLibbox() {
  run("make", ["lib_install"], { cwd: sourceDir });
  run("make", ["lib_android"], { cwd: sourceDir });
}

/** Copies the archive into the app, where Gradle's `libs` fileTree finds it. */
export function stageLibboxAar({ builtAar = resolve(sourceDir, BUILT_AAR), destination = targetAar } = {}) {
  if (!existsSync(builtAar) || !statSync(builtAar).isFile()) {
    throw new Error(
      `${AAR_NAME} was not produced at ${builtAar}. gomobile writes it beside the sing-box checkout; check that 'make lib_android' finished.`,
    );
  }

  mkdirSync(resolve(destination, ".."), { recursive: true });
  copyFileSync(builtAar, destination);

  return destination;
}

export function buildLibboxForAndroid() {
  ensureSingBoxSource({ repoRoot, sourceDir });
  buildLibbox();
  console.log(`libbox.aar staged at ${stageLibboxAar()}`);
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    buildLibboxForAndroid();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
