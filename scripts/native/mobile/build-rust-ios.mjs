import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import {
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
} from "../../lib/common.mjs";
import { parseArgs } from "../../lib/args.mjs";
import { generateUniffiBindings, rustProfile } from "./uniffi-bindings.mjs";

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
  { triple: "aarch64-apple-ios", slice: "ios-arm64", runsOn: "device" },
  { triple: "aarch64-apple-ios-sim", slice: "ios-arm64-simulator", runsOn: "simulator" },
];

/**
 * The slices a build asked for: `--slice device`, `--slice simulator`, or
 * both when it says nothing.
 *
 * A lane that only ever runs on one of them — the simulator smoke test, the
 * App Store archive — otherwise pays for a release build of the other, which
 * is the whole dependency graph again for a library nothing will load.
 */
export function selectIosTargets(argv, targets = IOS_TARGETS) {
  // Through the shared reader, so `--slice=device` works and a misspelt flag
  // is an error rather than a silent build of both slices.
  const { slice: wanted } = parseArgs(
    argv.filter((argument) => argument !== "--"),
    { "--slice": { key: "slice" } },
  );
  if (wanted === undefined) return targets;
  const selected = targets.filter((target) => target.runsOn === wanted);
  if (selected.length === 0) {
    throw new Error(`--slice takes "device" or "simulator", not ${JSON.stringify(wanted)}`);
  }

  return selected;
}

const LIBRARY_NAME = "libvoya_mobile_ffi.a";
const FRAMEWORK_NAME = "VoyaMobile.xcframework";

const repoRoot = repoRootFromScript(import.meta.url);
const { directory: profileDir, profile } = rustProfile();
const outputRoot = resolve(repoRoot, "apps", "mobile", "ios", "Frameworks");
const bindingsRoot = resolve(repoRoot, "apps", "mobile", "ios", "VoyaVPN", "Generated");

/** Targets rustup has, so a missing one is named rather than guessed at. */
export function missingTargets(installed, wanted = IOS_TARGETS) {
  const present = new Set(installed);

  return wanted.map((target) => target.triple).filter((triple) => !present.has(triple));
}

function ensureTargets(targets) {
  const installed = checkedCapture("rustup", ["target", "list", "--installed"])
    .stdout.split("\n")
    .map((line) => line.trim());
  const missing = missingTargets(installed, targets);

  if (missing.length > 0) {
    throw new Error(
      `Missing Rust targets: ${missing.join(", ")}. Run: rustup target add ${missing.join(" ")}`,
    );
  }
}

function buildSlices(targets) {
  for (const { triple } of targets) {
    run(
      "cargo",
      [
        "rustc",
        "-p",
        "voya-mobile-ffi",
        "--lib",
        "--target",
        triple,
        "--crate-type",
        "staticlib",
        "--profile",
        profile,
      ],
      { cwd: repoRoot },
    );
  }
}

function staticLibrary(triple) {
  const path = resolve(repoRoot, "target", triple, profileDir, LIBRARY_NAME);
  if (!existsSync(path)) {
    throw new Error(`Expected ${path} after building ${triple}`);
  }

  return path;
}

function packageFramework(targets) {
  const framework = resolve(outputRoot, FRAMEWORK_NAME);
  rmSync(framework, { force: true, recursive: true });
  mkdirSync(outputRoot, { recursive: true });

  // Libraries only, no `-headers`. The app compiles the generated Swift and
  // reads the C header from where the bindings are written (the project's
  // `HEADER_SEARCH_PATHS` and bridging header point there), so a copy inside
  // the framework would be a second one that nothing reads.
  run(
    "xcodebuild",
    [
      "-create-xcframework",
      ...targets.flatMap(({ triple }) => ["-library", staticLibrary(triple)]),
      "-output",
      framework,
    ],
    { cwd: repoRoot },
  );

  return framework;
}

/**
 * Replaces `directory` with `fresh`, leaving alone every file whose bytes are
 * already there. Xcode recompiles what a newer timestamp touches, and the
 * bridging header imports these: rewriting identical bindings rebuilt the app's
 * Swift after every backend build.
 */
export function syncGeneratedFiles(fresh, directory) {
  mkdirSync(directory, { recursive: true });
  const wanted = new Set(readdirSync(fresh));
  for (const name of readdirSync(directory)) {
    if (!wanted.has(name)) rmSync(resolve(directory, name), { force: true, recursive: true });
  }
  for (const name of wanted) {
    const target = resolve(directory, name);
    const content = readFileSync(resolve(fresh, name));
    if (!existsSync(target) || !content.equals(readFileSync(target))) {
      writeFileSync(target, content);
    }
  }
}

/** The Swift bindings, regenerated from scratch next to the library. */
function generateBindings(targets) {
  const fresh = mkdtempSync(resolve(tmpdir(), "voya-ios-bindings-"));
  try {
    generateUniffiBindings({
      language: "swift",
      // Any slice describes the same interface.
      library: staticLibrary(targets[0].triple),
      outDir: fresh,
      repoRoot,
    });
    syncGeneratedFiles(fresh, bindingsRoot);
  } finally {
    rmSync(fresh, { force: true, recursive: true });
  }
}

function buildRustForIos(targets = IOS_TARGETS) {
  requireDarwin("The iOS xcframework can only be built on macOS.");
  ensureTargets(targets);
  buildSlices(targets);
  generateBindings(targets);
  const framework = packageFramework(targets);
  console.log(`Built ${framework} (${targets.map((target) => target.slice).join(", ")})`);
  console.log(`Swift bindings in ${bindingsRoot}`);
}

if (isCliEntrypoint(import.meta.url)) {
  buildRustForIos(selectIosTargets(process.argv.slice(2)));
}
