import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  rmdirSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { capture, isCliEntrypoint, repoRootFromScript, requireDarwin, run, truthy } from "../../lib/common.mjs";
import {
  appBundleIdentifier,
  libboxBinaryPath,
  incompatiblePacketTunnelBundle,
  packetTunnelBundleIdentifier,
  packetTunnelLayout,
  packetTunnelSources,
  resolvePacketTunnelVersions,
  distributionFromIdentityName,
} from "./tunnel-layout.mjs";
import {
  assertProfileCapabilities,
  distributionProfileLabel,
  formatProfileSelectionError,
  localProvisioningUdid,
  plistBuddy,
  profileRejectionReason,
  resolveProfileFromEnv,
  resolveSigningIdentity,
  writeProfileEntitlements,
} from "./provisioning.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const nativeRoot = resolve(repoRoot, "apps", "desktop", "src-tauri", "native", "macos");
const outRoot = resolve(repoRoot, "target", "native", "macos");
const appBundle = resolve(process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(outRoot, "VoyaVPN.app"));
const appContents = resolve(appBundle, "Contents");
const providerSources = packetTunnelSources(nativeRoot);
const resolvedIdentity = resolveIdentityFromEnv();
const macosDistribution = distributionFromIdentityName(
  resolvedIdentity?.name ?? "",
  process.env.VOYAVPN_MACOS_DISTRIBUTION,
);
const tunnelLayout = packetTunnelLayout(appContents, macosDistribution);
const incompatibleTunnelBundle = incompatiblePacketTunnelBundle(appContents, macosDistribution);
const appexContents = tunnelLayout.contents;
const appexBundle = tunnelLayout.bundle;
const appexBinary = tunnelLayout.binary;
const appexFrameworks = tunnelLayout.frameworks;
const appProvisioningProfileDestination = resolve(appContents, "embedded.provisionprofile");
const packetTunnelProvisioningProfileDestination = tunnelLayout.provisioningProfile;
const defaultLibboxFramework = resolve(nativeRoot, "Frameworks", "Libbox.framework");
const libboxFramework = resolve(process.env.VOYAVPN_LIBBOX_FRAMEWORK || defaultLibboxFramework);
const embeddedLibboxFramework = tunnelLayout.embeddedLibboxFramework;
const appEntitlements = resolve(repoRoot, "apps", "desktop", "src-tauri", "entitlements", "macos-app.plist");
const packetTunnelEntitlements = resolve(repoRoot, "apps", "desktop", "src-tauri", "entitlements", "packet-tunnel.plist");
const defaultProvisioningProfileDir = resolve(repoRoot, "..", "docs", "certs");
const provisioningProfileDir = resolve(process.env.VOYAVPN_PROVISIONING_PROFILE_DIR || defaultProvisioningProfileDir);
const generatedEntitlementsDir = resolve(outRoot, "generated-entitlements");

