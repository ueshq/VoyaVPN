import { existsSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint, repoRootFromScript, run, runCli } from "../../lib/common.mjs";
import { podsUpToDate, recordInstalledPods } from "./ios-pods-cache.mjs";

/**
 * `vp run dev ios` / `vp run dev android`: the mobile app in Debug, on the
 * simulator or the Android emulator/device, served by Metro.
 *
 * The Debug build links the same native artifacts a release does, and they are
 * gitignored build outputs, so a fresh checkout has none. Each one is built
 * here only when it is missing: the Rust backend is the one that changes with
 * the code, so `--rebuild-rust` rebuilds it on request; Libbox is pinned and
 * only ever built once. Every other argument goes to `react-native run-ios` /
 * `run-android` (`vp run dev ios --simulator "iPhone 17"`).
 *
 * The Rust backend uses the `mobile-smoke` profile unless `VOYAVPN_RUST_PROFILE`
 * names another: fat LTO is most of a release build's wait, and nothing here
 * measures the backend.
 */

const OWN_FLAGS = ["--rebuild-rust"];

/** Splits this script's own flags from those passed on to React Native. */
export function parseDevArgs(argv) {
  // `vp run dev ios -- --simulator X` passes the `--` through.
  const args = argv.filter((arg) => arg !== "--");
  return {
    rebuildRust: args.includes("--rebuild-rust"),
    reactNativeArgs: args.filter((arg) => !OWN_FLAGS.includes(arg)),
  };
}

/** Whether an xcframework, given its directory entries, has a simulator slice. */
export function hasSimulatorSlice(entries) {
  return entries !== null && entries.some((name) => name.startsWith("ios-") && name.includes("simulator"));
}

function xcframeworkEntries(path) {
  return existsSync(path) ? readdirSync(path) : null;
}

const ANDROID_ABIS = ["arm64-v8a", "x86_64"];

/** The Android native artifacts Gradle links, as paths that must exist. */
export function androidArtifactPaths(androidRoot) {
  const main = resolve(androidRoot, "app/src/main");
  return {
    libbox: [resolve(androidRoot, "app/libs/libbox.aar")],
    rust: [
      ...ANDROID_ABIS.map((abi) => resolve(main, "jniLibs", abi, "libvoya_mobile_ffi.so")),
      resolve(main, "java/uniffi/voya_mobile_ffi"),
    ],
  };
}

function rustEnv() {
  return { ...process.env, VOYAVPN_RUST_PROFILE: process.env.VOYAVPN_RUST_PROFILE || "mobile-smoke" };
}

function devIos(root, { rebuildRust, reactNativeArgs }) {
  if (process.platform !== "darwin") {
    throw new Error("vp run dev ios needs macOS with Xcode and CocoaPods.");
  }
  const ios = resolve(root, "apps/mobile/ios");
  const frameworks = resolve(ios, "Frameworks");

  if (rebuildRust || !hasSimulatorSlice(xcframeworkEntries(resolve(frameworks, "VoyaMobile.xcframework")))) {
    run("rustup", ["target", "add", "aarch64-apple-ios-sim"]);
    run("vp", ["run", "native", "mobile", "rust", "ios", "--slice", "simulator"], { cwd: root, env: rustEnv() });
  }
  if (!hasSimulatorSlice(xcframeworkEntries(resolve(frameworks, "Libbox.xcframework")))) {
    run("vp", ["run", "native", "mobile", "libbox", "ios"], { cwd: root });
  }
  if (!podsUpToDate(root)) {
    // CocoaPods crashes outright under a non-UTF-8 locale, which an editor's
    // task runner is apt to start it with.
    run("pod", ["install"], { cwd: ios, env: { ...process.env, LANG: process.env.LANG || "en_US.UTF-8" } });
    recordInstalledPods(root);
  }
  // Idempotent and quick; `pod install` and a pull both rewrite parts of the
  // project it owns.
  run("vp", ["run", "native", "mobile", "ios", "project"], { cwd: root });
  run("vp", ["run", "--filter", "@voya/mobile", "ios", ...reactNativeArgs], { cwd: root });
}

function devAndroid(root, { rebuildRust, reactNativeArgs }) {
  if (!process.env.ANDROID_HOME) {
    throw new Error("ANDROID_HOME is not set. Install the Android SDK and point ANDROID_HOME at it.");
  }
  const artifacts = androidArtifactPaths(resolve(root, "apps/mobile/android"));
  const missing = (paths) => paths.some((path) => !existsSync(path));

  if (rebuildRust || missing(artifacts.rust)) {
    run("rustup", ["target", "add", "aarch64-linux-android", "x86_64-linux-android"]);
    run("vp", ["run", "native", "mobile", "rust", "android"], { cwd: root, env: rustEnv() });
  }
  if (missing(artifacts.libbox)) {
    run("vp", ["run", "native", "mobile", "libbox", "android"], { cwd: root });
  }
  run("vp", ["run", "--filter", "@voya/mobile", "android", ...reactNativeArgs], { cwd: root });
}

export function main([platform, ...argv] = process.argv.slice(2)) {
  const root = repoRootFromScript(import.meta.url);
  const options = parseDevArgs(argv);
  if (platform === "ios") devIos(root, options);
  else if (platform === "android") devAndroid(root, options);
  else throw new Error(`Usage: node scripts/native/mobile/dev.mjs <ios|android> [--rebuild-rust] [react-native args]`);
}

if (isCliEntrypoint(import.meta.url)) {
  runCli(main);
}
