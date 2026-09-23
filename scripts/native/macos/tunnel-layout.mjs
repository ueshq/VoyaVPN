import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { checkedCapture } from "../../lib/common.mjs";

export const appBundleIdentifier = "app.voyavpn.desktop";
export const packetTunnelBundleIdentifier = "app.voyavpn.desktop.PacketTunnel";
/**
 * The provider executable, and the appex folder name. App Store validation
 * requires an app extension's folder to be its CFBundleExecutable plus
 * `.appex` (ITMS-90362). The executable keeps this short name rather than
 * taking the bundle id: macOS truncates process names to 16 characters, and
 * the build and doctor scripts find the running provider by that name.
 */
export const packetTunnelExecutableName = "VoyaPacketTunnel";
/**
 * The appex folder name builds used before ITMS-90362 was caught. PlugInKit
 * can still hold registrations under it, so the doctor keeps recognizing it.
 */
export const legacyPacketTunnelAppexName = `${packetTunnelBundleIdentifier}.appex`;

/**
 * The provider sources, which macOS and iOS share.
 *
 * They live at `native/apple/` rather than under either app: the two ship the
 * same provider with different bundle ids, App Groups and Libbox slices, and a
 * copy would have drifted the first time one of them was fixed. What stays
 * per-platform is the `Info.plist` (which names the App Group the sources
 * read) and the framework that is linked in.
 */
function appleNativeRoot(repoRoot) {
  return resolve(repoRoot, "native", "apple");
}

/** The build and native checks compile exactly the same provider sources. */
export function packetTunnelSources(repoRoot) {
  return [
    "PacketTunnelProvider.swift",
    "PacketTunnelRuntime.swift",
    "PacketTunnelDiagnostics.swift",
    "PacketTunnelPlatform.swift",
  ].map((name) => resolve(appleNativeRoot(repoRoot), "PacketTunnel", name));
}

export function normalizeDistribution(value) {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (!normalized || normalized === "auto") {
    return "auto";
  }
  if (["developer-id", "developerid", "notarized", "dmg"].includes(normalized)) {
    return "developer-id";
  }
  if (["app-store", "appstore", "testflight", "mas", "development", "debug"].includes(normalized)) {
    return "app-store";
  }
  throw new Error("VOYAVPN_MACOS_DISTRIBUTION must be auto, developer-id, or app-store.");
}

export function distributionFromIdentityName(identityName, explicitValue) {
  const explicit = normalizeDistribution(explicitValue);
  if (explicit !== "auto") {
    return explicit;
  }

  const name = String(identityName ?? "");
  if (name.includes("3rd Party Mac Developer") || name.includes("Apple Distribution")) {
    return "app-store";
  }
  if (name.includes("Developer ID Application")) {
    return "developer-id";
  }

  return "app-store";
}

export function packagingModeForDistribution(distribution) {
  return distribution === "developer-id" ? "system-extension" : "app-extension";
}

/**
 * The PacketTunnel bundle's CFBundleShortVersionString / CFBundleVersion.
 *
 * App Store Connect rejects an embedded extension whose version fields differ
 * from the containing app (ITMS-90473), so the container's own Info.plist wins
 * and the root package.json version is the fallback for a bundle that has not
 * been written yet. These used to be the literals "0.1.0" and "1" inside
 * build-tunnel.mjs, which drift on any version bump and already disagreed with
 * each other.
 */
export function resolvePacketTunnelVersions({ appShortVersion, appBundleVersion, packageVersion } = {}) {
  const marketing = text(appShortVersion) || text(packageVersion);
  if (!marketing) {
    throw new Error(
      "Unable to resolve a PacketTunnel version: the app Info.plist has no CFBundleShortVersionString "
        + "and package.json has no version.",
    );
  }

  return { build: text(appBundleVersion) || marketing, marketing };
}

function text(value) {
  return typeof value === "string" ? value.trim() : "";
}

export function requiredNetworkExtensionValue(distribution) {
  return distribution === "developer-id" ? "packet-tunnel-provider-systemextension" : "packet-tunnel-provider";
}

