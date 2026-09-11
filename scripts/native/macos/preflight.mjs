import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { capture, repoRootFromScript } from "../../lib/common.mjs";
import {
  appBundleIdentifier,
  libboxBinaryPath,
  packetTunnelBundleIdentifier,
} from "./tunnel-layout.mjs";
import {
  findMatchingIdentities,
  formatProfileSelectionError,
  listCodesigningIdentities,
  localProvisioningUdid,
  parseCodesigningIdentities,
  resolveProfileFromEnv,
  resolveSigningIdentity,
} from "./provisioning.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const defaultProvisioningProfileDir = resolve(repoRoot, "..", "docs", "certs");
const provisioningProfileDir = resolve(process.env.VOYAVPN_PROVISIONING_PROFILE_DIR || defaultProvisioningProfileDir);
const defaultLibboxFramework = resolve(
  repoRoot,
  "apps",
  "desktop",
  "src-tauri",
  "native",
  "macos",
  "Frameworks",
  "Libbox.framework",
);
const libboxFramework = resolve(process.env.VOYAVPN_LIBBOX_FRAMEWORK || defaultLibboxFramework);
const installedAppBundle = "/Applications/VoyaVPN.app";
const localIdentityPattern = /Apple Development:|Mac Developer:/;
const runbook = "docs/release/macos-local-tun-testing.md";

const errors = [];
const warnings = [];

function ok(message) {
  console.log(`✓ ${message}`);
}

function fail(message) {
  errors.push(message);
  console.error(`✗ ${message.split("\n").join("\n  ")}`);
}

function warn(message) {
  warnings.push(message);
  console.warn(`! ${message}`);
}

function checkXcode() {
  const developerDir = spawnSync("xcode-select", ["-p"], { encoding: "utf8" });
  if (developerDir.status !== 0) {
    fail("Xcode command line tools are missing. Run xcode-select --install (or install Xcode).");
    return;
  }
  ok(`Xcode developer dir: ${developerDir.stdout.trim()}`);

  const swiftc = spawnSync("xcrun", ["--find", "swiftc"], { encoding: "utf8" });
  if (swiftc.status !== 0) {
    fail("swiftc is not available via xcrun; the PacketTunnel provider cannot be compiled. Install Xcode.");
    return;
  }
  ok(`swiftc: ${swiftc.stdout.trim()}`);
}

function checkIdentity() {
  const explicit = process.env.VOYAVPN_CODESIGN_IDENTITY?.trim();
  const spec = explicit || localIdentityPattern;
  const label = explicit ? "VOYAVPN_CODESIGN_IDENTITY" : "Apple Development or Mac Developer";
  try {
    const identity = resolveSigningIdentity(spec, label);
    ok(`Signing identity: ${identity.name} (${identity.sha1})`);
    return identity;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const hints = [];
    let validMatches;
    try {
      validMatches = findMatchingIdentities(listCodesigningIdentities(), spec).length;
    } catch {
      validMatches = 0;
    }
    const unvalidated = spawnSync("security", ["find-identity", "-p", "codesigning"], { encoding: "utf8" });
    const unvalidatedMatches = findMatchingIdentities(parseCodesigningIdentities(unvalidated.stdout ?? ""), spec);
    if (validMatches === 0 && unvalidatedMatches.length > 0) {
      hints.push(
        "A matching certificate exists in the keychain but is not valid: install Apple's WWDR intermediate certificate (G3) and confirm the private key is present.",
      );
    }
    if (validMatches === 0) {
      hints.push(
        `Create an Apple Development certificate from the CSR in ${provisioningProfileDir} on the Apple Developer portal, download it, and double-click to install. See ${runbook}.`,
      );
    }
    fail(hints.length ? `${message}\n${hints.join("\n")}` : message);
    return null;
  }
}

function checkUdid() {
  const udid = localProvisioningUdid();
  if (!udid) {
    warn(
      "Could not determine this Mac's Provisioning UDID (system_profiler SPHardwareDataType). Set VOYAVPN_PROVISIONING_UDID to verify device coverage.",
    );
    return null;
  }
  ok(`Provisioning UDID: ${udid} (register this device on the Apple Developer portal)`);
  return udid;
}

