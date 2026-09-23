import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { capture, isCliEntrypoint, repoRootFromScript, run, truthy } from "../../lib/common.mjs";
import {
  appBundleIdentifier,
  packetTunnelBundleIdentifier,
  packetTunnelLayout,
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
const outRoot = resolve(repoRoot, "target", "native", "macos");
const appBundle = resolve(process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "native", "macos", "VoyaVPN.app"));
const appContents = resolve(appBundle, "Contents");
const appInfoPlist = resolve(appContents, "Info.plist");
const appProvisioningProfileDestination = resolve(appContents, "embedded.provisionprofile");
const appEntitlements = resolve(repoRoot, "apps", "desktop", "src-tauri", "entitlements", "macos-app.plist");
const packetTunnelEntitlements = resolve(repoRoot, "apps", "desktop", "src-tauri", "entitlements", "packet-tunnel.plist");
const helperEntitlements = resolve(repoRoot, "apps", "desktop", "src-tauri", "entitlements", "macos-inherit.plist");
const windowsTunnelService = resolve(appContents, "MacOS", "voyavpn-tunnel-service");
const defaultProvisioningProfileDir = resolve(repoRoot, "..", "docs", "certs");
const provisioningProfileDir = resolve(process.env.VOYAVPN_PROVISIONING_PROFILE_DIR || defaultProvisioningProfileDir);
const generatedEntitlementsDir = resolve(outRoot, "generated-entitlements");
let tunnelLayout;
let packetTunnelBundle;
let packetTunnelProvisioningProfileDestination;
let embeddedLibboxFramework;

function provisioningProfile(bundleIdentifier, explicitEnvName, criteria) {
  return resolveProfileFromEnv({
    bundleIdentifier,
    explicitEnvName,
    profileDir: provisioningProfileDir,
    criteria,
  });
}

function validateProvisioningProfile(profile, criteria, label, bundleIdentifier) {
  const reason = profileRejectionReason(profile, { ...criteria, bundleIdentifier });
  if (reason) {
    throw new Error(`${label} provisioning profile ${profile.path} cannot be used: ${reason}.`);
  }
  assertProfileCapabilities(profile, { label, distribution: criteria.distribution });
}

function missingProfileError(label, bundleIdentifier, explicitEnvName, rejections) {
  return new Error(
    `${formatProfileSelectionError(label, bundleIdentifier, rejections, provisioningProfileDir)}\nSet ${explicitEnvName} to select a profile explicitly.`,
  );
}

function codesignBaseArgs(identity) {
  const args = ["--force", "--options", "runtime", "--sign", identity];
  if (!truthy(process.env.VOYAVPN_DISABLE_CODESIGN_TIMESTAMP)) {
    args.push("--timestamp");
  }
  return args;
}

/**
 * A helper the sandboxed app launches. Every macOS lane keeps App Sandbox
 * (ADR 0007), and App Store validation rejects a nested executable that does
 * not enable it, so helpers inherit the app's sandbox.
 */
function signSandboxedHelper(identity, path, label) {
  if (!existsSync(path)) {
    return;
  }
  run("codesign", [...codesignBaseArgs(identity), "--entitlements", helperEntitlements, path], { cwd: repoRoot });
  console.log(`Signed nested executable with inherited sandbox: ${label}`);
}

/**
 * `voyavpn-tunnel-service` is the Windows TUN service. Cargo builds every
 * `[[bin]]` of the shell and Tauri copies them all into `Contents/MacOS`, but
 * macOS runs its tunnel in the PacketTunnel provider and never launches it.
 * Shipping it would put an unsandboxed, unused executable in the bundle.
 */
function removeWindowsTunnelService() {
  if (!existsSync(windowsTunnelService)) {
    return;
  }
  rmSync(windowsTunnelService, { force: true });
  console.log("Removed the Windows-only voyavpn-tunnel-service from the macOS bundle.");
}

function signNestedCode(identity, packetTunnelProfile) {
  if (existsSync(embeddedLibboxFramework)) {
    run("codesign", [...codesignBaseArgs(identity), embeddedLibboxFramework], { cwd: repoRoot });
    console.log("Signed nested framework: Libbox.framework");
  }

  if (existsSync(packetTunnelBundle)) {
    const entitlements = packetTunnelProfile
      ? writeProfileEntitlements(
        packetTunnelProfile,
        resolve(generatedEntitlementsDir, "packet-tunnel.plist"),
        packetTunnelEntitlements,
      )
      : packetTunnelEntitlements;
    run("codesign", [...codesignBaseArgs(identity), "--entitlements", entitlements, packetTunnelBundle], { cwd: repoRoot });
    console.log(`Signed nested ${tunnelLayout.label}: PacketTunnel`);
  }

  signSandboxedHelper(identity, resolve(appContents, "Resources", "core-seeds", "sing_box", "sing-box"), "sing-box core seed");
}

function removeUnsupportedLaunchServicesKeys() {
  if (!existsSync(appInfoPlist)) {
    throw new Error(`macOS app Info.plist is missing: ${appInfoPlist}`);
  }

  const carbonRequirement = plistBuddy(appInfoPlist, ":LSRequiresCarbon", true);
  if (!carbonRequirement) {
    return;
  }

  run("/usr/libexec/PlistBuddy", ["-c", "Delete :LSRequiresCarbon", appInfoPlist], { cwd: repoRoot });
  console.log("Removed unsupported LSRequiresCarbon from macOS app Info.plist.");
}

