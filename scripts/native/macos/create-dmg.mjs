import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { isCliEntrypoint, repoRootFromScript, requireDarwin, run, truthy } from "../../lib/common.mjs";
import {
  incompatiblePacketTunnelBundle,
  packetTunnelLayout,
  normalizeDistribution,
  resolveDmgPath,
} from "./tunnel-layout.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const packageJson = JSON.parse(readFileSync(resolve(repoRoot, "package.json"), "utf8"));
const appBundle = resolve(
  process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "release", "bundle", "macos", "VoyaVPN.app"),
);
const appContents = resolve(appBundle, "Contents");
const dmgDir = resolve(process.env.VOYAVPN_MACOS_DMG_DIR || resolve(repoRoot, "target", "release", "bundle", "dmg"));
const stagingRoot = resolve(repoRoot, "target", "native", "macos", "dmg-staging");
const stagingApp = resolve(stagingRoot, "VoyaVPN.app");
const stagingApplicationsLink = resolve(stagingRoot, "Applications");
const appProvisioningProfile = resolve(appContents, "embedded.provisionprofile");
let macosDistribution;
let tunnelLayout;
let incompatibleTunnelBundle;
let packetTunnelBundle;
let packetTunnelBinary;
let packetTunnelProvisioningProfile;

function requirePath(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} is missing: ${path}`);
  }
}

function optionalOrRequiredPath(path, label) {
  if (existsSync(path)) {
    return;
  }
  const message = `${label} is missing: ${path}`;
  if (truthy(process.env.VOYAVPN_REQUIRE_PROVISIONING)) {
    throw new Error(message);
  }
  console.warn(`! ${message}`);
}

function codesignEntitlements(path) {
  const result = spawnSync("codesign", ["-d", "--entitlements", ":-", path], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  return `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
}

function inferDistribution() {
  const explicit = normalizeDistribution(process.env.VOYAVPN_MACOS_DISTRIBUTION);
  if (explicit !== "auto") {
    return explicit;
  }
  const entitlements = existsSync(appBundle) ? codesignEntitlements(appBundle) : "";
  if (entitlements.includes("packet-tunnel-provider-systemextension")) {
    return "developer-id";
  }
  if (existsSync(packetTunnelLayout(appContents, "developer-id").bundle)) {
    return "developer-id";
  }
  return "app-store";
}

function initializeTunnelLayout() {
  macosDistribution = inferDistribution();
  tunnelLayout = packetTunnelLayout(appContents, macosDistribution);
  incompatibleTunnelBundle = incompatiblePacketTunnelBundle(appContents, macosDistribution);
  packetTunnelBundle = tunnelLayout.bundle;
  packetTunnelBinary = tunnelLayout.binary;
  packetTunnelProvisioningProfile = tunnelLayout.provisioningProfile;
}

function verifyFinalApp() {
  requirePath(appBundle, "macOS app bundle");
  if (existsSync(incompatibleTunnelBundle)) {
    throw new Error(
      `Incompatible PacketTunnel bundle is present for ${macosDistribution}: ${incompatibleTunnelBundle}. Re-run pnpm native:macos:tunnel before creating the DMG.`,
    );
  }
  requirePath(packetTunnelBundle, tunnelLayout.label);
  requirePath(packetTunnelBinary, "PacketTunnel binary");
  optionalOrRequiredPath(appProvisioningProfile, "macOS app provisioning profile");
  optionalOrRequiredPath(packetTunnelProvisioningProfile, "PacketTunnel provisioning profile");
}

function createStagingDirectory() {
  rmSync(stagingRoot, { force: true, recursive: true });
  mkdirSync(stagingRoot, { recursive: true });
  cpSync(appBundle, stagingApp, {
    dereference: false,
    force: true,
    recursive: true,
    verbatimSymlinks: true,
  });
  symlinkSync("/Applications", stagingApplicationsLink);
}

function createDmg(outputPath) {
  mkdirSync(dirname(outputPath), { recursive: true });
  rmSync(outputPath, { force: true });
  run("hdiutil", [
    "create",
    "-volname",
    "VoyaVPN",
    "-srcfolder",
    stagingRoot,
    "-fs",
    "HFS+",
    "-ov",
    "-format",
    "UDZO",
    outputPath,
  ], { cwd: repoRoot });
}