function checkProfile(identity, udid, bundleIdentifier, explicitEnvName, label) {
  if (!identity) {
    warn(`Skipping ${label} provisioning profile verification until a signing identity is available.`);
    return;
  }
  const criteria = {
    distribution: "app-store",
    allowDevelopmentProvisioning: true,
    identitySha1: identity?.sha1 ?? null,
    deviceUdid: udid,
  };
  let result;
  try {
    result = resolveProfileFromEnv({
      bundleIdentifier,
      explicitEnvName,
      profileDir: provisioningProfileDir,
      criteria,
    });
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
    return;
  }

  const { profile, rejections } = result;
  if (!profile) {
    fail(formatProfileSelectionError(`${label} development`, bundleIdentifier, rejections, provisioningProfileDir));
    return;
  }
  if (!profile.appGroups.includes("group.app.voyavpn.desktop")) {
    fail(`${label} profile ${profile.name || profile.path} does not include the group.app.voyavpn.desktop App Group.`);
    return;
  }
  if (!profile.networkExtensions.includes("packet-tunnel-provider")) {
    fail(`${label} profile ${profile.name || profile.path} does not include the packet-tunnel-provider Network Extension capability.`);
    return;
  }
  if (profile.expirationDate) {
    const expires = new Date(profile.expirationDate);
    if (!Number.isNaN(expires.getTime())) {
      const daysLeft = Math.floor((expires.getTime() - Date.now()) / 86_400_000);
      if (daysLeft < 7) {
        warn(`${label} profile ${profile.name || profile.path} expires in ${daysLeft} day(s) (${profile.expirationDate}).`);
      }
    }
  }
  ok(`${label} profile: ${profile.name || profile.path} (expires ${profile.expirationDate ?? "unknown"})`);
}

function checkLibbox() {
  if (!existsSync(libboxFramework)) {
    fail(`Libbox.framework is missing: ${libboxFramework}. Build it with pnpm native:macos:libbox.`);
    return;
  }
  const binary = libboxBinaryPath(libboxFramework);
  if (!existsSync(binary)) {
    fail(`Libbox.framework binary is missing: ${binary}. Rebuild it with pnpm native:macos:libbox.`);
    return;
  }
  const lipo = capture("lipo", ["-archs", binary], { cwd: repoRoot });
  const architectures = lipo.status === 0 ? lipo.stdout.trim().split(/\s+/) : [];
  const missing = ["arm64", "x86_64"].filter((architecture) => !architectures.includes(architecture));
  if (missing.length > 0) {
    fail(`Libbox.framework must be universal macOS (arm64, x86_64); missing ${missing.join(", ")}: ${binary}`);
    return;
  }
  ok(`Universal macOS Libbox.framework: ${libboxFramework}`);
}

function warnIfInstalledAppRunning() {
  for (const executable of ["voyavpn", "VoyaVPN", "VoyaPacketTunnel"]) {
    const result = spawnSync("pgrep", ["-x", executable], { encoding: "utf8" });
    if (result.status === 0) {
      warn(
        executable === "VoyaPacketTunnel"
          ? `The PacketTunnel provider (${executable}) is running; disable TUN before the build replaces ${installedAppBundle}.`
          : `VoyaVPN is running; quit it before the build replaces ${installedAppBundle}.`,
      );
    }
  }
}

function main() {
  if (process.platform !== "darwin") {
    throw new Error("pnpm native:macos:preflight must run on macOS.");
  }

  console.log("VoyaVPN macOS local TUN preflight");
  console.log(`Provisioning profile dir: ${provisioningProfileDir}`);
  console.log("");

  checkXcode();
  const identity = checkIdentity();
  const udid = checkUdid();
  checkProfile(identity, udid, appBundleIdentifier, "VOYAVPN_MACOS_APP_PROVISIONING_PROFILE", "macOS app");
  checkProfile(
    identity,
    udid,
    packetTunnelBundleIdentifier,
    "VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE",
    "PacketTunnel",
  );
  checkLibbox();
  warnIfInstalledAppRunning();

  console.log("");
  if (errors.length) {
    console.error(
      `Preflight failed with ${errors.length} error(s). Fix them and re-run pnpm native:macos:preflight. See ${runbook}.`,
    );
    process.exit(1);
  }
  console.log(
    warnings.length
      ? `Preflight passed with ${warnings.length} warning(s). pnpm build:mac:local is ready to run.`
      : "Preflight passed. pnpm build:mac:local is ready to run.",
  );
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