function main() {
  if (process.platform !== "darwin") {
    throw new Error("macOS app signing must run on macOS.");
  }
  if (!existsSync(appBundle)) {
    throw new Error(`macOS app bundle is missing: ${appBundle}`);
  }

  const identityEnv = process.env.VOYAVPN_CODESIGN_IDENTITY;
  if (!identityEnv) {
    throw new Error("VOYAVPN_CODESIGN_IDENTITY is required to sign the macOS app bundle.");
  }
  const resolvedIdentity = resolveSigningIdentity(identityEnv);
  const identity = resolvedIdentity.sha1;

  const distribution = distributionFromIdentityName(resolvedIdentity.name, process.env.VOYAVPN_MACOS_DISTRIBUTION);
  const criteria = {
    distribution,
    allowDevelopmentProvisioning: truthy(process.env.VOYAVPN_ALLOW_DEVELOPMENT_PROVISIONING),
    identitySha1: resolvedIdentity.sha1,
    deviceUdid: localProvisioningUdid(),
  };
  tunnelLayout = packetTunnelLayout(appContents, distribution);
  packetTunnelBundle = tunnelLayout.bundle;
  packetTunnelProvisioningProfileDestination = tunnelLayout.provisioningProfile;
  embeddedLibboxFramework = tunnelLayout.embeddedLibboxFramework;
  removeUnsupportedLaunchServicesKeys();
  removeWindowsTunnelService();
  const appProfileResult = provisioningProfile(
    appBundleIdentifier,
    "VOYAVPN_MACOS_APP_PROVISIONING_PROFILE",
    criteria,
  );
  const appProfile = appProfileResult.profile;
  const packetTunnelProfileResult = existsSync(packetTunnelBundle)
    ? provisioningProfile(packetTunnelBundleIdentifier, "VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE", criteria)
    : { profile: null, rejections: [] };
  const packetTunnelProfile = packetTunnelProfileResult.profile;
  const entitlements = appProfile
    ? writeProfileEntitlements(appProfile, resolve(generatedEntitlementsDir, "macos-app.plist"), appEntitlements)
    : appEntitlements;
  if (appProfile) {
    validateProvisioningProfile(appProfile, criteria, "macOS app", appBundleIdentifier);
    cpSync(appProfile.path, appProvisioningProfileDestination);
    console.log(`Using macOS app provisioning profile ${appProfile.name || appProfile.path}`);
  } else if (distribution === "app-store" || distribution === "developer-id" || truthy(process.env.VOYAVPN_REQUIRE_PROVISIONING)) {
    throw missingProfileError(
      `macOS app ${distributionProfileLabel(distribution)}`,
      appBundleIdentifier,
      "VOYAVPN_MACOS_APP_PROVISIONING_PROFILE",
      appProfileResult.rejections,
    );
  }
  if (packetTunnelProfile) {
    validateProvisioningProfile(packetTunnelProfile, criteria, "PacketTunnel", packetTunnelBundleIdentifier);
    mkdirSync(dirname(packetTunnelProvisioningProfileDestination), { recursive: true });
    cpSync(packetTunnelProfile.path, packetTunnelProvisioningProfileDestination);
    console.log(`Using PacketTunnel provisioning profile ${packetTunnelProfile.name || packetTunnelProfile.path}`);
  } else if (existsSync(packetTunnelBundle) && truthy(process.env.VOYAVPN_REQUIRE_PROVISIONING)) {
    throw missingProfileError(
      `PacketTunnel ${distributionProfileLabel(distribution)}`,
      packetTunnelBundleIdentifier,
      "VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE",
      packetTunnelProfileResult.rejections,
    );
  }

  signNestedCode(identity, packetTunnelProfile);

  const args = [...codesignBaseArgs(identity), "--entitlements", entitlements];
  args.push(appBundle);

  run("codesign", args, { cwd: repoRoot });
  run("codesign", ["--verify", "--deep", "--strict", "--verbose=2", appBundle], { cwd: repoRoot });
  if (distribution === "developer-id") {
    const assessment = capture("spctl", ["--assess", "--type", "execute", "--verbose=4", appBundle], {
      cwd: repoRoot,
    });
    if (assessment.status === 0) {
      console.log("Developer ID app passed local spctl assessment.");
    } else if (truthy(process.env.VOYAVPN_REQUIRE_GATEKEEPER_ASSESSMENT)) {
      throw new Error(`spctl assessment failed for Developer ID app: ${assessment.stderr || assessment.stdout}`);
    } else {
      console.warn(
        "Skipping spctl failure for Developer ID signing because Gatekeeper assessment is expected to pass after notarization and stapling.",
      );
    }
  } else {
    const assessment = capture("spctl", ["--assess", "--type", "execute", "--verbose=4", appBundle], {
      cwd: repoRoot,
    });
    if (assessment.status === 0) {
      console.log("App Store/TestFlight app also passed local spctl assessment.");
    } else {
      console.warn(
        "Skipping spctl failure for App Store/TestFlight distribution; this artifact is intended for App Store Connect/TestFlight, not direct drag-to-Applications launch.",
      );
      console.warn(
        "For direct macOS distribution, sign with a Developer ID Application identity and run pnpm native:macos:app:notarize.",
      );
    }
  }
  console.log(`macOS app bundle signed for ${distribution}: ${appBundle}`);
}

// Guarded so importing this module (a unit test, another script) cannot start
// codesigning the app bundle as a side effect of the import.
if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