/**
 * Developer ID disk images must carry their own signature: `pnpm build:mac`
 * notarizes the DMG and then asserts `spctl --assess --context
 * context:primary-signature`, which rejects an unsigned image even when the app
 * inside it is signed, notarized, and stapled.
 */
export function dmgSigningPlan({ distribution, identity, requireCodesign = false, disableTimestamp = false }) {
  if (distribution !== "developer-id") {
    return { sign: false, reason: `${distribution} distribution does not ship a Developer ID signed disk image` };
  }

  const resolvedIdentity = String(identity ?? "").trim();
  if (!resolvedIdentity) {
    if (requireCodesign) {
      throw new Error(
        "VOYAVPN_CODESIGN_IDENTITY is required to sign the Developer ID DMG before notarization and Gatekeeper assessment.",
      );
    }
    return { sign: false, reason: "VOYAVPN_CODESIGN_IDENTITY is not set, so Gatekeeper will reject the disk image" };
  }

  const args = ["--force", "--sign", resolvedIdentity];
  if (!disableTimestamp) {
    args.push("--timestamp");
  }
  return { sign: true, args };
}

function signDmg(outputPath) {
  const plan = dmgSigningPlan({
    distribution: macosDistribution,
    identity: process.env.VOYAVPN_CODESIGN_IDENTITY,
    requireCodesign: truthy(process.env.VOYAVPN_REQUIRE_CODESIGN),
    disableTimestamp: truthy(process.env.VOYAVPN_DISABLE_CODESIGN_TIMESTAMP),
  });
  if (!plan.sign) {
    console.warn(`! Skipping DMG signing: ${plan.reason}`);
    return;
  }

  run("codesign", [...plan.args, outputPath], { cwd: repoRoot });
  run("codesign", ["--verify", "--strict", "--verbose=2", outputPath], { cwd: repoRoot });
  console.log(`macOS DMG signed for Developer ID distribution: ${outputPath}`);
}

function attachDmg(outputPath) {
  const mountPoint = resolve(repoRoot, "target", "native", "macos", "dmg-verify-mount");
  rmSync(mountPoint, { force: true, recursive: true });
  mkdirSync(mountPoint, { recursive: true });
  run("hdiutil", ["attach", "-nobrowse", "-readonly", "-mountpoint", mountPoint, outputPath], { cwd: repoRoot });
  return mountPoint;
}

function detachDmg(mountPoint) {
  run("hdiutil", ["detach", mountPoint], { cwd: repoRoot });
}

function verifyDmgContents(outputPath) {
  const mountPoint = attachDmg(outputPath);
  const mountedApp = resolve(mountPoint, "VoyaVPN.app");
  try {
    requirePath(mountedApp, "DMG macOS app bundle");
    requirePath(
      packetTunnelLayout(resolve(mountedApp, "Contents"), macosDistribution).bundle,
      `DMG ${tunnelLayout.label}`,
    );
    requirePath(
      packetTunnelLayout(resolve(mountedApp, "Contents"), macosDistribution).binary,
      "DMG PacketTunnel binary",
    );
    optionalOrRequiredPath(
      resolve(mountedApp, "Contents", "embedded.provisionprofile"),
      "DMG macOS app provisioning profile",
    );
    optionalOrRequiredPath(
      packetTunnelLayout(resolve(mountedApp, "Contents"), macosDistribution).provisioningProfile,
      "DMG PacketTunnel provisioning profile",
    );
    run("pnpm", ["native:macos:tunnel:verify"], {
      cwd: repoRoot,
      env: {
        ...process.env,
        VOYAVPN_MACOS_APP_BUNDLE: mountedApp,
        VOYAVPN_MACOS_DISTRIBUTION: macosDistribution,
      },
    });
  } finally {
    detachDmg(mountPoint);
  }
}

function main() {
  requireDarwin("macOS DMG creation must run on macOS.");
  initializeTunnelLayout();
  verifyFinalApp();
  createStagingDirectory();
  const outputPath = resolveDmgPath({ appContents, dmgDir, version: packageJson.version });
  createDmg(outputPath);
  try {
    signDmg(outputPath);
    verifyDmgContents(outputPath);
  } catch (error) {
    rmSync(outputPath, { force: true });
    throw error;
  }
  console.log(`macOS DMG created: ${outputPath}`);
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
