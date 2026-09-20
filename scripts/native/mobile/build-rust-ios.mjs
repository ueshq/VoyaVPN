import { existsSync, mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";

import {
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
} from "../../lib/common.mjs";

/**
 * Builds `voya-mobile-ffi` for iOS and packages it as an xcframework.
 *
 * Two slices, because a build has to run on both: `aarch64-apple-ios` for a
 * device and `aarch64-apple-ios-sim` for the simulator. They cannot be merged
 * into one fat library — both are arm64, and `lipo` refuses two slices of the
 * same architecture — which is exactly what an xcframework is for.
 *
 * The Swift bindings come out of the same crate through uniffi, so the header,
 * the module map and the `.swift` file always describe the library beside them.
 */
export const IOS_TARGETS = [
  { triple: "aarch64-apple-ios", slice: "ios-arm64" },
  { triple: "aarch64-apple-ios-sim", slice: "ios-arm64-simulator" },
];

const LIBRARY_NAME = "libvoya_mobile_ffi.a";
const FRAMEWORK_NAME = "VoyaMobile.xcframework";

const repoRoot = repoRootFromScript(import.meta.url);
const profile = process.env.VOYAVPN_RUST_PROFILE || "release";
const outputRoot = resolve(repoRoot, "apps", "mobile", "ios", "Frameworks");
const bindingsRoot = resolve(repoRoot, "apps", "mobile", "ios", "VoyaVPN", "Generated");

/** Targets rustup has, so a missing one is named rather than guessed at. */
export function missingTargets(installed, wanted = IOS_TARGETS) {
  const present = new Set(installed);

  return wanted.map((target) => target.triple).filter((triple) => !present.has(triple));
}

function ensureTargets() {
  const installed = checkedCapture("rustup", ["target", "list", "--installed"])
    .stdout.split("\n")
    .map((line) => line.trim());
  const missing = missingTargets(installed);

  if (missing.length > 0) {
    throw new Error(
      `Missing Rust targets: ${missing.join(", ")}. Run: rustup target add ${missing.join(" ")}`,
    );
  }
}

function buildSlices() {
  for (const { triple } of IOS_TARGETS) {
    run(
      "cargo",
      ["build", "-p", "voya-mobile-ffi", "--target", triple, ...(profile === "release" ? ["--release"] : [])],
      { cwd: repoRoot },
    );
  }
}

function staticLibrary(triple) {
  const path = resolve(repoRoot, "target", triple, profile, LIBRARY_NAME);
  if (!existsSync(path)) {
    throw new Error(`Expected ${path} after building ${triple}`);
  }

  return path;
}

function packageFramework() {
  const framework = resolve(outputRoot, FRAMEWORK_NAME);
  rmSync(framework, { force: true, recursive: true });
  mkdirSync(outputRoot, { recursive: true });

  run(
    "xcodebuild",
    [
      "-create-xcframework",
      ...IOS_TARGETS.flatMap(({ triple }) => [
        "-library",
        staticLibrary(triple),
        "-headers",
        bindingsRoot,
      ]),
      "-output",
      framework,
    ],
    { cwd: repoRoot },
  );

  return framework;
}

/**
 * Generates the Swift bindings next to the library.
 *
 * uniffi reads the built library rather than the source, so this runs after the
 * build: the bindings then describe the very symbols the framework exports.
 */
function generateBindings() {
  rmSync(bindingsRoot, { force: true, recursive: true });
  mkdirSync(bindingsRoot, { recursive: true });

  run(
    "cargo",
    [
      "run",
      "-p",
      "voya-mobile-ffi",
      "--bin",
      "uniffi-bindgen",
      "--",
      "generate",
      "--library",
      staticLibrary(IOS_TARGETS[0].triple),
      "--language",
      "swift",
      "--out-dir",
      bindingsRoot,
    ],
    { cwd: repoRoot },
  );
}

export function buildRustForIos() {
  requireDarwin("The iOS xcframework can only be built on macOS.");
  ensureTargets();
  buildSlices();
  generateBindings();
  const framework = packageFramework();
  console.log(`Built ${framework}`);
  console.log(`Swift bindings in ${bindingsRoot}`);
}

if (isCliEntrypoint(import.meta.url)) {
  buildRustForIos();
}
