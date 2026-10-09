import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";

import { capture, checkedCapture, isCliEntrypoint, repoRootFromScript, run, runCli } from "../../lib/common.mjs";
import { readJson } from "../../lib/fs.mjs";
import { resolveStoreBuildNumber } from "../../tauri/mac-app-store-config.mjs";
import { androidArtifactPaths } from "./dev.mjs";

/**
 * `vp run build android`: the production APK distributed outside Google Play,
 * signed with the release key.
 *
 *   --reuse-libbox  Keep the staged libbox.aar instead of rebuilding it.
 *   --skip-native   Skip the Rust and Libbox steps; link what is staged.
 *
 * The key comes from four environment variables, which `app/build.gradle`
 * reads; without them the release build type falls back to the checked-in
 * debug keystore, so this lane refuses to start instead. The version code is
 * `VOYAVPN_ANDROID_VERSION_CODE`, or the commit count like the store lanes'
 * build numbers, so every build from a later commit installs over an earlier
 * one. See docs/release/mobile-android-signing.md.
 */

const applicationId = "app.voyavpn.mobile";
const abis = ["arm64-v8a", "x86_64"];
const nativeLibraries = ["libvoya_mobile_ffi.so", "libbox.so"];
// Android's ceiling for versionCode.
const maxVersionCode = 2_100_000_000;

export const signingVariables = [
  "VOYAVPN_ANDROID_KEYSTORE",
  "VOYAVPN_ANDROID_KEYSTORE_PASSWORD",
  "VOYAVPN_ANDROID_KEY_ALIAS",
  "VOYAVPN_ANDROID_KEY_PASSWORD",
];

/** What stops `env` from signing a release, as messages. */
export function signingProblems(env, fileExists = existsSync) {
  const missing = signingVariables.filter((name) => !env[name]?.trim());
  if (missing.length > 0) return [`${missing.join(", ")} ${missing.length === 1 ? "is" : "are"} not set.`];
  const keystore = env.VOYAVPN_ANDROID_KEYSTORE.trim();
  return fileExists(keystore) ? [] : [`VOYAVPN_ANDROID_KEYSTORE points at ${keystore}, which does not exist.`];
}

export function resolveAndroidVersionCode({ env = process.env, repoRoot, captureCommand = capture } = {}) {
  const code = resolveStoreBuildNumber({ envName: "VOYAVPN_ANDROID_VERSION_CODE", env, repoRoot, captureCommand });
  if (!/^[1-9]\d*$/u.test(code) || Number(code) > maxVersionCode) {
    throw new Error(`The Android versionCode must be an integer from 1 to ${maxVersionCode} (got "${code}").`);
  }
  return code;
}

/** A digest without separators, in upper case: how keytool and apksigner print it differs. */
function normalizeDigest(digest) {
  return digest.replaceAll(":", "").toUpperCase();
}

/**
 * The first signer's DN and SHA-256 digest from `apksigner verify --print-certs`.
 * Build-tools 37 labels it `V2 Signer:`, older ones `Signer #1`.
 */
export function parseApkSigner(output) {
  const field = (name) =>
    output.match(new RegExp(`^(?:Signer #1|V\\d+ Signer:) certificate ${name}: (.+)$`, "mu"))?.[1]?.trim() ?? null;
  return { dn: field("DN"), sha256: field("SHA-256 digest") };
}

/** The SHA-256 fingerprint of the key's certificate from `keytool -list -v`. */
export function parseKeytoolSha256(output) {
  return output.match(/^\s*SHA256:\s*([0-9A-F:]+)\s*$/imu)?.[1] ?? null;
}

/**
 * What is wrong with a built APK. `badging` is `aapt2 dump badging`, `signer`
 * is {@link parseApkSigner}'s result, `keySha256` the release key's
 * certificate digest, and `entries` the archive's file list.
 */
export function apkProblems({ badging, signer, keySha256, entries, version, versionCode }) {
  const problems = [];
  const pkg = badging.match(/^package: name='([^']*)' versionCode='([^']*)' versionName='([^']*)'/mu);
  if (!pkg) {
    problems.push("aapt2 printed no package line.");
  } else {
    const [, name, code, versionName] = pkg;
    if (name !== applicationId) problems.push(`The package is ${name}, expected ${applicationId}.`);
    if (code !== versionCode) problems.push(`versionCode is ${code}, expected ${versionCode}.`);
    if (versionName !== version) problems.push(`versionName is ${versionName}, expected ${version}.`);
  }
  if (/^application-debuggable$/mu.test(badging)) problems.push("The APK is debuggable.");
  if (!signer.sha256) {
    problems.push("apksigner reported no signer.");
  } else if (!keySha256 || normalizeDigest(signer.sha256) !== normalizeDigest(keySha256)) {
    problems.push(`The APK is signed by ${signer.dn}, not by the release key.`);
  }
  for (const abi of abis) {
    for (const library of nativeLibraries) {
      if (!entries.includes(`lib/${abi}/${library}`)) problems.push(`lib/${abi}/${library} is missing.`);
    }
  }
  return problems;
}

export function resolveApkPath({ outputDir, version, versionCode }) {
  return resolve(outputDir, `VoyaVPN_${version}_${versionCode}.apk`);
}

