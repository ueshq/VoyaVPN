import { existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import {
  capture,
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
  truthy,
} from "../../lib/common.mjs";
import { readJson, walkFilesSync } from "../../lib/fs.mjs";
import { SING_BOX_SEED_MANIFEST, SING_BOX_SOURCE_EXCLUDED_TAGS } from "../../core/sing-box-installer.mjs";
import { bundleImportReport } from "./macho-imports.mjs";
import {
  appBundleIdentifier,
  codesignEntitlements,
  compareMacosVersions,
  incompatiblePacketTunnelBundle,
  legacyPacketTunnelAppexBundle,
  normalizeDistribution,
  packetTunnelBundleIdentifier,
  packetTunnelLayout,
  requireAbsent,
  requirePath,
} from "./tunnel-layout.mjs";
import {
  decodeProvisioningProfile,
  findMatchingIdentities,
  parseCodesigningIdentities,
  isStoreDistributionProfile,
  plistBuddy,
} from "./provisioning.mjs";
import { findQuarantined, quarantineAttribute } from "./quarantine.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const appBundle = resolve(
  process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "release", "bundle", "macos", "VoyaVPN.app"),
);
const appContents = resolve(appBundle, "Contents");
const pkgDir = resolve(process.env.VOYAVPN_MACOS_PKG_DIR || resolve(repoRoot, "target", "release", "bundle", "pkg"));

/** The Mac App Store accepts installer packages signed by one of these. */
export const installerIdentityPattern = /3rd Party Mac Developer Installer:|Mac Installer Distribution:/;
/** ...around an app signed by one of these. */
const appStoreApplicationAuthority = /^Authority=(?:3rd Party Mac Developer Application|Apple Distribution):/m;

/**
 * The `productbuild` arguments for a Mac App Store upload: the signed app as
 * a single component installed into /Applications, signed by the installer
 * identity. App Store Connect ingests exactly this shape.
 */
export function pkgBuildPlan({ appBundle: app, installerIdentity, outputPath, disableTimestamp = false }) {
  const identity = String(installerIdentity ?? "").trim();
  if (!identity) {
    throw new Error("A 3rd Party Mac Developer Installer identity is required to sign the App Store package.");
  }
  const args = ["--component", app, "/Applications", "--sign", identity];
  if (!disableTimestamp) {
    args.push("--timestamp");
  }
  args.push(outputPath);
  return args;
}

/**
 * The one installer identity to sign with.
 *
 * `security find-identity -v` lists every identity once per policy section,
 * and a certificate imported twice shows up twice with the same SHA-1, so
 * matches are compared by fingerprint, not by count.
 */
export function selectInstallerIdentity(identities, explicit) {
  const spec = String(explicit ?? "").trim() || installerIdentityPattern;
  const matches = findMatchingIdentities(identities, spec);
  const unique = [...new Map(matches.map((identity) => [identity.sha1, identity])).values()];
  if (unique.length === 1) {
    return unique[0];
  }
  const label = spec instanceof RegExp ? "3rd Party Mac Developer Installer" : `"${spec}"`;
  if (unique.length === 0) {
    throw new Error(
      `No ${label} identity is in the keychain. Install the Mac Installer Distribution certificate, or set VOYAVPN_INSTALLER_IDENTITY.`,
    );
  }
  throw new Error(
    `Multiple installer identities match ${label}; set VOYAVPN_INSTALLER_IDENTITY to one SHA-1:\n${unique
      .map((identity) => `  ${identity.sha1} "${identity.name}"`)
      .join("\n")}`,
  );
}

/** App Store validation rejects any executable that does not enable the sandbox. */
export function entitlementsEnableSandbox(entitlementsOutput) {
  return /<key>com\.apple\.security\.app-sandbox<\/key>\s*<true\s*\/>/u.test(String(entitlementsOutput ?? ""));
}

/**
 * The deployment targets a Mach-O declares, one per slice: `minos` of
 * LC_BUILD_VERSION, or `version` of the older LC_VERSION_MIN_MACOSX. Parses
 * `otool -arch all -l` output.
 */
export function parseMachOMinimumVersions(otoolOutput) {
  const versions = [];
  for (const block of String(otoolOutput ?? "").split(/Load command \d+/u)) {
    const field = /cmd LC_BUILD_VERSION\b/u.test(block)
      ? "minos"
      : /cmd LC_VERSION_MIN_MACOSX\b/u.test(block)
        ? "version"
        : null;
    const match = field ? block.match(new RegExp(`^\\s*${field}\\s+(\\S+)`, "mu")) : null;
    if (match) versions.push(match[1]);
  }
  return versions;
}

