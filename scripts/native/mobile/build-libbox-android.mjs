import { copyFileSync, existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint, repoRootFromScript, run, runCli } from "../../lib/common.mjs";
import { ensureSingBoxSource, installLibboxTools, singBoxSourceDir } from "../sing-box-source.mjs";

/**
 * Builds the `libbox.aar` the Android app links.
 *
 * The same pinned checkout the Apple builds use, through the tool behind
 * sing-box's own `make lib_android` (gomobile). gomobile packages every
 * requested ABI into the one archive, so the ABIs are named here: the default
 * is all four, and the Gradle build's `abiFilters` keeps two.
 *
 * Unlike iOS there is also no extension: Android's `VpnService` runs in the
 * app's own process, so this archive and the Rust `.so` are loaded by the same
 * process and the Clash API is reachable over plain loopback.
 */
const AAR_NAME = "libbox.aar";
/** Where gomobile leaves the archive, relative to the sing-box checkout. */
const BUILT_AAR = AAR_NAME;
/**
 * The ABIs `abiFilters` in `apps/mobile/android/app/build.gradle` keeps: a
 * phone and the emulator. `make lib_android` compiles sing-box for arm and
 * 386 as well, for each of the two archives it produces, and Gradle throws
 * those away. The legacy archive has no switch of its own, so it is still
 * built — for these two ABIs instead of four.
 */
const ANDROID_PLATFORMS = "android/arm64,android/amd64";

const repoRoot = repoRootFromScript(import.meta.url);
const sourceDir = singBoxSourceDir(repoRoot);
const libsRoot = resolve(repoRoot, "apps", "mobile", "android", "app", "libs");
const targetAar = resolve(process.env.VOYAVPN_LIBBOX_ANDROID_AAR || resolve(libsRoot, AAR_NAME));

function buildLibbox() {
  installLibboxTools(sourceDir);
  // gomobile leaves the archive in the checkout and the staging step copies
  // it out, so one from an earlier build would satisfy the "was it produced"
  // check after a build that produced nothing.
  rmSync(resolve(sourceDir, BUILT_AAR), { force: true });
  run("go", ["run", "./cmd/internal/build_libbox", "-target", "android", "-platform", ANDROID_PLATFORMS], {
    cwd: sourceDir,
  });
}

/** Copies the archive into the app, where Gradle's `libs` fileTree finds it. */
export function stageLibboxAar({ builtAar = resolve(sourceDir, BUILT_AAR), destination = targetAar } = {}) {
  if (!existsSync(builtAar) || !statSync(builtAar).isFile()) {
    throw new Error(
      `${AAR_NAME} was not produced at ${builtAar}. gomobile writes it beside the sing-box checkout; check that the build finished.`,
    );
  }

  mkdirSync(resolve(destination, ".."), { recursive: true });
  copyFileSync(builtAar, destination);

  return destination;
}

function buildLibboxForAndroid() {
  ensureSingBoxSource({ repoRoot, sourceDir });
  buildLibbox();
  console.log(`libbox.aar staged at ${stageLibboxAar()}`);
}

if (isCliEntrypoint(import.meta.url)) {
  runCli(buildLibboxForAndroid);
}
