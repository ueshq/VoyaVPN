import { mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint, repoRootFromScript, run } from "../../lib/common.mjs";

/**
 * Builds `voya-mobile-ffi` for Android and drops the `.so` files where Gradle
 * expects them, plus the Kotlin bindings.
 *
 * `cargo-ndk` is what turns a Rust target triple into the NDK's clang and
 * sysroot; without it every link fails on a missing `libc`. The ABI names are
 * Android's, not Rust's, which is why they are listed rather than derived.
 */
export const ANDROID_ABIS = [
  { abi: "arm64-v8a", triple: "aarch64-linux-android" },
  { abi: "armeabi-v7a", triple: "armv7-linux-androideabi" },
  // The emulator, which is what most development runs on.
  { abi: "x86_64", triple: "x86_64-linux-android" },
];

const repoRoot = repoRootFromScript(import.meta.url);
const profile = process.env.VOYAVPN_RUST_PROFILE || "release";
const jniLibs = resolve(repoRoot, "apps", "mobile", "android", "app", "src", "main", "jniLibs");
const bindingsRoot = resolve(
  repoRoot,
  "apps",
  "mobile",
  "android",
  "app",
  "src",
  "main",
  "java",
);

function requireEnvironment() {
  for (const variable of ["ANDROID_HOME", "ANDROID_NDK_HOME"]) {
    if (!process.env[variable]) {
      throw new Error(
        `${variable} is not set. Install the Android SDK and NDK, then point ${variable} at them.`,
      );
    }
  }
}

function buildLibraries() {
  rmSync(jniLibs, { force: true, recursive: true });
  mkdirSync(jniLibs, { recursive: true });

  run(
    "cargo",
    [
      "ndk",
      ...ANDROID_ABIS.flatMap(({ abi }) => ["-t", abi]),
      "-o",
      jniLibs,
      "rustc",
      "-p",
      "voya-mobile-ffi",
      "--lib",
      "--crate-type",
      "cdylib",
      ...(profile === "release" ? ["--release"] : []),
    ],
    { cwd: repoRoot },
  );
}

/**
 * The Kotlin bindings, generated from the built library so they describe the
 * symbols it actually exports.
 */
function generateBindings() {
  const library = resolve(
    repoRoot,
    "target",
    ANDROID_ABIS[0].triple,
    profile,
    "libvoya_mobile_ffi.so",
  );

  run(
    "cargo",
    [
      "run",
      "-p",
      "voya-mobile-ffi",
      "--features",
      "bindgen",
      "--bin",
      "uniffi-bindgen",
      "--",
      "generate",
      "--library",
      library,
      "--language",
      "kotlin",
      "--out-dir",
      bindingsRoot,
    ],
    { cwd: repoRoot },
  );
}

export function buildRustForAndroid() {
  requireEnvironment();
  buildLibraries();
  generateBindings();
  console.log(`Built Android libraries in ${jniLibs}`);
  console.log(`Kotlin bindings in ${bindingsRoot}`);
}

if (isCliEntrypoint(import.meta.url)) {
  buildRustForAndroid();
}
