import { spawnSync } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

import {
  capture,
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  runCli,
} from "../../lib/common.mjs";
import { readJson } from "../../lib/fs.mjs";
import { resolveStoreBuildNumber } from "../../tauri/mac-app-store-config.mjs";
import { bundleImportReport, bundlePrivilegeEscalationReport } from "../macos/macho-imports.mjs";
import {
  decodeProvisioningProfile,
  defaultProvisioningProfileDir,
  formatProfileSelectionError,
  installedProvisioningProfileDir,
  isStoreDistributionProfile,
  plistBuddy,
  resolveProfileFromEnv,
  resolveSigningIdentity,
} from "../macos/provisioning.mjs";
import { findQuarantined } from "../macos/quarantine.mjs";
import { checkIosBundleInputs, IOS_DEPLOYMENT_TARGET, parsePlist } from "./ios-bundle-checks.mjs";
import { podsUpToDate, recordInstalledPods } from "./ios-pods-cache.mjs";

/**
 * `pnpm build:ios:appstore`: the `.ipa` that Transporter uploads to App Store
 * Connect for TestFlight and App Review.
 *
 *   --unsigned      Archive without signing and skip the export. Runs every
 *                   check that does not need a signature, so the lane can be
 *                   exercised without a distribution certificate.
 *   --reuse-libbox  Keep the staged Libbox.xcframework instead of rebuilding it.
 *   --skip-native   Skip the Rust, Libbox, CocoaPods and project steps.
 *
 * Signing is manual, with explicit App Store profiles, like the Mac lane: the
 * profiles are files that can be read and checked before anything is built.
 * Nothing about signing is written into the Xcode project; every setting is
 * passed on the command line. See docs/release/mobile-ios-signing.md.
 */

const appBundleId = "app.voyavpn.mobile";
const tunnelBundleId = "app.voyavpn.mobile.PacketTunnel";
const appGroup = "group.app.voyavpn.mobile";
const tunnelCapability = "packet-tunnel-provider";
const minimumOsVersion = IOS_DEPLOYMENT_TARGET;
const distributionIdentity = /^(?:Apple|iPhone) Distribution: /u;

const networkExtensionKey = "com.apple.developer.networking.networkextension";
const appGroupsKey = "com.apple.security.application-groups";
const teamKey = "com.apple.developer.team-identifier";

const sameList = (left, right) => JSON.stringify(left) === JSON.stringify(right);

/** What would stop this App Store profile from signing `bundleId`. */
export function iosProfileProblems(profile, bundleId) {
  const problems = [];
  const label = `Profile "${profile.name}"`;
  if (profile.applicationIdentifier !== `${profile.teamIdentifier}.${bundleId}`) {
    problems.push(`${label} is for ${profile.applicationIdentifier}, expected ${profile.teamIdentifier}.${bundleId}.`);
  }
  if (!isStoreDistributionProfile(profile)) {
    problems.push(`${label} is not an App Store distribution profile (it lists devices or has no distribution certificate).`);
  }
  if (profile.getTaskAllow) {
    problems.push(`${label} allows debugging (get-task-allow); App Store profiles do not.`);
  }
  if (!profile.appGroups.includes(appGroup)) {
    problems.push(`${label} does not grant the App Group ${appGroup}.`);
  }
  if (!profile.networkExtensions.includes(tunnelCapability)) {
    problems.push(`${label} does not grant Network Extensions ${tunnelCapability}.`);
  }
  return problems;
}

/**
 * What App Store validation would reject in a signed bundle's entitlements.
 * `entitlements` is the parsed `codesign -d --entitlements` plist.
 */