/**
 * App Store Connect's deployment-target rules for the bundle as a whole.
 *
 * - An arm64-only app must declare macOS 12.0 or later (ITMS-90869).
 * - No executable may need a newer macOS than the app declares; the app would
 *   install on a release where that code cannot run.
 */
export function deploymentTargetProblems({ appMinimumSystemVersion, arm64Only, executables }) {
  if (!appMinimumSystemVersion) {
    return ["The app's Info.plist declares no LSMinimumSystemVersion."];
  }
  const problems = [];
  if (arm64Only && compareMacosVersions(appMinimumSystemVersion, "12.0") < 0) {
    problems.push(
      `An arm64-only app must declare LSMinimumSystemVersion 12.0 or later, not ${appMinimumSystemVersion} (ITMS-90869).`,
    );
  }
  for (const { name, minimumVersions } of executables) {
    const newer = minimumVersions.filter((version) => compareMacosVersions(version, appMinimumSystemVersion) > 0);
    if (newer.length) {
      problems.push(`${name} is built for macOS ${newer.join(", ")}, newer than the app's ${appMinimumSystemVersion}.`);
    }
  }
  return problems;
}

/** The Info.plist fields App Store validation checks in each app extension. */
export function appexInfoProblems({ folderName, executableName, minimumSystemVersion, appMinimumSystemVersion }) {
  const problems = [];
  const expectedExecutable = folderName.replace(/\.appex$/u, "");
  if (executableName !== expectedExecutable) {
    problems.push(
      `${folderName}: CFBundleExecutable is "${executableName}" but must equal the folder name, "${expectedExecutable}" (ITMS-90362).`,
    );
  }
  if (!minimumSystemVersion) {
    problems.push(`${folderName}: Info.plist declares no LSMinimumSystemVersion (ITMS-90360).`);
  } else if (appMinimumSystemVersion && compareMacosVersions(minimumSystemVersion, appMinimumSystemVersion) !== 0) {
    problems.push(
      `${folderName}: LSMinimumSystemVersion ${minimumSystemVersion} differs from the app's ${appMinimumSystemVersion}.`,
    );
  }
  return problems;
}

/**
 * The bundled seed must be the one built from source. The upstream release
 * binary is exactly what App Review rejected (Guideline 2.5.1), so this is
 * checked by its manifest as well as by the symbol scan.
 */
export function seedOriginProblems(manifest) {
  if (!manifest) {
    return [`The bundled sing-box seed has no ${SING_BOX_SEED_MANIFEST}.`];
  }
  const problems = [];
  if (manifest.origin !== "source") {
    problems.push(
      `The bundled sing-box seed is the ${manifest.origin ?? "upstream"} build; the store package needs the source-built one (pnpm core:sing-box:build).`,
    );
  }
  const excluded = (Array.isArray(manifest.tags) ? manifest.tags : []).filter((tag) =>
    SING_BOX_SOURCE_EXCLUDED_TAGS.includes(tag));
  if (excluded.length) {
    problems.push(`The bundled sing-box seed was built with ${excluded.join(", ")}.`);
  }
  return problems;
}

export function resolvePkgPath({ pkgDir: dir, version, buildNumber, arch, env = process.env }) {
  const explicit = env.VOYAVPN_MACOS_PKG_PATH?.trim();
  if (explicit) return resolve(explicit);
  return resolve(dir, `VoyaVPN_${version}_${buildNumber}_${arch}.pkg`);
}

function requireAppStoreDistribution() {
  const distribution = normalizeDistribution(process.env.VOYAVPN_MACOS_DISTRIBUTION);
  if (distribution === "developer-id") {
    throw new Error("The App Store package needs VOYAVPN_MACOS_DISTRIBUTION=app-store; Developer ID ships a DMG.");
  }
}

function executablesIn(contents) {
  return walkFilesSync(contents).filter((path) => {
    if (path.includes("/_CodeSignature/")) return false;
    // Any user-execute bit: Mach-O executables in MacOS/ and the core seed in Resources/.
    return (statSync(path).mode & 0o100) !== 0;
  });
}

