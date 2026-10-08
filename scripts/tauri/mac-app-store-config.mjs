import { resolve } from "node:path";

import { capture, truthy } from "../lib/common.mjs";
import { writeJson } from "../lib/fs.mjs";

/** LSMinimumSystemVersion of the store package; see `macAppStoreOverlay`. */
const macAppStoreMinimumSystemVersion = "26.0";

/** The shell feature that leaves the self-updater out of the binary. */
export const macAppStoreFeature = "mac-app-store";

/**
 * `vp run build:mac:appstore` sets this. It is a separate switch from
 * `VOYAVPN_MACOS_DISTRIBUTION=app-store` on purpose: `vp run build:mac:local`
 * signs the same App-Store-shaped appex for local TUN testing and must keep
 * its ordinary configuration.
 */
export function requestedMacAppStoreBuild(env = process.env) {
  return truthy(env.VOYAVPN_MAC_APP_STORE);
}

// CFBundleVersion for a Mac App Store upload: one to three period-separated
// non-negative integers, and each upload of one marketing version must be
// strictly greater than the last.
const bundleVersionPattern = /^\d+(?:\.\d+){0,2}$/u;

/**
 * The build number App Store Connect sees as CFBundleVersion.
 *
 * The environment variable named by `envName` wins; otherwise the commit count
 * of HEAD, which only grows on a linear main branch. Without either the upload
 * would reuse the marketing version, and App Store Connect rejects a repeated
 * build. The Mac and iOS lanes share the rule and differ only in the variable.
 */
export function resolveStoreBuildNumber({ envName, env = process.env, repoRoot, captureCommand = capture }) {
  const explicit = env[envName]?.trim();
  if (explicit) {
    if (!bundleVersionPattern.test(explicit)) {
      throw new Error(`${envName} must be one to three period-separated integers (got "${explicit}").`);
    }
    return explicit;
  }

  const result = captureCommand("git", ["rev-list", "--count", "HEAD"], { cwd: repoRoot });
  const count = String(result?.stdout ?? "").trim();
  if (result?.error || result?.status !== 0 || !/^\d+$/u.test(count)) {
    throw new Error(`Unable to derive the App Store build number from git; set ${envName}.`);
  }
  return count;
}

/** {@link resolveStoreBuildNumber} for the Mac App Store package. */
export function resolveMacAppStoreBuildNumber({ env = process.env, repoRoot, captureCommand = capture } = {}) {
  return resolveStoreBuildNumber({ envName: "VOYAVPN_MACOS_BUILD_NUMBER", env, repoRoot, captureCommand });
}

/**
 * The Tauri config the Mac App Store build merges over `tauri.conf.json`.
 *
 * - Only the `.app` bundle: the signed `.pkg` is made later by
 *   `scripts/native/macos/create-pkg.mjs`, after the PacketTunnel is staged.
 * - macOS 26 only. App Store Connect requires 12.0 or later for an arm64-only
 *   package (ITMS-90869), and the pinned sing-box seed is itself built for
 *   macOS 26, so no lower floor would run everything the package ships.
 * - No updater artifacts: the store delivers updates.
 */
export function macAppStoreOverlay({ buildNumber }) {
  return {
    bundle: {
      targets: ["app"],
      createUpdaterArtifacts: false,
      macOS: {
        minimumSystemVersion: macAppStoreMinimumSystemVersion,
        bundleVersion: buildNumber,
      },
    },
  };
}

export function writeMacAppStoreOverlay({ repoRoot, env = process.env, captureCommand = capture }) {
  const buildNumber = resolveMacAppStoreBuildNumber({ env, repoRoot, captureCommand });
  const overlayPath = resolve(repoRoot, "target", "release-config", "tauri.mac-app-store.generated.json");
  writeJson(overlayPath, macAppStoreOverlay({ buildNumber }), { onlyIfChanged: true });
  return { overlayPath, buildNumber };
}