export function packetTunnelLayout(appContents, distribution) {
  const mode = packagingModeForDistribution(distribution);
  const base =
    mode === "system-extension"
      ? resolve(appContents, "Library", "SystemExtensions", `${packetTunnelBundleIdentifier}.systemextension`)
      : resolve(appContents, "PlugIns", `${packetTunnelExecutableName}.appex`);
  const contents = resolve(base, "Contents");
  const label = mode === "system-extension" ? "PacketTunnel system extension" : "PacketTunnel appex";

  return {
    binary: resolve(contents, "MacOS", packetTunnelExecutableName),
    bundle: base,
    contents,
    embeddedLibboxFramework: resolve(contents, "Frameworks", "Libbox.framework"),
    frameworks: resolve(contents, "Frameworks"),
    infoPackageType: mode === "system-extension" ? "SYSX" : "XPC!",
    label,
    mode,
    provisioningProfile: resolve(contents, "embedded.provisionprofile"),
  };
}

/** An appex left over from a build that still used the bundle-id folder name. */
export function legacyPacketTunnelAppexBundle(appContents) {
  return resolve(appContents, "PlugIns", legacyPacketTunnelAppexName);
}

function versionParts(value) {
  return String(value ?? "")
    .trim()
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);
}

/** Compares dotted macOS versions numerically: `compareMacosVersions("12.0", "11.5") > 0`. */
export function compareMacosVersions(left, right) {
  const a = versionParts(left);
  const b = versionParts(right);
  for (let index = 0; index < Math.max(a.length, b.length, 2); index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference !== 0) return Math.sign(difference);
  }
  return 0;
}

/**
 * The deployment target the PacketTunnel is compiled for and declares in its
 * Info.plist: the containing app's LSMinimumSystemVersion, so the provider
 * runs everywhere the app installs.
 *
 * Without `-target`, swiftc builds for the host's macOS release, which marked
 * the provider for macOS 26 inside an app that declared 10.15. Apple Silicon
 * has no release before macOS 11, so an arm64 build is raised to at least 11.0.
 */
export function resolvePacketTunnelDeploymentTarget({ appMinimumSystemVersion, fallbackMinimumSystemVersion, hostArch = process.arch }) {
  const declared = text(appMinimumSystemVersion) || text(fallbackMinimumSystemVersion);
  if (!declared) {
    throw new Error("Unable to resolve the PacketTunnel deployment target: the app declares no LSMinimumSystemVersion.");
  }
  const arch = hostArch === "arm64" ? "arm64" : "x86_64";
  const minimumSystemVersion =
    arch === "arm64" && compareMacosVersions(declared, "11.0") < 0 ? "11.0" : declared;
  return { minimumSystemVersion, target: `${arch}-apple-macos${minimumSystemVersion}` };
}

export function incompatiblePacketTunnelBundle(appContents, distribution) {
  const oppositeDistribution = distribution === "developer-id" ? "app-store" : "developer-id";
  return packetTunnelLayout(appContents, oppositeDistribution).bundle;
}

export function libboxBinaryPath(frameworkPath) {
  const direct = join(frameworkPath, "Libbox");
  return existsSync(direct) ? direct : join(frameworkPath, "Versions", "A", "Libbox");
}

/** The build and DMG signing steps must resolve the same artifact filename. */
export function resolveDmgPath({
  appContents,
  dmgDir,
  version,
  env = process.env,
  hostArch = process.arch,
  captureCommand = checkedCapture,
}) {
  const explicit = env.VOYAVPN_MACOS_DMG_PATH?.trim();
  if (explicit) return resolve(explicit);

  let arch = env.VOYAVPN_MACOS_DMG_ARCH?.trim();
  if (!arch) {
    const executableName = captureCommand(
      "/usr/libexec/PlistBuddy",
      ["-c", "Print :CFBundleExecutable", resolve(appContents, "Info.plist")],
      { env },
    ).stdout.trim();
    const executable = resolve(appContents, "MacOS", executableName);
    const archs = captureCommand("lipo", ["-archs", executable], { env }).stdout.trim().split(/\s+/);
    const hasArm64 = archs.includes("arm64");
    const hasX64 = archs.includes("x86_64");
    if (hasArm64 && hasX64) arch = "universal";
    else if (hasArm64) arch = "aarch64";
    else if (hasX64) arch = "x64";
    else arch = hostArch === "arm64" ? "aarch64" : hostArch;
  }

  return resolve(dmgDir, `VoyaVPN_${version}_${arch}.dmg`);
}