function verifyBundleShape(layout) {
  requirePath(appBundle, "macOS app bundle");
  requirePath(layout.bundle, layout.label);
  requirePath(layout.binary, "PacketTunnel binary");
  requirePath(resolve(appContents, "embedded.provisionprofile"), "macOS app provisioning profile");
  requirePath(layout.provisioningProfile, "PacketTunnel provisioning profile");
  const reason = "must not ship in the App Store package";
  requireAbsent(incompatiblePacketTunnelBundle(appContents, "app-store"), "A Developer ID system extension", { reason });
  // Remove after 2026-10-31, together with the matching guard in build-tunnel.mjs.
  requireAbsent(legacyPacketTunnelAppexBundle(appContents), "A PacketTunnel under the old bundle-id folder name", { reason });
  requireAbsent(resolve(appContents, "MacOS", "export-bindings"), "The export-bindings development tool", { reason });
  requireAbsent(resolve(appContents, "MacOS", "voyavpn-tunnel-service"), "The Windows-only tunnel service", { reason });
}

/** A development profile signs fine and uploads fine, then fails App Review. */
function verifyDistributionProfile(path, label, bundleIdentifier) {
  const profile = decodeProvisioningProfile(path);
  if (profile.bundleIdentifier !== bundleIdentifier) {
    throw new Error(`${label} profile is for ${profile.bundleIdentifier}, expected ${bundleIdentifier}: ${path}`);
  }
  if (!isStoreDistributionProfile(profile)) {
    throw new Error(
      `${label} profile "${profile.name}" is not a Mac App Store distribution profile. `
        + "Create a Mac App Store Connect profile for it (docs/release/macos-app-store.md).",
    );
  }
  console.log(`✓ ${label} provisioning profile is a Mac App Store profile: ${profile.name}`);
}

function verifySignatures(executables) {
  run("codesign", ["--verify", "--deep", "--strict", "--verbose=2", appBundle], { cwd: repoRoot });
  const details = capture("codesign", ["-dvv", appBundle], { cwd: repoRoot });
  if (!appStoreApplicationAuthority.test(`${details.stdout ?? ""}\n${details.stderr ?? ""}`)) {
    throw new Error(
      "The app is not signed by a 3rd Party Mac Developer Application or Apple Distribution identity. Re-run pnpm build:mac:appstore.",
    );
  }
  console.log("✓ App is signed by a Mac App Store application identity");

  for (const executable of executables) {
    const name = relative(appContents, executable);
    if (!entitlementsEnableSandbox(codesignEntitlements(executable))) {
      throw new Error(`${name} does not enable App Sandbox; App Store validation would reject the package.`);
    }
    const archs = checkedCapture("lipo", ["-archs", executable], { cwd: repoRoot }).stdout.trim().split(/\s+/);
    if (!archs.includes("arm64")) {
      throw new Error(`${name} has no arm64 slice (${archs.join(", ")}).`);
    }
    console.log(`✓ ${name}: sandboxed, ${archs.join("+")}`);
  }
}

/** Guideline 2.5.1: no non-public symbol, no private library, in any Mach-O of the bundle. */
function verifyPublicApiImports() {
  const { binaries, problems } = bundleImportReport(appContents);
  throwProblems(problems);
  for (const name of binaries) {
    console.log(`✓ ${name}: public API only`);
  }
}

function verifySeedOrigin() {
  const manifestPath = resolve(appContents, "Resources", "core-seeds", "sing_box", SING_BOX_SEED_MANIFEST);
  const manifest = existsSync(manifestPath) ? readJson(manifestPath) : null;
  throwProblems(seedOriginProblems(manifest));
  console.log(`✓ sing-box seed is built from source at ${manifest.commit} (${manifest.tags.join(",")})`);
}

function throwProblems(problems) {
  if (problems.length) {
    throw new Error(`App Store validation would reject this bundle:\n${problems.map((line) => `  - ${line}`).join("\n")}`);
  }
}

function verifyAppExtensions(appMinimumSystemVersion) {
  const plugIns = resolve(appContents, "PlugIns");
  const folders = existsSync(plugIns) ? readdirSync(plugIns).filter((name) => name.endsWith(".appex")) : [];
  const problems = folders.flatMap((folderName) => {
    const infoPlist = resolve(plugIns, folderName, "Contents", "Info.plist");
    return appexInfoProblems({
      folderName,
      executableName: plistBuddy(infoPlist, ":CFBundleExecutable", true),
      minimumSystemVersion: plistBuddy(infoPlist, ":LSMinimumSystemVersion", true),
      appMinimumSystemVersion,
    });
  });
  throwProblems(problems);
  console.log(`✓ App extensions name their executables and declare macOS ${appMinimumSystemVersion}: ${folders.join(", ")}`);
}

