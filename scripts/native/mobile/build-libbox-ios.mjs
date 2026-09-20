import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { resolve } from "node:path";

import {
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
} from "../../lib/common.mjs";
import { ensureSingBoxSource, singBoxSourceDir } from "../sing-box-source.mjs";

/**
 * Builds the iOS `Libbox.xcframework` the PacketTunnel extension links.
 *
 * The same `make lib_apple` the macOS build runs, from the same pinned
 * checkout: one core version across the desktop, the phone and the packaged
 * seed. What differs is which slices survive — macOS keeps only its universal
 * one, and this keeps only the iOS ones, because an xcframework carrying
 * slices nobody links is megabytes of app for nothing.
 *
 * The result stays an xcframework rather than being flattened to a framework:
 * a device slice and a simulator slice are both arm64, so no single framework
 * can hold them.
 */
const IOS_SLICE_PREFIX = "ios-";
/** The simulator slice is an iOS slice too, and both are needed. */
const SIMULATOR_MARKER = "simulator";
const XCFRAMEWORK_NAME = "Libbox.xcframework";

const repoRoot = repoRootFromScript(import.meta.url);
const sourceDir = singBoxSourceDir(repoRoot);
const frameworkRoot = resolve(repoRoot, "apps", "mobile", "ios", "Frameworks");
const targetXCFramework = resolve(
  process.env.VOYAVPN_LIBBOX_IOS_XCFRAMEWORK || resolve(frameworkRoot, XCFRAMEWORK_NAME),
);

function buildLibbox() {
  rmSync(resolve(sourceDir, XCFRAMEWORK_NAME), { force: true, recursive: true });
  run("make", ["lib_install"], { cwd: sourceDir });
  run("make", ["lib_apple"], { cwd: sourceDir });
}

/**
 * The slices an iOS app needs: one device, one simulator.
 *
 * Named by what they are rather than matched exactly, because the simulator
 * slice's directory name carries its architectures
 * (`ios-arm64_x86_64-simulator` today, `ios-arm64-simulator` on an
 * Apple-silicon-only build) and pinning the spelling would break on the next
 * toolchain.
 */
export function iosSlices(xcframeworkPath) {
  const slices = readdirSync(xcframeworkPath, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.toLowerCase().startsWith(IOS_SLICE_PREFIX))
    .map((entry) => entry.name)
    .sort();
  const simulator = slices.filter((name) => name.toLowerCase().includes(SIMULATOR_MARKER));
  const device = slices.filter((name) => !name.toLowerCase().includes(SIMULATOR_MARKER));

  if (device.length === 0 || simulator.length === 0) {
    throw new Error(
      `Libbox.xcframework must carry an iOS device slice and a simulator slice; found: ${
        slices.length > 0 ? slices.join(", ") : "none"
      }`,
    );
  }

  return [...device, ...simulator];
}

/**
 * Copies the iOS slices into the app, dropping everything else.
 *
 * `Info.plist` comes along and is rewritten to list exactly the slices that
 * were kept: Xcode reads it to pick one, and a plist promising a macOS slice
 * that is not there fails the build rather than being ignored.
 */
export function stageLibboxXCFramework({
  outputXCFramework = resolve(sourceDir, XCFRAMEWORK_NAME),
  destination = targetXCFramework,
  plist = plutil,
} = {}) {
  if (!existsSync(outputXCFramework) || !statSync(outputXCFramework).isDirectory()) {
    throw new Error(`Libbox.xcframework was not produced at ${outputXCFramework}`);
  }
  const slices = iosSlices(outputXCFramework);

  rmSync(destination, { force: true, recursive: true });
  mkdirSync(destination, { recursive: true });
  for (const slice of slices) {
    cpSync(resolve(outputXCFramework, slice), resolve(destination, slice), {
      dereference: false,
      force: true,
      recursive: true,
      verbatimSymlinks: true,
    });
  }
  cpSync(resolve(outputXCFramework, "Info.plist"), resolve(destination, "Info.plist"), {
    force: true,
  });
  pruneInfoPlist(destination, slices, plist);

  rmSync(outputXCFramework, { force: true, recursive: true });

  return slices;
}

/**
 * The `plutil` calls, as one injectable surface.
 *
 * Reading and removing rather than rewriting the whole plist: the file also
 * carries the format version and the package type, which are sing-box's to
 * decide, not this script's.
 */
const plutil = {
  read: (path, keyPath) =>
    checkedCapture("plutil", ["-extract", keyPath, "raw", "-o", "-", path]).stdout.trim(),
  remove: (path, keyPath) => {
    checkedCapture("plutil", ["-remove", keyPath, path]);
  },
};

/** Drops every `AvailableLibraries` entry whose slice directory was not kept. */
function pruneInfoPlist(destination, slices, plist) {
  const path = resolve(destination, "Info.plist");
  const kept = new Set(slices);
  const entries = Number(plist.read(path, "AvailableLibraries"));
  if (!Number.isInteger(entries)) {
    throw new Error(`Libbox.xcframework Info.plist has no AvailableLibraries array: ${path}`);
  }
  // Walked back to front: removing an entry renumbers every one after it.
  for (let index = entries - 1; index >= 0; index -= 1) {
    const identifier = plist.read(path, `AvailableLibraries.${index}.LibraryIdentifier`);
    if (!kept.has(identifier)) {
      plist.remove(path, `AvailableLibraries.${index}`);
    }
  }
}

export function buildLibboxForIos() {
  requireDarwin("Libbox.xcframework must be built on macOS with Xcode command line tools.");
  ensureSingBoxSource({ repoRoot, sourceDir });
  buildLibbox();
  const slices = stageLibboxXCFramework();
  console.log(`Libbox.xcframework staged at ${targetXCFramework} with slices: ${slices.join(", ")}`);
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    buildLibboxForIos();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