export function entitlementProblems(entitlements, { name, bundleId, teamId }) {
  const problems = [];
  const expected = {
    "application-identifier": `${teamId}.${bundleId}`,
    [teamKey]: teamId,
    [networkExtensionKey]: [tunnelCapability],
    [appGroupsKey]: [appGroup],
  };
  for (const [key, value] of Object.entries(expected)) {
    if (!sameList(entitlements[key], value)) {
      problems.push(`${name}: ${key} is ${JSON.stringify(entitlements[key])}, expected ${JSON.stringify(value)}.`);
    }
  }
  if (entitlements["get-task-allow"]) {
    problems.push(`${name}: get-task-allow is set; the bundle was signed for development.`);
  }
  // App Store profiles add beta-reports-active for TestFlight.
  const allowed = new Set([...Object.keys(expected), "get-task-allow", "beta-reports-active"]);
  for (const key of Object.keys(entitlements).filter((entry) => !allowed.has(entry))) {
    problems.push(`${name}: unexpected entitlement ${key}.`);
  }
  return problems;
}

/** Version fields of the built app and extension, read from their Info.plists. */
export function builtInfoProblems({ app, tunnel, version, buildNumber }) {
  const problems = [];
  for (const [name, info] of [["VoyaVPN.app", app], ["PacketTunnel.appex", tunnel]]) {
    if (info.shortVersion !== version) {
      problems.push(`${name} CFBundleShortVersionString is ${info.shortVersion}, expected ${version}.`);
    }
    // An extension whose build number differs from its app is ITMS-90473.
    if (info.bundleVersion !== buildNumber) {
      problems.push(`${name} CFBundleVersion is ${info.bundleVersion}, expected ${buildNumber}.`);
    }
    if (info.minimumOsVersion !== minimumOsVersion) {
      problems.push(`${name} MinimumOSVersion is ${info.minimumOsVersion}, expected ${minimumOsVersion}.`);
    }
  }
  if (app.bundleId !== appBundleId) problems.push(`VoyaVPN.app bundle id is ${app.bundleId}.`);
  if (tunnel.bundleId !== tunnelBundleId) problems.push(`PacketTunnel.appex bundle id is ${tunnel.bundleId}.`);
  if (!app.hasIcons) problems.push("VoyaVPN.app has no CFBundleIcons; the icon set was not compiled in.");
  return problems;
}

/**
 * xcodebuild settings that sign the app and the extension with their own
 * profiles. A command-line setting applies to every target, CocoaPods ones
 * included, so the profile is chosen through a macro keyed by product name:
 * it resolves to nothing for a pod, which needs no profile.
 */
export function signingBuildSettings({ teamId, identityName, appProfile, tunnelProfile }) {
  return [
    "CODE_SIGN_STYLE=Manual",
    `DEVELOPMENT_TEAM=${teamId}`,
    `CODE_SIGN_IDENTITY=${identityName}`,
    "PROVISIONING_PROFILE_SPECIFIER=$(VOYA_PROFILE_$(PRODUCT_NAME))",
    `VOYA_PROFILE_VoyaVPN=${appProfile.uuid}`,
    `VOYA_PROFILE_PacketTunnel=${tunnelProfile.uuid}`,
  ];
}

const xmlEscape = (text) => String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");