function verifyDeploymentTargets(executables, appMinimumSystemVersion) {
  const slices = executables.map((executable) => ({
    name: relative(appContents, executable),
    archs: checkedCapture("lipo", ["-archs", executable], { cwd: repoRoot }).stdout.trim().split(/\s+/),
    minimumVersions: parseMachOMinimumVersions(
      checkedCapture("otool", ["-arch", "all", "-l", executable], { cwd: repoRoot }).stdout,
    ),
  }));
  throwProblems(
    deploymentTargetProblems({
      appMinimumSystemVersion,
      arm64Only: slices.every((slice) => !slice.archs.includes("x86_64")),
      executables: slices,
    }),
  );
  for (const slice of slices) {
    console.log(`✓ ${slice.name}: built for macOS ${slice.minimumVersions.join(", ") || "(undeclared)"}`);
  }
}

function assertNoQuarantine(root, label) {
  const found = findQuarantined(root);
  if (found.length) {
    throw new Error(
      `${label} has files with ${quarantineAttribute}, which App Store Connect rejects (ITMS-91109):\n${found
        .map((path) => `  - ${path}`)
        .join("\n")}`,
    );
  }
  console.log(`✓ ${label} has no quarantined files`);
}

/**
 * `productbuild` keeps extended attributes in the payload, so the check that
 * matters is on the package itself, exactly as it will be uploaded. The
 * expansion holds a copy of the PacketTunnel appex; it is deleted right away
 * so PlugInKit never sees it (AGENTS.md, NetworkExtension hygiene).
 */
function verifyPackagePayload(outputPath) {
  const expanded = resolve(repoRoot, "target", "native", "macos", "pkg-verify");
  rmSync(expanded, { recursive: true, force: true });
  mkdirSync(dirname(expanded), { recursive: true });
  try {
    run("pkgutil", ["--expand-full", outputPath, expanded], { cwd: repoRoot });
    assertNoQuarantine(expanded, "Package payload");
  } finally {
    rmSync(expanded, { recursive: true, force: true });
  }
}

function installerIdentity() {
  const listing = checkedCapture("security", ["find-identity", "-v"], { cwd: repoRoot }).stdout;
  return selectInstallerIdentity(parseCodesigningIdentities(listing), process.env.VOYAVPN_INSTALLER_IDENTITY);
}

function main() {
  requireDarwin("The Mac App Store package must be built on macOS.");
  requireAppStoreDistribution();
  const layout = packetTunnelLayout(appContents, "app-store");
  verifyBundleShape(layout);
  verifyDistributionProfile(resolve(appContents, "embedded.provisionprofile"), "macOS app", appBundleIdentifier);
  verifyDistributionProfile(layout.provisioningProfile, "PacketTunnel", packetTunnelBundleIdentifier);

  const infoPlist = resolve(appContents, "Info.plist");
  const appMinimumSystemVersion = plistBuddy(infoPlist, ":LSMinimumSystemVersion", true);
  const executables = executablesIn(appContents);
  verifySignatures(executables);
  verifyDeploymentTargets(executables, appMinimumSystemVersion);
  verifyAppExtensions(appMinimumSystemVersion);
  verifySeedOrigin();
  verifyPublicApiImports();
  assertNoQuarantine(appBundle, "App bundle");

  const version = plistBuddy(infoPlist, ":CFBundleShortVersionString");
  const buildNumber = plistBuddy(infoPlist, ":CFBundleVersion");
  const mainExecutable = resolve(appContents, "MacOS", plistBuddy(infoPlist, ":CFBundleExecutable"));
  const archs = checkedCapture("lipo", ["-archs", mainExecutable], { cwd: repoRoot }).stdout.trim().split(/\s+/);
  const arch = archs.includes("x86_64") ? "universal" : "aarch64";
  const outputPath = resolvePkgPath({ pkgDir, version, buildNumber, arch });

  const identity = installerIdentity();
  console.log(`Signing the installer package with ${identity.name} (${identity.sha1})`);
  mkdirSync(dirname(outputPath), { recursive: true });
  rmSync(outputPath, { force: true });
  try {
    run(
      "productbuild",
      pkgBuildPlan({
        appBundle,
        installerIdentity: identity.sha1,
        outputPath,
        disableTimestamp: truthy(process.env.VOYAVPN_DISABLE_CODESIGN_TIMESTAMP),
      }),
      { cwd: repoRoot },
    );
    run("pkgutil", ["--check-signature", outputPath], { cwd: repoRoot });
    verifyPackagePayload(outputPath);
  } catch (error) {
    rmSync(outputPath, { force: true });
    throw error;
  }

  console.log("");
  console.log(`Mac App Store package: ${outputPath}`);
  console.log(`  Version ${version} (build ${buildNumber}), ${arch}`);
  console.log("Upload it with Transporter: add the .pkg, choose Verify, then Deliver.");
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