function throwProblems(title, problems) {
  if (problems.length > 0) {
    throw new Error(`${title}:\n${problems.map((line) => `  - ${line}`).join("\n")}`);
  }
}

/** The newest installed build-tools directory, where apksigner and aapt2 live. */
function buildTools(androidHome) {
  const root = resolve(androidHome, "build-tools");
  const newest = existsSync(root)
    ? readdirSync(root).sort((a, b) => b.localeCompare(a, undefined, { numeric: true }))[0]
    : undefined;
  if (!newest) throw new Error(`No Android build-tools under ${root}; install them with the SDK Manager.`);
  return resolve(root, newest);
}

export function main(argv = process.argv.slice(2)) {
  const flags = ["--reuse-libbox", "--skip-native"];
  const unknown = argv.filter((flag) => flag !== "--" && !flags.includes(flag));
  if (unknown.length > 0) throw new Error(`Unknown option ${unknown.join(" ")}.`);
  const skipNative = argv.includes("--skip-native");
  const reuseLibbox = argv.includes("--reuse-libbox");
  if (process.platform === "win32") {
    throw new Error("vp run build android runs on macOS or Linux.");
  }

  // --- 1. preflight -------------------------------------------------------------
  const root = repoRootFromScript(import.meta.url);
  const androidRoot = resolve(root, "apps/mobile/android");
  const androidHome = process.env.ANDROID_HOME?.trim();
  if (!androidHome) {
    throw new Error("ANDROID_HOME is not set. Install the Android SDK and point ANDROID_HOME at it.");
  }
  const tools = buildTools(androidHome);
  throwProblems("Release signing", signingProblems(process.env));
  const keySha256 = parseKeytoolSha256(
    checkedCapture("keytool", [
      "-list",
      "-v",
      "-keystore",
      process.env.VOYAVPN_ANDROID_KEYSTORE.trim(),
      "-alias",
      process.env.VOYAVPN_ANDROID_KEY_ALIAS.trim(),
      // `:env` keeps the password off the command line.
      "-storepass:env",
      "VOYAVPN_ANDROID_KEYSTORE_PASSWORD",
    ]).stdout,
  );
  if (!keySha256) throw new Error("keytool printed no SHA-256 fingerprint for the release key.");

  const version = readJson(resolve(root, "package.json")).version;
  const versionCode = resolveAndroidVersionCode({ repoRoot: root });
  console.log(`VoyaVPN ${version} (${versionCode}) for Android`);
  if (capture("git", ["status", "--porcelain"], { cwd: root }).stdout?.trim()) {
    console.warn("! The working tree has uncommitted changes; the version code counts commits only.");
  }

  // --- 2. native artifacts -------------------------------------------------------
  const artifacts = androidArtifactPaths(androidRoot);
  if (skipNative) {
    const missing = [...artifacts.rust, ...artifacts.libbox].filter((path) => !existsSync(path));
    throwProblems(
      "--skip-native needs the staged native artifacts",
      missing.map((path) => `${path} is missing`),
    );
  } else {
    run("rustup", ["target", "add", "aarch64-linux-android", "x86_64-linux-android"]);
    // Always the release profile: a `VOYAVPN_RUST_PROFILE` left exported from
    // an iteration loop would otherwise ship an unoptimised backend.
    run("vp", ["run", "native", "mobile", "rust", "android"], {
      cwd: root,
      env: { ...process.env, VOYAVPN_RUST_PROFILE: "release" },
    });
    if (!reuseLibbox || artifacts.libbox.some((path) => !existsSync(path))) {
      run("vp", ["run", "native", "mobile", "libbox", "android"], { cwd: root });
    }
  }

  // --- 3. Gradle -------------------------------------------------------------------
  const apkDir = resolve(androidRoot, "app/build/outputs/apk/release");
  // A stale APK from an earlier build must not pass for this one.
  rmSync(apkDir, { recursive: true, force: true });
  run("./gradlew", ["assembleRelease", `-PvoyaVersionCode=${versionCode}`], { cwd: androidRoot });
  const built = resolve(apkDir, "app-release.apk");
  if (!existsSync(built)) throw new Error(`Gradle produced no ${built}.`);

  // --- 4. verify what was built ----------------------------------------------------
  const signer = parseApkSigner(checkedCapture(resolve(tools, "apksigner"), ["verify", "--print-certs", built]).stdout);
  const badging = checkedCapture(resolve(tools, "aapt2"), ["dump", "badging", built]).stdout;
  const entries = checkedCapture("unzip", ["-Z1", built]).stdout.split("\n");
  throwProblems("Built APK", apkProblems({ badging, signer, keySha256, entries, version, versionCode }));
  console.log(`✓ ${applicationId} ${version} (${versionCode}), signed by ${signer.dn}, ${abis.join(" + ")}`);

  const outputDir = resolve(root, "target/release/bundle/android");
  mkdirSync(outputDir, { recursive: true });
  const apk = resolveApkPath({ outputDir, version, versionCode });
  copyFileSync(built, apk);
  console.log(`\nAndroid package: ${apk}`);
}

if (isCliEntrypoint(import.meta.url)) {
  runCli(main);
}
