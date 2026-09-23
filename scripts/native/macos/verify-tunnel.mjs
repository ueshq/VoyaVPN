import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { capture, isCliEntrypoint, repoRootFromScript, requireDarwin, run, truthy } from "../../lib/common.mjs";
import {
  appBundleIdentifier,
  incompatiblePacketTunnelBundle,
  packetTunnelBundleIdentifier,
  packetTunnelLayout,
  requiredNetworkExtensionValue,
  distributionFromIdentityName,
  normalizeDistribution,
} from "./tunnel-layout.mjs";
import {
  assertProfileCapabilities,
  plistBuddy,
  signedNetworkExtensions,
  decodeProvisioningProfile as decodeProfileWithDir,
} from "./provisioning.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const appBundle = resolve(
  process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "native", "macos", "VoyaVPN.app"),
);
const appContents = resolve(appBundle, "Contents");
const appInfoPlist = resolve(appContents, "Info.plist");
const appProvisioningProfile = resolve(appContents, "embedded.provisionprofile");
const tunnelService = resolve(appContents, "MacOS", "voyavpn-tunnel-service");
// A cargo example since the package stopped shipping it; a copy here means a
// stale bin target is being bundled again.
const exportBindings = resolve(appContents, "MacOS", "export-bindings");
// The PacketTunnel's Libbox runs the connection; this seed only measures nodes
// while disconnected.
const singBoxCoreSeed = resolve(appContents, "Resources", "core-seeds", "sing_box", "sing-box");
// Only entry points the provider actually calls: the appex links with
// `-dead_strip`, which drops Libbox exports nothing references.
const libboxSymbols = ["_LibboxSetup", "_LibboxNewCommandServer", "_LibboxGetTunnelFileDescriptor"];
let macosDistribution;
let tunnelLayout;
let incompatibleTunnelBundle;
let appex;
let appexBinary;
let packetTunnelProvisioningProfile;
let libbox;

