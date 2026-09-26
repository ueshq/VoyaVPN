import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { capture, isCliEntrypoint, repoRootFromScript, requireDarwin, run, truthy } from "../../lib/common.mjs";
import {
  appBundleIdentifier,
  codesignEntitlements,
  initializeTunnelLayout,
  packetTunnelBundleIdentifier,
  requireAbsent,
  requirePath,
  requiredNetworkExtensionValue,
} from "./tunnel-layout.mjs";
import {
  assertProfileCapabilities,
  decodeProvisioningProfile,
  inferDistribution,
  plistBuddy,
  signedNetworkExtensions,
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
let tunnel;

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
  assertProfileCapabilities(profile, { label, distribution: tunnel.distribution });

  console.log(`✓ ${label} provisioning profile: ${profile.name || profile.uuid || path}`);
  console.log(`✓ ${label} provisioning profile app id: ${profile.applicationIdentifier}`);
  return profile;
}

function profileRequiredEntitlements(profile) {
  if (!profile) {
    return [requiredNetworkExtensionValue(tunnel.distribution)];
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

function verifyNoIncompatibleTunnelBundle() {
  if (existsSync(tunnel.incompatibleBundle)) {
    throw new Error(
      `Incompatible PacketTunnel bundle is present for ${tunnel.distribution}: ${tunnel.incompatibleBundle}. Re-run pnpm native:macos:tunnel to stage only ${tunnel.layout.label}.`,
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

  const output = codesignEntitlements(path);
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
  const appexBinary = tunnel.layout.binary;
  const libbox = tunnel.layout.embeddedLibboxFramework;
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
  requirePath(appInfoPlist, "macOS app Info.plist", { log: true });
  const carbonRequirement = plistBuddy(appInfoPlist, ":LSRequiresCarbon", true);
  if (carbonRequirement) {
    throw new Error("macOS app Info.plist must not include LSRequiresCarbon; LaunchServices may refuse to open modern Tauri apps.");
  }
  console.log("✓ macOS app Info.plist does not include LSRequiresCarbon");
}

function main() {
  requireDarwin("macOS native tunnel verification must run on macOS.");
  run(process.execPath, [resolve(repoRoot, "scripts/native/macos/test-bridge.mjs")], { cwd: repoRoot });
  tunnel = initializeTunnelLayout(appContents, inferDistribution({ appContents }));
  requirePath(appBundle, "macOS app bundle", { log: true });
  verifyLaunchServicesMetadata();
  verifyNoIncompatibleTunnelBundle();
  requirePath(tunnel.layout.bundle, tunnel.layout.label, { log: true });
  requirePath(tunnel.layout.binary, "PacketTunnel binary", { log: true });
  requireAbsent(exportBindings, "Export bindings development tool", { log: true });
  requirePath(singBoxCoreSeed, "sing-box core seed", { log: true });
  verifyLibboxRuntime();

  const appProfile = verifyProvisioningProfile(appProvisioningProfile, "macOS app", appBundleIdentifier);
  const packetTunnelProfile = verifyProvisioningProfile(
    tunnel.layout.provisioningProfile,
    "PacketTunnel",
    packetTunnelBundleIdentifier,
  );

  verifySignature(appBundle, "macOS app bundle", [
    "com.apple.developer.networking.networkextension",
    ...profileRequiredEntitlements(appProfile),
    "com.apple.security.application-groups",
    "group.app.voyavpn.desktop",
    // Required, and questioned by App Review: the inherited-sandbox seed and the
    // self-hosted node listen (docs/release/app-store-review-notes.md).
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
  verifySignature(tunnel.layout.bundle, tunnel.layout.label, [
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