/** The options `xcodebuild -exportArchive` needs to write an App Store `.ipa`. */
export function exportOptionsPlist({ teamId, identityName, appProfile, tunnelProfile }) {
  const certificate = identityName.split(":")[0];
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key>
  <string>app-store-connect</string>
  <key>destination</key>
  <string>export</string>
  <key>teamID</key>
  <string>${xmlEscape(teamId)}</string>
  <key>signingStyle</key>
  <string>manual</string>
  <key>signingCertificate</key>
  <string>${xmlEscape(certificate)}</string>
  <key>provisioningProfiles</key>
  <dict>
    <key>${appBundleId}</key>
    <string>${xmlEscape(appProfile.uuid)}</string>
    <key>${tunnelBundleId}</key>
    <string>${xmlEscape(tunnelProfile.uuid)}</string>
  </dict>
  <key>manageAppVersionAndBuildNumber</key>
  <false/>
  <key>uploadSymbols</key>
  <true/>
  <key>stripSwiftSymbols</key>
  <true/>
</dict>
</plist>
`;
}

/**
 * Where Xcode looks up a provisioning profile by UUID. Xcode 16 moved the
 * folder; older versions read only the MobileDevice one.
 */
export function xcodeProfileDir({ xcodeMajor, home }) {
  return xcodeMajor >= 16
    ? resolve(home, "Library/Developer/Xcode/UserData/Provisioning Profiles")
    : resolve(home, "Library/MobileDevice/Provisioning Profiles");
}

/** The major version from `xcodebuild -version` output ("Xcode 26.6\nBuild version …"). */
export function parseXcodeMajor(versionOutput) {
  const match = /^Xcode (\d+)/mu.exec(String(versionOutput ?? ""));
  if (!match) throw new Error(`Unable to read the Xcode version from: ${versionOutput}`);
  return Number.parseInt(match[1], 10);
}

/**
 * Makes a selected profile visible to xcodebuild, which resolves
 * `PROVISIONING_PROFILE_SPECIFIER` only among installed profiles: a file in
 * `../docs/certs` does not count. The install is the file under its UUID in
 * Xcode's folder, which is also all that Xcode's own "install" does.
 *
 * The bytes are written fresh rather than copied. A profile saved from a
 * browser carries `com.apple.quarantine`, a copy inherits it, Xcode embeds
 * that copy into the app, and App Store Connect rejects a quarantined file
 * (ITMS-91109).
 */
export function installProfileForXcode(profile, directory, io = { mkdirSync, readFileSync, writeFileSync }) {
  const destination = resolve(directory, `${profile.uuid}.mobileprovision`);
  if (resolve(profile.path) !== destination) {
    io.mkdirSync(directory, { recursive: true });
    io.writeFileSync(destination, io.readFileSync(profile.path));
  }
  return destination;
}

/**
 * What `--skip-native` takes on trust: an archive is a device build, and both
 * frameworks have to carry a device slice. The simulator smoke lane rebuilds
 * the Rust host with only its own slice, so "the artifacts are already there"
 * is routinely false after it, and the archive would find out at link time,
 * after every pod has compiled. `slices` maps each framework to the directory
 * names inside it, or `null` when the framework is missing.
 */
export function deviceSliceProblems(slices) {
  return Object.entries(slices).flatMap(([framework, names]) => {
    if (names === null) return [`${framework} is missing`];
    const device = names.some((name) => name.startsWith("ios-") && !name.includes("simulator"));
    return device ? [] : [`${framework} has no device slice (found: ${names.join(", ") || "none"})`];
  });
}

/**
 * The two executables every archive has, by their path inside the app. A scan
 * that did not reach them checked nothing, and would report a clean bundle.
 */
export function scannedBinaryProblems(binaries) {
  return ["VoyaVPN", "PlugIns/PacketTunnel.appex/PacketTunnel"]
    .filter((binary) => !binaries.includes(binary))
    .map((binary) => `the import scan did not reach ${binary}`);
}

export function resolveIpaPath({ outputDir, version, buildNumber }) {
  return resolve(outputDir, `VoyaVPN_${version}_${buildNumber}.ipa`);
}

function throwProblems(title, problems) {
  if (problems.length > 0) {
    throw new Error(`${title}:\n${problems.map((line) => `  - ${line}`).join("\n")}`);
  }
}

function resolveStoreProfile(bundleId, explicitEnvName, identity) {
  const override = process.env.VOYAVPN_PROVISIONING_PROFILE_DIR?.trim();
  const profileDir = override
    ? [resolve(override)]
    : [
      defaultProvisioningProfileDir,
      installedProvisioningProfileDir,
      // Where Xcode 16 and later keep profiles, including ones this lane installed.
      resolve(homedir(), "Library/Developer/Xcode/UserData/Provisioning Profiles"),
    ];
  const { profile, rejections } = resolveProfileFromEnv({
    bundleIdentifier: bundleId,
    explicitEnvName,
    profileDir,
    criteria: { distribution: "app-store", identitySha1: identity.sha1 },
  });
  if (!profile) {
    throw new Error(
      `${formatProfileSelectionError("App Store", bundleId, rejections, profileDir)}\n`
        + "Create an App Store Connect distribution profile for it (docs/release/mobile-ios-signing.md).",
    );
  }
  throwProblems(`Provisioning profile for ${bundleId}`, iosProfileProblems(profile, bundleId));
  console.log(`✓ ${bundleId}: ${profile.name} (${profile.uuid})`);
  return profile;
}

function builtInfo(bundle) {
  const plist = resolve(bundle, "Info.plist");
  return {
    bundleId: plistBuddy(plist, ":CFBundleIdentifier"),
    shortVersion: plistBuddy(plist, ":CFBundleShortVersionString"),
    bundleVersion: plistBuddy(plist, ":CFBundleVersion"),
    minimumOsVersion: plistBuddy(plist, ":MinimumOSVersion"),
    hasIcons: plistBuddy(plist, ":CFBundleIcons", true) !== "",
  };
}

function machOArchProblems(app, binaries) {
  return binaries.flatMap((name) => {
    const archs = checkedCapture("lipo", ["-archs", resolve(app, name)]).stdout.trim();
    return archs === "arm64" ? [] : [`${name} is built for "${archs}", expected arm64 only.`];
  });
}

function signedEntitlements(bundle) {
  const result = checkedCapture("codesign", ["-d", "--entitlements", "-", "--xml", bundle]);
  return parsePlist(result.stdout);
}

export function main(argv = process.argv.slice(2)) {
  const unsigned = argv.includes("--unsigned");
  const skipNative = argv.includes("--skip-native");
  const reuseLibbox = argv.includes("--reuse-libbox");
  const unknown = argv.filter((flag) => !["--unsigned", "--skip-native", "--reuse-libbox"].includes(flag));
  if (unknown.length > 0) throw new Error(`Unknown option ${unknown.join(" ")}.`);
  if (process.platform !== "darwin" || process.arch !== "arm64") {
    throw new Error("The iOS App Store build needs an Apple silicon Mac with Xcode.");
  }

  const root = repoRootFromScript(import.meta.url);
  const ios = resolve(root, "apps/mobile/ios");
  const outputDir = resolve(root, "target/release/bundle/ios");
  const logs = resolve(outputDir, "logs");
  rmSync(logs, { recursive: true, force: true });
  mkdirSync(logs, { recursive: true });

  // xcodebuild prints tens of thousands of lines; they go to a file per step.
  let step = 0;
  const runLogged = (label, program, args, { cwd = root, env = process.env } = {}) => {
    const log = resolve(logs, `${String(++step).padStart(3, "0")}-${label}.log`);
    console.log(`→ ${label} (${log})`);
    const fd = openSync(log, "w");
    try {
      const result = spawnSync(program, args, { cwd, env, stdio: ["ignore", fd, fd] });
      if (result.error) throw result.error;
      if (result.status !== 0) throw new Error(`${label} exited ${result.status}; see ${log}`);
    } finally {
      closeSync(fd);
    }
  };

  // --- 1. preflight: everything that can fail before a 20-minute build -----
  const xcodeVersion = checkedCapture("xcodebuild", ["-version"]).stdout.trim();
  console.log(xcodeVersion.replace("\n", ", "));
  const version = readJson(resolve(root, "package.json")).version;
  const buildNumber = resolveStoreBuildNumber({ envName: "VOYAVPN_IOS_BUILD_NUMBER", repoRoot: root });
  console.log(`VoyaVPN ${version} (${buildNumber})${unsigned ? ", unsigned dry run" : ""}`);
  if (capture("git", ["status", "--porcelain"], { cwd: root }).stdout?.trim()) {
    console.warn("! The working tree has uncommitted changes; the build number counts commits only.");
  }
  checkIosBundleInputs(root);
  if (skipNative) {
    const frameworks = resolve(ios, "Frameworks");
    const slices = Object.fromEntries(
      ["VoyaMobile.xcframework", "Libbox.xcframework"].map((name) => {
        const framework = resolve(frameworks, name);
        return [name, existsSync(framework) ? readdirSync(framework) : null];
      }),
    );
    throwProblems(
      "--skip-native needs the device builds of both frameworks; run without it, or `pnpm native:mobile:rust:ios --slice device` and `pnpm native:mobile:libbox:ios`",
      deviceSliceProblems(slices),
    );
  }

  let signing = null;
  if (!unsigned) {
    const identity = resolveSigningIdentity(process.env.VOYAVPN_CODESIGN_IDENTITY?.trim() || distributionIdentity, "App Store");
    console.log(`✓ Signing identity: ${identity.name}`);
    const appProfile = resolveStoreProfile(appBundleId, "VOYAVPN_IOS_APP_PROVISIONING_PROFILE", identity);
    const tunnelProfile = resolveStoreProfile(tunnelBundleId, "VOYAVPN_IOS_PACKET_TUNNEL_PROVISIONING_PROFILE", identity);
    signing = { teamId: appProfile.teamIdentifier, identityName: identity.name, appProfile, tunnelProfile };
    const profileDir = xcodeProfileDir({ xcodeMajor: parseXcodeMajor(xcodeVersion), home: homedir() });
    for (const profile of [appProfile, tunnelProfile]) {
      installProfileForXcode(profile, profileDir);
    }
    console.log(`✓ Both profiles are installed for Xcode in ${profileDir}`);
  }

  // --- 2. native artifacts ---------------------------------------------------
  if (!skipNative) {
    // An archive is a device build; the simulator slice would never be linked.
    runLogged("rustup-targets", "rustup", ["target", "add", "aarch64-apple-ios"]);
    // Always the release profile: a `VOYAVPN_RUST_PROFILE` left exported from
    // an iteration loop would otherwise archive a debug backend, and nothing
    // after this step can tell.
    runLogged("rust-host", "pnpm", ["native:mobile:rust:ios", "--slice", "device"], {
      env: { ...process.env, VOYAVPN_RUST_PROFILE: "release" },
    });
    if (!reuseLibbox || !existsSync(resolve(ios, "Frameworks/Libbox.xcframework"))) {
      runLogged("libbox", "pnpm", ["native:mobile:libbox:ios"]);
    }
    if (!podsUpToDate(root)) {
      runLogged("pod-install", "pod", ["install"], { cwd: ios });
      recordInstalledPods(root);
    }
    runLogged("xcode-project", "pnpm", ["native:mobile:ios:project"]);
  }

  // --- 3. archive -------------------------------------------------------------
  const archive = resolve(outputDir, "VoyaVPN.xcarchive");
  rmSync(archive, { recursive: true, force: true });
  const signingSettings = signing ? signingBuildSettings(signing) : ["CODE_SIGNING_ALLOWED=NO"];
  if (signing) {
    // The same settings as a file: evidence of what was signed with, and the
    // fallback (`-xcconfig`) if a future Xcode stops resolving the macro.
    writeFileSync(resolve(outputDir, "signing.xcconfig"), `${signingSettings.map((line) => line.replace("=", " = ")).join("\n")}\n`);
  }
  runLogged("archive", "xcodebuild", [
    "archive",
    "-workspace", resolve(ios, "VoyaVPN.xcworkspace"),
    "-scheme", "VoyaVPN",
    "-configuration", "Release",
    "-destination", "generic/platform=iOS",
    "-archivePath", archive,
    "-derivedDataPath", resolve(ios, "build/DerivedData-appstore"),
    `CURRENT_PROJECT_VERSION=${buildNumber}`,
    "ARCHS=arm64",
    "ONLY_ACTIVE_ARCH=NO",
    ...signingSettings,
  ]);

  // --- 4. verify what was built ----------------------------------------------
  const app = resolve(archive, "Products/Applications/VoyaVPN.app");
  const tunnel = resolve(app, "PlugIns/PacketTunnel.appex");
  if (!existsSync(app) || !existsSync(tunnel)) {
    throw new Error(`The archive has no VoyaVPN.app with a PacketTunnel.appex: ${archive}`);
  }
  if (readdirSync(resolve(app, "PlugIns")).length !== 1) {
    throw new Error("VoyaVPN.app embeds more than the PacketTunnel extension.");
  }
  throwProblems("Built Info.plist", builtInfoProblems({ app: builtInfo(app), tunnel: builtInfo(tunnel), version, buildNumber }));
  console.log(`✓ App and PacketTunnel are ${version} (${buildNumber}) for iOS ${minimumOsVersion}+`);

  const imports = bundleImportReport(app);
  throwProblems("Mach-O scan", scannedBinaryProblems(imports.binaries));
  throwProblems("Non-public API (Guideline 2.5.1)", imports.problems);
  throwProblems("Privilege escalation text", bundlePrivilegeEscalationReport(app).problems);
  throwProblems("Architectures", machOArchProblems(app, imports.binaries));
  console.log(`✓ ${imports.binaries.length} Mach-O files: arm64, public API only`);

  if (!signing) {
    console.log(`\nUnsigned archive: ${archive}`);
    console.log("Signing, entitlements and the .ipa export were skipped (--unsigned).");
    return;
  }

  checkedCapture("codesign", ["--verify", "--deep", "--strict", app]);
  const { teamId } = signing;
  throwProblems("Signed entitlements", [
    ...entitlementProblems(signedEntitlements(app), { name: "VoyaVPN.app", bundleId: appBundleId, teamId }),
    ...entitlementProblems(signedEntitlements(tunnel), { name: "PacketTunnel.appex", bundleId: tunnelBundleId, teamId }),
  ]);
  for (const [bundle, profile] of [[app, signing.appProfile], [tunnel, signing.tunnelProfile]]) {
    const embedded = decodeProvisioningProfile(resolve(bundle, "embedded.mobileprovision"));
    if (embedded.uuid !== profile.uuid) {
      throw new Error(`${bundle} embeds profile ${embedded.uuid}, expected ${profile.uuid}.`);
    }
  }
  const quarantined = findQuarantined(app);
  throwProblems(
    "Quarantined files (ITMS-91109)",
    quarantined.map((path) => `${path} carries com.apple.quarantine.`),
  );
  console.log("✓ Signature, entitlements and embedded profiles match the App Store profiles; nothing is quarantined");

  // --- 5. export ---------------------------------------------------------------
  const exportOptions = resolve(outputDir, "ExportOptions.plist");
  writeFileSync(exportOptions, exportOptionsPlist(signing));
  const exportDir = resolve(outputDir, "export");
  rmSync(exportDir, { recursive: true, force: true });
  runLogged("export", "xcodebuild", [
    "-exportArchive",
    "-archivePath", archive,
    "-exportOptionsPlist", exportOptions,
    "-exportPath", exportDir,
  ]);
  const exported = readdirSync(exportDir).find((name) => name.endsWith(".ipa"));
  if (!exported) throw new Error(`xcodebuild exported no .ipa into ${exportDir}.`);
  const ipa = resolveIpaPath({ outputDir, version, buildNumber });
  renameSync(resolve(exportDir, exported), ipa);

  console.log(`\nApp Store package: ${ipa}`);
  console.log(`Archive (symbols): ${archive}`);
  console.log("Upload with Transporter, then follow docs/release/mobile-ios-signing.md.");
}

if (isCliEntrypoint(import.meta.url)) {
  runCli(main);
}