function requirePath(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} is missing: ${path}`);
  }
  console.log(`✓ ${label}: ${path}`);
}

function requireAbsent(path, label) {
  if (existsSync(path)) {
    throw new Error(`${label} must not be packaged: ${path}`);
  }
  console.log(`✓ ${label} is not packaged`);
}

function decodeProvisioningProfile(profilePath) {
  const decodedDir = resolve(repoRoot, "target", "native", "macos", "verify-provisioning-profiles");
  return decodeProfileWithDir(profilePath, decodedDir);
}

function verifyProvisioningProfile(path, label, bundleIdentifier) {
  if (!existsSync(path)) {
    const message = `${label} provisioning profile is missing: ${path}`;
    if (truthy(process.env.VOYAVPN_REQUIRE_PROVISIONING)) {
      throw new Error(message);
    }
    console.warn(`! ${message}`);
    return null;
  }

  const profile = decodeProvisioningProfile(path);
  if (profile.bundleIdentifier !== bundleIdentifier) {
    throw new Error(`${label} provisioning profile bundle id mismatch: expected ${bundleIdentifier}, got ${profile.bundleIdentifier}.`);
  }
  assertProfileCapabilities(profile, { label, distribution: macosDistribution });

  console.log(`✓ ${label} provisioning profile: ${profile.name || profile.uuid || path}`);
  console.log(`✓ ${label} provisioning profile app id: ${profile.applicationIdentifier}`);
  return profile;
}

function profileRequiredEntitlements(profile) {
  if (!profile) {
    return [requiredNetworkExtensionValue(macosDistribution)];
  }
  const required = [...signedNetworkExtensions(profile)];
  if (profile.systemExtensionInstall) {
    required.push("com.apple.developer.system-extension.install");
  }
  if (profile.appSandbox) {
    required.push("com.apple.security.app-sandbox");
  }
  if (profile.networkClient) {
    required.push("com.apple.security.network.client");
  }
  return required;
}

function profileDistribution(profile) {
  if (
    profile.developerCertificateSubjects?.some((subject) => subject.includes("CN=Developer ID Application:"))
    || profile.networkExtensions.includes("packet-tunnel-provider-systemextension")
  ) {
    return "developer-id";
  }
  return "app-store";
}

function inferDistribution() {
  const explicit = normalizeDistribution(process.env.VOYAVPN_MACOS_DISTRIBUTION);
  if (explicit !== "auto") {
    return explicit;
  }
  if (existsSync(appProvisioningProfile)) {
    return profileDistribution(decodeProvisioningProfile(appProvisioningProfile));
  }
  if (existsSync(packetTunnelLayout(appContents, "developer-id").bundle)) {
    return "developer-id";
  }
  return distributionFromIdentityName("", "app-store");
}

function initializeTunnelLayout() {
  macosDistribution = inferDistribution();
  tunnelLayout = packetTunnelLayout(appContents, macosDistribution);
  incompatibleTunnelBundle = incompatiblePacketTunnelBundle(appContents, macosDistribution);
  appex = tunnelLayout.bundle;
  appexBinary = tunnelLayout.binary;
  packetTunnelProvisioningProfile = tunnelLayout.provisioningProfile;
  libbox = tunnelLayout.embeddedLibboxFramework;
}

function verifyNoIncompatibleTunnelBundle() {
  if (existsSync(incompatibleTunnelBundle)) {
    throw new Error(
      `Incompatible PacketTunnel bundle is present for ${macosDistribution}: ${incompatibleTunnelBundle}. Re-run pnpm native:macos:tunnel to stage only ${tunnelLayout.label}.`,
    );
  }
}

function verifySignature(path, label, requiredEntitlements = []) {
  const verify = capture("codesign", ["--verify", "--strict", "--verbose=2", path], { cwd: repoRoot });
  if (verify.status !== 0) {
    const message = `${label} is not signed or failed signature verification.`;
    if (truthy(process.env.VOYAVPN_REQUIRE_CODESIGN)) {
      throw new Error(`${message}\n${verify.stderr || verify.stdout}`);
    }
    console.warn(`! ${message}`);
    return "";
  }

  console.log(`✓ ${label} signature is valid`);
  verifyNotarizationReadySignature(path, label);

  if (!requiredEntitlements.length) {
    return "";
  }

  const entitlements = capture("codesign", ["-d", "--entitlements", ":-", path], { cwd: repoRoot });
  const output = `${entitlements.stdout ?? ""}\n${entitlements.stderr ?? ""}`;
  for (const entitlement of requiredEntitlements) {
    if (!output.includes(entitlement)) {
      const message = `${label} signature does not include ${entitlement}.`;
      if (truthy(process.env.VOYAVPN_REQUIRE_CODESIGN)) {
        throw new Error(message);
      }
      console.warn(`! ${message}`);
    } else {
      console.log(`✓ ${label} entitlement includes ${entitlement}`);
    }
  }
  return output;
}

function verifyNotarizationReadySignature(path, label) {
  if (!truthy(process.env.VOYAVPN_REQUIRE_NOTARIZATION_READY)) {
    return;
  }

  const details = capture("codesign", ["-dv", "--verbose=4", path], { cwd: repoRoot });
  const output = `${details.stdout ?? ""}\n${details.stderr ?? ""}`;
  const checks = [
    ["Developer ID Application authority", "Authority=Developer ID Application:"],
    ["secure timestamp", "Timestamp="],
    ["hardened runtime", "runtime"],
  ];
  for (const [description, token] of checks) {
    if (!output.includes(token)) {
      throw new Error(`${label} signature is not notarization-ready: missing ${description}.`);
    }
    console.log(`✓ ${label} signature includes ${description}`);
  }
}

function verifyLibboxRuntime() {
  const nm = capture("nm", ["-gU", appexBinary], { cwd: repoRoot });
  const symbols = `${nm.stdout ?? ""}\n${nm.stderr ?? ""}`;
  const hasStaticLibbox = nm.status === 0 && libboxSymbols.every((symbol) => symbols.includes(symbol));
  if (hasStaticLibbox) {
    console.log("✓ Static Libbox symbols are linked into PacketTunnel binary");
    return;
  }

  if (existsSync(libbox)) {
    console.log(`✓ Embedded Libbox.framework: ${libbox}`);
    verifySignature(libbox, "Libbox.framework");
    return;
  }

  const message = `Libbox runtime is missing: no static symbols in ${appexBinary} and no embedded framework at ${libbox}`;
  if (truthy(process.env.VOYAVPN_REQUIRE_LIBBOX)) {
    throw new Error(message);
  }
  console.warn(`! ${message}`);
}

function verifyLaunchServicesMetadata() {
  requirePath(appInfoPlist, "macOS app Info.plist");
  const carbonRequirement = plistBuddy(appInfoPlist, ":LSRequiresCarbon", true);
  if (carbonRequirement) {
    throw new Error("macOS app Info.plist must not include LSRequiresCarbon; LaunchServices may refuse to open modern Tauri apps.");
  }
  console.log("✓ macOS app Info.plist does not include LSRequiresCarbon");
}

function main() {
  requireDarwin("macOS native tunnel verification must run on macOS.");
  run(process.execPath, [resolve(repoRoot, "scripts/native/macos/test-bridge.mjs")], { cwd: repoRoot });
  initializeTunnelLayout();
  requirePath(appBundle, "macOS app bundle");
  verifyLaunchServicesMetadata();
  verifyNoIncompatibleTunnelBundle();
  requirePath(appex, tunnelLayout.label);
  requirePath(appexBinary, "PacketTunnel binary");
  requireAbsent(exportBindings, "Export bindings development tool");
  requirePath(singBoxCoreSeed, "sing-box core seed");
  verifyLibboxRuntime();

  const appProfile = verifyProvisioningProfile(appProvisioningProfile, "macOS app", appBundleIdentifier);
  const packetTunnelProfile = verifyProvisioningProfile(
    packetTunnelProvisioningProfile,
    "PacketTunnel",
    packetTunnelBundleIdentifier,
  );

  verifySignature(appBundle, "macOS app bundle", [
    "com.apple.developer.networking.networkextension",
    ...profileRequiredEntitlements(appProfile),
    "com.apple.security.application-groups",
    "group.app.voyavpn.desktop",
    "com.apple.security.network.server",
    ...(appProfile ? [
      "com.apple.application-identifier",
      appProfile.applicationIdentifier,
      "com.apple.developer.team-identifier",
      appProfile.teamIdentifier,
    ] : []),
  ]);
  if (existsSync(tunnelService)) {
    verifySignature(tunnelService, "Tunnel service binary");
  }
  verifySignature(singBoxCoreSeed, "sing-box core seed binary", [
    "com.apple.security.app-sandbox",
    "com.apple.security.inherit",
  ]);
  verifySignature(appex, tunnelLayout.label, [
    "com.apple.developer.networking.networkextension",
    ...profileRequiredEntitlements(packetTunnelProfile),
    "com.apple.security.application-groups",
    "group.app.voyavpn.desktop",
    "com.apple.security.network.server",
    ...(packetTunnelProfile ? [
      "com.apple.application-identifier",
      packetTunnelProfile.applicationIdentifier,
      "com.apple.developer.team-identifier",
      packetTunnelProfile.teamIdentifier,
    ] : []),
  ]);
}

// Guarded so importing this module (a unit test, another script) cannot start
// shelling out to codesign/pluginkit as a side effect of the import.
if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