function resolveIdentityFromEnv() {
  const value = process.env.VOYAVPN_CODESIGN_IDENTITY?.trim();
  if (!value) {
    return null;
  }
  try {
    return resolveSigningIdentity(value);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

function signingCriteria() {
  return {
    distribution: macosDistribution,
    allowDevelopmentProvisioning: truthy(process.env.VOYAVPN_ALLOW_DEVELOPMENT_PROVISIONING),
    identitySha1: resolvedIdentity?.sha1 ?? null,
    deviceUdid: resolvedIdentity ? localProvisioningUdid() : null,
  };
}

/**
 * Reads the containing app's version fields so the embedded PacketTunnel always
 * matches its container, falling back to the root package.json version when the
 * app bundle has not been written yet.
 */
function packetTunnelVersions() {
  const appInfoPlist = resolve(appContents, "Info.plist");
  const hasAppPlist = existsSync(appInfoPlist);

  return resolvePacketTunnelVersions({
    appShortVersion: hasAppPlist ? plistBuddy(appInfoPlist, "CFBundleShortVersionString", true) : "",
    appBundleVersion: hasAppPlist ? plistBuddy(appInfoPlist, "CFBundleVersion", true) : "",
    packageVersion: JSON.parse(readFileSync(resolve(repoRoot, "package.json"), "utf8")).version,
  });
}

function writePlist(source, destination, replacements = {}) {
  let text = readFileSync(source, "utf8");
  for (const [from, to] of Object.entries(replacements)) {
    text = text.replaceAll(from, to);
  }
  writeFileSync(destination, text);
}

function findProvisioningProfile(bundleIdentifier, envName, criteria) {
  return resolveProfileFromEnv({
    bundleIdentifier,
    explicitEnvName: envName,
    profileDir: provisioningProfileDir,
    criteria,
  });
}

function validateProvisioningProfile(profile, label, bundleIdentifier, criteria) {
  const reason = profileRejectionReason(profile, { ...criteria, bundleIdentifier });
  if (reason) {
    throw new Error(`${label} provisioning profile ${profile.path} cannot be used: ${reason}.`);
  }
  if (profile.teamIdentifier && !profile.applicationIdentifier.startsWith(`${profile.teamIdentifier}.`)) {
    throw new Error(`${label} provisioning profile application identifier does not match its team identifier.`);
  }
  assertProfileCapabilities(profile, { label, distribution: criteria.distribution });
}

function profileOrWarn(bundleIdentifier, envName, label, criteria) {
  const { profile, rejections } = findProvisioningProfile(bundleIdentifier, envName, criteria);
  if (!profile) {
    const message = `${formatProfileSelectionError(
      `${label} ${distributionProfileLabel(criteria.distribution)}`,
      bundleIdentifier,
      rejections,
      provisioningProfileDir,
    )}\nSet ${envName} to select a profile explicitly.`;
    if (truthy(process.env.VOYAVPN_REQUIRE_PROVISIONING) || process.env.VOYAVPN_CODESIGN_IDENTITY) {
      throw new Error(message);
    }
    console.warn(message);
    return null;
  }

  validateProvisioningProfile(profile, label, bundleIdentifier, criteria);
  console.log(`Using ${label} provisioning profile ${profile.name || profile.uuid || profile.path}`);
  return profile;
}

function entitlementsForSigning(baseEntitlements, profile, name) {
  if (!profile) {
    return baseEntitlements;
  }
  return writeProfileEntitlements(profile, resolve(generatedEntitlementsDir, name), baseEntitlements);
}

function stageProvisioningProfiles() {
  const criteria = signingCriteria();
  const appProfile = profileOrWarn(
    appBundleIdentifier,
    "VOYAVPN_MACOS_APP_PROVISIONING_PROFILE",
    "macOS app",
    criteria,
  );
  const packetTunnelProfile = profileOrWarn(
    packetTunnelBundleIdentifier,
    "VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE",
    "PacketTunnel",
    criteria,
  );

  if (appProfile) {
    mkdirSync(appContents, { recursive: true });
    cpSync(appProfile.path, appProvisioningProfileDestination);
  }
  if (packetTunnelProfile) {
    mkdirSync(appexContents, { recursive: true });
    cpSync(packetTunnelProfile.path, packetTunnelProvisioningProfileDestination);
  }

  return { app: appProfile, packetTunnel: packetTunnelProfile };
}

function findLibboxFramework() {
  if (!existsSync(libboxFramework)) {
    return null;
  }
  return libboxFramework;
}

function libboxFrameworkLinkage(frameworkPath) {
  const binary = libboxBinaryPath(frameworkPath);
  const result = capture("file", [binary], {
    cwd: repoRoot,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`file ${binary} failed with status ${result.status}`);
  }

  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if (output.includes("current ar archive")) {
    return "static";
  }
  if (output.includes("dynamically linked shared library")) {
    return "dynamic";
  }
  return "unknown";
}

function removeSwiftModuleArtifacts(binaryPath) {
  for (const extension of [".abi.json", ".swiftdoc", ".swiftmodule", ".swiftsourceinfo"]) {
    const artifact = `${binaryPath}${extension}`;
    if (existsSync(artifact)) {
      unlinkSync(artifact);
    }
  }
}

function removeDirectoryIfEmpty(path) {
  if (!existsSync(path)) {
    return;
  }
  if (readdirSync(path).length !== 0) {
    return;
  }
  rmdirSync(path);
}

function buildPacketTunnel() {
  const libboxFramework = findLibboxFramework();
  if (!libboxFramework) {
    const message = `Libbox.framework not found at ${libboxFramework}; PacketTunnel will build but fail closed until the framework is provided.`;
    if (truthy(process.env.VOYAVPN_REQUIRE_LIBBOX)) {
      throw new Error(message);
    }
    console.warn(message);
  }

  rmSync(incompatibleTunnelBundle, { force: true, recursive: true });
  mkdirSync(dirname(appexBinary), { recursive: true });
  const args = [
    "swiftc",
    "-O",
    "-emit-executable",
    "-parse-as-library",
    "-module-name",
    "VoyaPacketTunnel",
    "-framework",
    "Foundation",
    "-framework",
    "NetworkExtension",
    "-framework",
    "Network",
    "-framework",
    "AppKit",
    "-framework",
    "CoreText",
    "-framework",
    "SystemConfiguration",
    "-framework",
    "UniformTypeIdentifiers",
    "-lresolv",
    "-lbsm",
    "-Xlinker",
    "-e",
    "-Xlinker",
    "_NSExtensionMain",
  ];

  if (libboxFramework) {
    args.push(
      "-F",
      dirname(libboxFramework),
      "-framework",
      "Libbox",
      "-Xlinker",
      "-rpath",
      "-Xlinker",
      "@executable_path/../Frameworks",
    );
  }

  args.push(...providerSources, "-o", appexBinary);
  run("xcrun", args, { cwd: repoRoot });
  removeSwiftModuleArtifacts(appexBinary);

  writePlist(
    resolve(nativeRoot, "PacketTunnel", "Info.plist"),
    resolve(appexContents, "Info.plist"),
    {
      "$(PRODUCT_MODULE_NAME)": "VoyaPacketTunnel",
      "$(EXECUTABLE_NAME)": "VoyaPacketTunnel",
      "$(MARKETING_VERSION)": packetTunnelVersions().marketing,
      "$(CURRENT_PROJECT_VERSION)": packetTunnelVersions().build,
      "$(BUNDLE_PACKAGE_TYPE)": tunnelLayout.infoPackageType,
    },
  );

  rmSync(embeddedLibboxFramework, { force: true, recursive: true });
  if (libboxFramework) {
    const linkage = libboxFrameworkLinkage(libboxFramework);
    if (linkage === "static") {
      removeDirectoryIfEmpty(appexFrameworks);
      console.log(`Linked static Libbox.framework from ${libboxFramework}; no framework embedding is required.`);
      return;
    }

    rmSync(embeddedLibboxFramework, { force: true, recursive: true });
    mkdirSync(appexFrameworks, { recursive: true });
    cpSync(libboxFramework, embeddedLibboxFramework, {
      dereference: false,
      force: true,
      recursive: true,
      verbatimSymlinks: true,
    });
    console.log(`Embedded ${linkage} Libbox.framework from ${libboxFramework}`);
  }
}

function maybeCodesign(profiles) {
  const identity = resolvedIdentity?.sha1;
  if (!identity) {
    console.warn("Skipping codesign: VOYAVPN_CODESIGN_IDENTITY is not set.");
    console.warn(`App entitlements: ${appEntitlements}`);
    console.warn(`PacketTunnel entitlements: ${packetTunnelEntitlements}`);
    return;
  }

  const codesignArgs = ["--force", "--options", "runtime", "--sign", identity];
  if (!truthy(process.env.VOYAVPN_DISABLE_CODESIGN_TIMESTAMP)) {
    codesignArgs.push("--timestamp");
  }

  if (existsSync(embeddedLibboxFramework)) {
    run("codesign", [...codesignArgs, embeddedLibboxFramework], { cwd: repoRoot });
  }

  const packetEntitlements = entitlementsForSigning(
    packetTunnelEntitlements,
    profiles.packetTunnel,
    "packet-tunnel.plist",
  );
  run("codesign", [...codesignArgs, "--entitlements", packetEntitlements, appexBundle], { cwd: repoRoot });
}

function main() {
  requireDarwin("macOS native tunnel build must run on macOS with Xcode command line tools.");
  for (const source of providerSources) {
    if (!existsSync(source)) {
      throw new Error(`macOS PacketTunnel source is missing: ${source}`);
    }
  }
  buildPacketTunnel();
  const profiles = stageProvisioningProfiles();
  maybeCodesign(profiles);
  console.log(
    `macOS native tunnel staged for ${macosDistribution} as ${tunnelLayout.label}: ${appexBundle}`,
  );
}

// Guarded so importing this module (a unit test, another script) cannot start
// building an appex as a side effect of the import.
if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
